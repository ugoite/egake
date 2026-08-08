/* egake's dependency-free, same-origin browser runtime. */
(function () {
  "use strict";

  var API = "/api/egake/v1";
  var requestCounter = 0;
  var model = { app: null, state: {}, records: {}, schemas: {}, selected: {}, loading: {}, errors: [], toast: null };
  var root = document.getElementById("egake-root");

  function el(tag, text) {
    var node = document.createElement(tag);
    if (text !== undefined && text !== null) node.textContent = String(text);
    return node;
  }

  function attr(node, name, value) {
    if (value !== undefined && value !== null) node.setAttribute(name, String(value));
    return node;
  }

  function valueOf(value) {
    return value === null || value === undefined ? "" : String(value);
  }

  function toast(message, kind) {
    model.toast = { message: message, kind: kind || "ok" };
    render();
    window.setTimeout(function () {
      if (model.toast && model.toast.message === message) { model.toast = null; render(); }
    }, 3200);
  }

  function apiError(response, payload) {
    var error = payload && payload.error ? payload.error : {};
    var code = error.code || "internal";
    var message = error.message || ("Request failed (" + response.status + ")");
    if (error.fields) {
      var fields = Object.keys(error.fields).sort().map(function (key) { return key + ": " + error.fields[key]; });
      if (fields.length) message += " — " + fields.join(", ");
    }
    var requestId = error.request_id ? " (request " + error.request_id + ")" : "";
    var result = new Error(code + ": " + message + requestId);
    result.code = code;
    result.fields = error.fields || {};
    result.request_id = error.request_id || null;
    return result;
  }

  function request(path, options) {
    requestCounter += 1;
    var requestId = "req-browser-" + requestCounter.toString(36);
    var requestOptions = Object.assign({ credentials: "same-origin" }, options || {});
    requestOptions.headers = Object.assign({ "Content-Type": "application/json", "x-request-id": requestId }, (options && options.headers) || {});
    return window.fetch(API + path, requestOptions)
      .then(function (response) {
        return response.text().then(function (text) {
          var payload = {};
          try { payload = text ? JSON.parse(text) : {}; } catch (_) { payload = {}; }
          if (!response.ok) throw apiError(response, payload);
          return payload;
        });
      });
  }

  function componentAttr(component, name) {
    return component.attributes && component.attributes[name];
  }

  function findAction(name) {
    return model.app.actions.find(function (action) { return action.name === name; });
  }

  function resourceName(component) {
    return componentAttr(component, "resource");
  }

  function schemaField(resource, fieldName) {
    var schema = model.schemas[resource];
    return schema && (schema.fields || []).find(function (field) { return field.name === fieldName; });
  }

  function formatMatches(value, format) {
    if (typeof value !== "string") return false;
    if (format === "email") {
      var email = value.split("@");
      return email.length === 2 && email[0].length > 0 && email[1].indexOf(".") > 0 && !/\s/.test(value);
    }
    if (format === "date") return /^\d{4}-\d{2}-\d{2}$/.test(value);
    if (format === "date-time") return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?$/.test(value);
    return true;
  }

  function localValidationError(message, fields) {
    var error = new Error(message);
    error.code = "validation_failed";
    error.fields = fields;
    return error;
  }

  function formComponent() {
    var page = model.app.pages[0];
    var found = null;
    function visit(components) {
      (components || []).some(function (component) {
        if (component.kind === "form") { found = component; return true; }
        return visit(component.children);
      });
    }
    visit(page && page.components);
    return found;
  }

  function formFields(form) {
    var fields = [];
    function visit(components) {
      (components || []).forEach(function (component) {
        if (["text-input", "select", "textarea"].indexOf(component.kind) >= 0 && component.attributes && component.attributes.field) {
          fields.push(component.attributes.field);
        }
        visit(component.children);
      });
    }
    visit(form && form.children);
    return fields;
  }

  function setDraftFromRecord(record) {
    model.state.draft = record ? Object.assign({}, record) : {};
  }

  function resourceKey(resource) {
    var page = model.app.pages[0];
    var result = "id";
    function visit(components) {
      (components || []).some(function (component) {
        if (component.kind === "data-table" && resourceName(component) === resource) {
          result = componentAttr(component, "key") || "id";
          return true;
        }
        return visit(component.children);
      });
    }
    visit(page && page.components);
    return result;
  }

  function rememberError(error) {
    model.errors.push(error && error.message ? error.message : String(error));
    if (model.errors.length > 3) model.errors.shift();
    render();
  }

  function actionInput(raw, context) {
    if (raw === undefined || raw === null) return context && context.record ? context.record : {};
    if (typeof raw !== "string") return raw;
    if (raw.indexOf("$state.") === 0) return model.state[raw.slice(7)];
    if (raw.indexOf("$context.") === 0) {
      var path = raw.slice(9).split(".");
      var value = context;
      path.forEach(function (part) { value = value === undefined || value === null ? undefined : value[part]; });
      return value;
    }
    try { return JSON.parse(raw); } catch (_) { return raw; }
  }

  function invokeProviderAction(resource, action, input) {
    if (!resource || !action) return Promise.reject(new Error("invoke requires resource and action"));
    return request("/resources/" + encodeURIComponent(resource) + "/actions/" + encodeURIComponent(action), {
      method: "POST",
      body: JSON.stringify(input)
    }).then(function (result) {
      model.lastActionResult = result;
      toast("Action " + action + " completed", "ok");
      return result;
    }).catch(function (error) {
      rememberError(error);
      toast(error.message, "error");
      throw error;
    });
  }

  function runAction(name, context) {
    var action = findAction(name);
    if (!action) { toast("Unknown action: " + name, "error"); return Promise.resolve(); }
    context = context || {};
    if (name === "open-create" || name.indexOf("open-create") >= 0) {
      setDraftFromRecord(null); model.state.editorOpen = true; model.state.editorId = null; render(); return Promise.resolve();
    }
    if (name === "open-edit" || name.indexOf("open-edit") >= 0) {
      if (!context.record) { toast("Select a row first", "error"); return Promise.resolve(); }
      setDraftFromRecord(context.record); model.state.editorOpen = true;
      model.state.editorId = context.id || context.record[resourceKey(context.resource || firstResource())];
      render(); return Promise.resolve();
    }
    var steps = action.steps || [];
    var resourceForAction = context.resource || firstResource();
    var chain = (name.indexOf("delete") >= 0 || name === "remove")
      ? deleteSelected(resourceForAction)
      : Promise.resolve();
    steps.forEach(function (step) {
      chain = chain.then(function () {
        var attrs = step.attributes || {};
        var resource = attrs.resource || resourceForAction;
        if (step.kind === "refresh") return refresh(resource);
        if (step.kind === "toast") { toast(step.text || "Done", "ok"); return undefined; }
        if (step.kind === "upsert") return save(resource).then(function () { model.state.editorOpen = false; });
        if (step.kind === "validate") return validateDraft(resource);
        if (step.kind === "invoke") {
          return invokeProviderAction(resource, attrs.action, actionInput(attrs.input, context));
        }
        return undefined;
      });
    });
    if (steps.length === 0 && (name.indexOf("save") >= 0 || name.indexOf("upsert") >= 0)) {
      return save(resourceForAction).then(function () { model.state.editorOpen = false; });
    }
    return chain.then(render);
  }

  function validateDraft(resource) {
    if (!model.state.draft || typeof model.state.draft !== "object") return Promise.reject(new Error("editor value must be an object"));
    return request("/resources/" + encodeURIComponent(resource) + "/schema").then(function (schema) {
      model.schemas[resource] = schema;
      var fields = {};
      (schema.fields || []).forEach(function (field) {
        var value = model.state.draft[field.name];
        var empty = value === undefined || value === null || value === "";
        if (field.required && empty) fields[field.name] = "is required";
        if (empty) return;
        if (Array.isArray(field.enum) && !field.enum.some(function (expected) { return valueOf(expected) === valueOf(value); })) {
          fields[field.name] = "must be one of the declared values";
        }
        if (field.format && !formatMatches(valueOf(value), field.format)) fields[field.name] = "has an invalid format";
      });
      if (Object.keys(fields).length) throw localValidationError("Form values do not match the resource schema", fields);
    });
  }

  function save(resource) {
    if (!resource) return Promise.reject(new Error("No resource is attached to this form"));
    var key = resourceKey(resource);
    var draft = Object.assign({}, model.state.draft || {});
    var editing = model.state.editorId !== null && model.state.editorId !== undefined;
    if (!editing && !draft[key]) {
      draft[key] = "new-" + Date.now().toString(36) + "-" + Math.floor(Math.random() * 1000000).toString(36);
      model.state.draft = draft;
    }
    return validateDraft(resource).then(function () {
      var id = editing ? model.state.editorId : null;
      var path = "/resources/" + encodeURIComponent(resource);
      var options;
      if (editing) { delete draft[key]; options = { method: "PATCH", body: JSON.stringify(draft) }; path += "/items/" + encodeURIComponent(id); }
      else { options = { method: "POST", body: JSON.stringify(draft) }; }
      return request(path, options).then(function () { return refresh(resource); }).then(function () { toast("Saved", "ok"); });
    }).catch(function (error) { rememberError(error); toast(error.message, "error"); throw error; });
  }

  function deleteSelected(resource) {
    var key = resourceKey(resource);
    var id = model.state.editorId || (model.state.draft && model.state.draft[key]);
    if (!resource || !id) { toast("Select a record first", "error"); return Promise.resolve(); }
    if (!window.confirm("Delete this record?")) return Promise.resolve();
    return request("/resources/" + encodeURIComponent(resource) + "/items/" + encodeURIComponent(id), { method: "DELETE" })
      .then(function () { model.state.editorOpen = false; model.state.editorId = null; return refresh(resource); })
      .then(function () { toast("Deleted", "ok"); })
      .catch(function (error) { rememberError(error); toast(error.message, "error"); });
  }

  function refresh(resource) {
    if (!resource) return Promise.resolve();
    model.loading[resource] = true;
    render();
    var query = valueOf(model.state.query);
    var sort = componentTableSort(resource);
    var params = new URLSearchParams();
    if (query) params.set("q", query);
    if (sort) params.set("sort", sort);
    params.set("limit", "500");
    return request("/resources/" + encodeURIComponent(resource) + "?" + params.toString()).then(function (page) {
      model.loading[resource] = false;
      model.records[resource] = page.items || []; render(); return page;
    }).catch(function (error) {
      model.loading[resource] = false;
      rememberError(error); toast(error.message, "error"); throw error;
    });
  }

  function componentTableSort(resource) {
    var page = model.app.pages[0];
    var result = null;
    function visit(components) {
      (components || []).some(function (component) {
        if (component.kind === "data-table" && resourceName(component) === resource) {
          result = component.attributes && component.attributes.sort; return true;
        }
        return visit(component.children);
      });
    }
    visit(page && page.components);
    return result || "";
  }

  function safeId(value) {
    return valueOf(value).toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "") || "value";
  }

  function closeEditor() {
    model.state.editorOpen = false;
    model.state.editorId = null;
    render();
  }

  function renderText(component) {
    var text = el("p", component.text || componentAttr(component, "label") || "");
    attr(text, "class", "egake-text");
    return text;
  }

  function renderButton(component) {
    var button = el("button", component.text || componentAttr(component, "label") || "Action");
    attr(button, "class", "egake-button");
    attr(button, "type", "button");
    var variant = componentAttr(component, "variant");
    if (variant) attr(button, "data-variant", variant);
    if (component.id) attr(button, "id", component.id);
    button.addEventListener("click", function () {
      runAction(componentAttr(component, "action"), {}).catch(function () {});
    });
    return button;
  }

  function renderInput(component, formResource) {
    var field = componentAttr(component, "field");
    var binding = componentAttr(component, "bind");
    var label = componentAttr(component, "label") || field || "Value";
    var wrapper = el("div"); attr(wrapper, "class", "egake-field");
    var inputId = component.id || "egake-field-" + safeId(field || label);
    var labelNode = el("label");
    labelNode.textContent = label;
    attr(labelNode, "for", inputId);
    wrapper.appendChild(labelNode);
    var metadata = field ? schemaField(formResource, field) : null;
    var input = component.kind === "textarea" ? el("textarea") : component.kind === "select" ? el("select") : el("input");
    attr(input, "class", "egake-control");
    attr(input, "id", inputId);
    if (component.kind === "text-input") attr(input, "type", metadata && metadata.format === "email" ? "email" : metadata && metadata.format === "date" ? "date" : metadata && metadata.format === "date-time" ? "datetime-local" : "text");
    if (metadata && metadata.required) attr(input, "required", "required");
    if (field) attr(input, "name", field);
    if (metadata && metadata.required) {
      var required = el("span", "*");
      attr(required, "class", "egake-required");
      required.setAttribute("aria-hidden", "true");
      labelNode.appendChild(required);
    }
    var stateName = binding && binding.indexOf("state.") === 0 ? binding.slice(6) : null;
    input.value = stateName ? valueOf(model.state[stateName]) : valueOf(model.state.draft && model.state.draft[field]);
    if (component.kind === "select") {
      var values = metadata && Array.isArray(metadata.enum) ? metadata.enum : [];
      var current = input.value;
      if (!values.length) {
        var option = el("option", current || "Select a value"); option.value = current; input.appendChild(option);
      } else {
        var placeholder = el("option", "Select a value"); placeholder.value = ""; input.appendChild(placeholder);
        if (current && !values.some(function (value) { return valueOf(value) === current; })) values = [current].concat(values);
      }
      values.forEach(function (value) {
        var option = el("option", valueOf(value)); option.value = valueOf(value); input.appendChild(option);
      });
      input.value = current;
    }
    input.addEventListener("input", function () {
      if (stateName) {
        model.state[stateName] = input.value;
        if (stateName === "query") {
          var resource = firstResource();
          refresh(resource).catch(function () {});
        }
      } else if (field) {
        if (!model.state.draft || typeof model.state.draft !== "object") model.state.draft = {};
        model.state.draft[field] = input.value;
      }
    });
    wrapper.appendChild(input);
    return wrapper;
  }

  function renderTable(component) {
    var resource = resourceName(component), tableWrap = el("div"); attr(tableWrap, "class", "egake-table-wrap");
    var tableLabel = el("span", resource || "Records");
    attr(tableLabel, "class", "egake-table-toolbar-label");
    var toolbar = el("div"); attr(toolbar, "class", "egake-table-toolbar");
    toolbar.appendChild(tableLabel);
    var refreshButton = el("button", "Refresh");
    attr(refreshButton, "class", "egake-button");
    attr(refreshButton, "type", "button");
    refreshButton.addEventListener("click", function () { refresh(resource).catch(function () {}); });
    toolbar.appendChild(refreshButton);
    tableWrap.appendChild(toolbar);
    var table = el("table"), head = el("thead"), headRow = el("tr");
    attr(table, "class", "egake-table");
    attr(table, "aria-label", resource ? resource + " records" : "Records");
    attr(table, "aria-busy", model.loading[resource] ? "true" : "false");
    (component.children || []).forEach(function (column) { headRow.appendChild(el("th", componentAttr(column, "label") || componentAttr(column, "field"))); });
    head.appendChild(headRow); table.appendChild(head);
    var body = el("tbody");
    (model.records[resource] || []).forEach(function (record) {
      var row = el("tr"); attr(row, "data-selectable", "true"); attr(row, "tabindex", "0");
      var key = componentAttr(component, "key") || "id";
      var selected = model.state.editorId && valueOf(record[key]) === valueOf(model.state.editorId);
      if (selected) {
        attr(row, "data-selected", "true");
        attr(row, "aria-selected", "true");
      }
      (component.children || []).forEach(function (column) { row.appendChild(el("td", valueOf(record[componentAttr(column, "field")]))); });
      function selectRow() {
        model.selected[resource] = record;
        var event = (component.events || []).find(function (binding) { return binding.event === "select"; });
        var recordId = record[key];
        if (event) runAction(event.action, { resource: resource, record: record, id: recordId }).catch(function () {});
        else { setDraftFromRecord(record); model.state.editorOpen = true; model.state.editorId = recordId; render(); }
      }
      row.addEventListener("click", selectRow);
      row.addEventListener("keydown", function (event) {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          selectRow();
        }
      });
      body.appendChild(row);
    });
    if (model.loading[resource] || !(model.records[resource] || []).length) {
      var emptyRow = el("tr"), emptyCell = el("td", model.loading[resource] ? "Loading records…" : "No records yet.");
      attr(emptyRow, "class", "egake-table-empty-row");
      attr(emptyCell, "class", "egake-table-empty");
      if (model.loading[resource]) attr(emptyCell, "data-state", "loading");
      attr(emptyCell, "colspan", Math.max((component.children || []).length, 1));
      emptyRow.appendChild(emptyCell);
      body.appendChild(emptyRow);
    }
    table.appendChild(body); tableWrap.appendChild(table);
    return tableWrap;
  }

  function renderComponent(component, formResource) {
    var node;
    if (component.kind === "column") { node = el("div"); attr(node, "class", "egake-column"); }
    else if (component.kind === "row") { node = el("div"); attr(node, "class", "egake-row"); attr(node, "data-align", componentAttr(component, "align") || "start"); }
    else if (component.kind === "text") return renderText(component);
    else if (component.kind === "button") return renderButton(component);
    else if (["text-input", "select", "textarea"].indexOf(component.kind) >= 0) return renderInput(component, formResource);
    else if (component.kind === "data-table") return renderTable(component);
    else if (component.kind === "form") {
      node = el("section"); attr(node, "class", "egake-form"); attr(node, "data-mode", componentAttr(component, "mode") || "inline");
      if (!model.state.editorOpen) node.hidden = true;
      if (component.id) attr(node, "id", component.id);
      if (["drawer", "dialog"].indexOf(componentAttr(component, "mode")) >= 0) {
        attr(node, "role", "dialog");
        attr(node, "aria-modal", "true");
        var formTitle = el("h2", model.state.editorId === null || model.state.editorId === undefined ? "New record" : "Edit record");
        attr(formTitle, "class", "egake-form-title");
        var formHeading = el("div"); attr(formHeading, "class", "egake-form-head");
        formHeading.appendChild(formTitle);
        var close = el("button", "×");
        attr(close, "class", "egake-form-close");
        attr(close, "type", "button");
        attr(close, "aria-label", "Close editor");
        close.addEventListener("click", closeEditor);
        formHeading.appendChild(close);
        node.appendChild(formHeading);
      }
      var formResourceName = firstResource();
      (component.children || []).forEach(function (child) {
        var childNode = renderComponent(child, formResourceName);
        if (child.kind === "row") attr(childNode, "class", "egake-row egake-form-actions");
        node.appendChild(childNode);
      });
      return node;
    } else node = el("div");
    (component.children || []).forEach(function (child) { node.appendChild(renderComponent(child, formResource)); });
    return node;
  }

  function firstResource() { return model.app.resources.length ? model.app.resources[0].name : null; }

  function render() {
    if (!root || !model.app) return;
    while (root.firstChild) root.removeChild(root.firstChild);
    var page = model.app.pages[0];
    var shell = el("main"); attr(shell, "class", "egake-shell");
    var topbar = el("header"); attr(topbar, "class", "egake-topbar");
    var brand = el("div"); attr(brand, "class", "egake-brand");
    var brandMark = el("span", "e"); attr(brandMark, "class", "egake-brand-mark"); brandMark.setAttribute("aria-hidden", "true");
    brand.appendChild(brandMark); brand.appendChild(el("span", "egake"));
    topbar.appendChild(brand);
    var topbarTitle = el("span", page ? page.title : model.app.profile.name); attr(topbarTitle, "class", "egake-topbar-title");
    topbar.appendChild(topbarTitle);
    shell.appendChild(topbar);
    var main = el("div"); attr(main, "class", "egake-shell-main");
    var pageHeader = el("header"); attr(pageHeader, "class", "egake-page-header");
    var pageHeading = el("div"); attr(pageHeading, "class", "egake-page-heading");
    var eyebrow = el("span", "Workspace"); attr(eyebrow, "class", "egake-eyebrow");
    pageHeading.appendChild(eyebrow);
    var title = el("h1", page ? page.title : model.app.profile.name); attr(title, "class", "egake-page-title");
    pageHeading.appendChild(title); pageHeader.appendChild(pageHeading); main.appendChild(pageHeader);
    var content = el("div"); attr(content, "class", "egake-content");
    if (model.errors.length) {
      var errors = el("div"); attr(errors, "class", "egake-error"); attr(errors, "role", "alert"); errors.appendChild(el("strong", "Runtime errors"));
      var errorList = el("div"); attr(errorList, "class", "egake-error-list");
      model.errors.forEach(function (message) { errorList.appendChild(el("div", message)); });
      errors.appendChild(errorList); content.appendChild(errors);
    }
    (page ? page.components : []).forEach(function (component) { content.appendChild(renderComponent(component, firstResource())); });
    var form = formComponent();
    if (model.state.editorOpen && form && ["drawer", "dialog"].indexOf(componentAttr(form, "mode")) >= 0) {
      var backdrop = el("button");
      attr(backdrop, "class", "egake-backdrop");
      attr(backdrop, "type", "button");
      attr(backdrop, "aria-label", "Close editor");
      backdrop.addEventListener("click", closeEditor);
      content.appendChild(backdrop);
    }
    main.appendChild(content); shell.appendChild(main);
    if (model.toast) { var toastNode = el("div", model.toast.message); attr(toastNode, "class", "egake-toast"); attr(toastNode, "data-kind", model.toast.kind); attr(toastNode, "role", "status"); attr(toastNode, "aria-live", "polite"); shell.appendChild(toastNode); }
    root.appendChild(shell);
  }

  function load() {
    var inline = document.getElementById("egake-application");
    var application = inline
      ? Promise.resolve().then(function () {
        try { return JSON.parse(inline.textContent || ""); }
        catch (_) { throw new Error("Inline application bundle was not valid JSON"); }
      })
      : window.fetch(["app", "bundle", "json"].join("."), { headers: { "Accept": "application/json" } }).then(function (response) {
        if (!response.ok) throw new Error("Application bundle could not be loaded");
        return response.json();
      });
    return application.then(function (app) {
      model.app = app; (app.states || []).forEach(function (state) { model.state[state.name] = state.value; });
      (app.resources || []).forEach(function (resource) { model.schemas[resource.name] = { fields: resource.fields || [] }; });
      document.title = app.profile.name; render();
      return Promise.all((app.resources || []).filter(function (resource) { return (resource.capabilities || resource.required_capabilities || []).indexOf("list") >= 0; }).map(function (resource) {
        return request("/resources/" + encodeURIComponent(resource.name) + "/schema").then(function (schema) {
          model.schemas[resource.name] = schema; render();
        }).then(function () { return refresh(resource.name); });
      }));
    }).catch(function (error) { rememberError(error); });
  }

  load();
}());

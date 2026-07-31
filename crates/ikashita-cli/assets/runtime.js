/* ikashita's dependency-free, same-origin browser runtime. */
(function () {
  "use strict";

  var API = "/api/ikashita/v1";
  var model = { app: null, state: {}, records: {}, selected: {}, errors: [], toast: null };
  var root = document.getElementById("ikashita-root");

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
    return window.fetch(API + path, Object.assign({ headers: { "Content-Type": "application/json" } }, options || {}))
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
        if (step.kind === "invoke") return undefined;
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
      var missing = (schema.fields || []).filter(function (field) { return field.required && !valueOf(model.state.draft[field.name]); });
      if (missing.length) throw new Error("Required fields are missing: " + missing.map(function (field) { return field.name; }).join(", "));
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
    var query = valueOf(model.state.query);
    var sort = componentTableSort(resource);
    var params = new URLSearchParams();
    if (query) params.set("q", query);
    if (sort) params.set("sort", sort);
    params.set("limit", "500");
    return request("/resources/" + encodeURIComponent(resource) + "?" + params.toString()).then(function (page) {
      model.records[resource] = page.items || []; render(); return page;
    }).catch(function (error) { rememberError(error); toast(error.message, "error"); throw error; });
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

  function renderText(component) { return el("p", component.text || componentAttr(component, "label") || ""); }

  function renderButton(component) {
    var button = el("button", component.text || componentAttr(component, "label") || "Action");
    var variant = componentAttr(component, "variant");
    if (variant) attr(button, "data-variant", variant);
    button.addEventListener("click", function () {
      runAction(componentAttr(component, "action"), {}).catch(function () {});
    });
    return button;
  }

  function renderInput(component, formResource) {
    var field = componentAttr(component, "field");
    var binding = componentAttr(component, "bind");
    var label = componentAttr(component, "label") || field || "Value";
    var wrapper = el("div"); attr(wrapper, "class", "ikashita-field");
    wrapper.appendChild(el("label", label));
    var input = component.kind === "textarea" ? el("textarea") : component.kind === "select" ? el("select") : el("input");
    if (component.kind === "text-input") attr(input, "type", "text");
    attr(input, "name", field);
    var stateName = binding && binding.indexOf("state.") === 0 ? binding.slice(6) : null;
    input.value = stateName ? valueOf(model.state[stateName]) : valueOf(model.state.draft && model.state.draft[field]);
    if (component.kind === "select") {
      var current = input.value;
      var option = el("option", current || "Select a value"); option.value = current; input.appendChild(option);
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
    var resource = resourceName(component), tableWrap = el("div"); attr(tableWrap, "class", "ikashita-table-wrap");
    var refreshButton = el("button", "Refresh");
    attr(refreshButton, "type", "button");
    refreshButton.addEventListener("click", function () { refresh(resource).catch(function () {}); });
    tableWrap.appendChild(refreshButton);
    var table = el("table"), head = el("thead"), headRow = el("tr");
    (component.children || []).forEach(function (column) { headRow.appendChild(el("th", componentAttr(column, "label") || componentAttr(column, "field"))); });
    head.appendChild(headRow); table.appendChild(head);
    var body = el("tbody");
    (model.records[resource] || []).forEach(function (record) {
      var row = el("tr"); attr(row, "data-selectable", "true");
      var key = componentAttr(component, "key") || "id";
      if (model.state.editorId && valueOf(record[key]) === valueOf(model.state.editorId)) attr(row, "data-selected", "true");
      (component.children || []).forEach(function (column) { row.appendChild(el("td", valueOf(record[componentAttr(column, "field")]))); });
      row.addEventListener("click", function () {
        model.selected[resource] = record;
        var event = (component.events || []).find(function (binding) { return binding.event === "select"; });
        var recordId = record[key];
        if (event) runAction(event.action, { resource: resource, record: record, id: recordId }).catch(function () {});
        else { setDraftFromRecord(record); model.state.editorOpen = true; model.state.editorId = recordId; render(); }
      });
      body.appendChild(row);
    });
    table.appendChild(body); tableWrap.appendChild(table);
    return tableWrap;
  }

  function renderComponent(component, formResource) {
    var node;
    if (component.kind === "column") { node = el("div"); attr(node, "class", "ikashita-column"); }
    else if (component.kind === "row") { node = el("div"); attr(node, "class", "ikashita-row"); attr(node, "data-align", componentAttr(component, "align") || "start"); }
    else if (component.kind === "text") return renderText(component);
    else if (component.kind === "button") return renderButton(component);
    else if (["text-input", "select", "textarea"].indexOf(component.kind) >= 0) return renderInput(component, formResource);
    else if (component.kind === "data-table") return renderTable(component);
    else if (component.kind === "form") {
      node = el("section"); attr(node, "class", "ikashita-form"); attr(node, "data-mode", componentAttr(component, "mode") || "inline");
      if (!model.state.editorOpen) node.hidden = true;
      var formResourceName = firstResource();
      (component.children || []).forEach(function (child) { node.appendChild(renderComponent(child, formResourceName)); });
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
    var shell = el("main"); attr(shell, "class", "ikashita-shell");
    shell.appendChild(el("h1", page ? page.title : model.app.profile.name));
    if (model.errors.length) {
      var errors = el("div"); attr(errors, "class", "ikashita-error"); errors.appendChild(el("strong", "Runtime errors"));
      model.errors.forEach(function (message) { errors.appendChild(el("div", message)); }); shell.appendChild(errors);
    }
    (page ? page.components : []).forEach(function (component) { shell.appendChild(renderComponent(component, firstResource())); });
    if (model.toast) { var toastNode = el("div", model.toast.message); attr(toastNode, "class", "ikashita-toast"); attr(toastNode, "data-kind", model.toast.kind); shell.appendChild(toastNode); }
    root.appendChild(shell);
  }

  function load() {
    return window.fetch("app.bundle.json", { headers: { "Accept": "application/json" } }).then(function (response) {
      if (!response.ok) throw new Error("Application bundle could not be loaded");
      return response.json();
    }).then(function (app) {
      model.app = app; (app.states || []).forEach(function (state) { model.state[state.name] = state.value; });
      document.title = app.profile.name; render();
      return Promise.all((app.resources || []).filter(function (resource) { return resource.capabilities.indexOf("list") >= 0; }).map(function (resource) { return refresh(resource.name); }));
    }).catch(function (error) { rememberError(error); });
  }

  load();
}());

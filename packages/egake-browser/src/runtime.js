// deno-lint-ignore-file no-var no-inner-declarations no-redeclare
/* Egake owns application state, actions, and resource access. */
(function () {
  "use strict";
  var API = "/api/egake/v1";
  var requestCounter = 0;

  function emit(node, type, detail) {
    node.dispatchEvent(
      new CustomEvent(type, {
        detail: detail,
        bubbles: true,
        composed: true,
      }),
    );
  }

  function domEventName(name) {
    return {
      action: "ika-action",
      edit: "ika-edit",
      query: "ika-query",
      select: "ika-select",
    }[name] || name;
  }

  function targetFor(root, id) {
    if (!id) return null;
    var target = document.getElementById(id);
    return target && root.contains(target) ? target : null;
  }

  function request(path, options) {
    requestCounter += 1;
    var requestId = "req-browser-" + requestCounter.toString(36);
    var requestOptions = Object.assign(
      { credentials: "same-origin" },
      options || {},
    );
    requestOptions.headers = Object.assign(
      {
        "Content-Type": "application/json",
        "x-request-id": requestId,
      },
      (options && options.headers) || {},
    );
    requestOptions.headers["x-request-id"] = requestId;
    return fetch(API + path, requestOptions).then(function (response) {
      return response.text().then(function (text) {
        var payload = {};
        try {
          payload = text ? JSON.parse(text) : {};
        } catch (_) {
          payload = {};
        }
        if (!response.ok) {
          var apiError = payload && payload.error;
          var error = new Error(
            (apiError && apiError.message) ||
              "Egake resource request failed (" + response.status + ")",
          );
          error.code = apiError && apiError.code;
          error.fields = (apiError && apiError.fields) || {};
          error.requestId = apiError && apiError.request_id;
          throw error;
        }
        return payload;
      });
    });
  }

  function stateName(reference) {
    if (typeof reference !== "string") return null;
    if (reference.indexOf("state.") === 0) return reference.slice(6);
    if (reference.indexOf("$state.") === 0) return reference.slice(7);
    return null;
  }

  function formField(reference) {
    if (typeof reference !== "string" || reference.indexOf("form.") !== 0) {
      return null;
    }
    var rest = reference.slice(5);
    var separator = rest.indexOf(".");
    if (separator <= 0 || separator === rest.length - 1) return null;
    return { form: rest.slice(0, separator), field: rest.slice(separator + 1) };
  }

  function actionInput(raw, context, state) {
    if (raw === undefined || raw === null) {
      return context && context.record ? context.record : {};
    }
    if (typeof raw !== "string") return raw;
    var stateReference = stateName(raw);
    if (stateReference !== null) return state[stateReference];
    if (raw.indexOf("form.") === 0) {
      return context && context.formValue !== undefined
        ? context.formValue
        : state.draft;
    }
    if (raw.indexOf("$context.") === 0) {
      return raw.slice(9).split(".").reduce(function (value, part) {
        return value === undefined || value === null ? undefined : value[part];
      }, context);
    }
    try {
      return JSON.parse(raw);
    } catch (_) {
      return raw;
    }
  }

  function findAction(bundle, name) {
    return (bundle.actions || []).find(function (action) {
      return action.name === name;
    });
  }

  function findResourceBinding(bundle, resource) {
    return (bundle.bindings || []).find(function (binding) {
      return binding.kind === "resource" && binding.resource === resource;
    });
  }

  function resourceDescriptor(bundle, resource) {
    var binding = findResourceBinding(bundle, resource);
    var definition = (bundle.resources || []).find(function (candidate) {
      return candidate.name === resource;
    });
    if (!binding && !definition) return undefined;
    return Object.assign({}, binding || { resource: resource }, {
      key: definition && definition.key || binding && binding.key || "id",
    });
  }

  function gridBindings(root, bundle, resource) {
    return (bundle.bindings || []).filter(function (binding) {
      return binding.kind === "resource" && binding.resource === resource;
    }).map(function (binding) {
      return {
        binding: binding,
        grid: targetFor(root, binding.target),
      };
    }).filter(function (item) {
      return item.grid && item.grid.tagName.toLowerCase() === "ika-data-grid";
    });
  }

  function queryGrid(grid, binding, query) {
    query = query || { offset: 0, limit: 50, sort: [] };
    var token = (grid.__egakeRequestToken || 0) + 1;
    grid.__egakeRequestToken = token;
    grid.__egakeQuery = query;
    var params = new URLSearchParams({
      offset: String(query.offset),
      limit: String(query.limit),
    });
    if (query.filter) params.set("q", query.filter);
    if (query.sort && query.sort.length) {
      params.set(
        "sort",
        query.sort.map(function (sort) {
          return sort.direction === "desc" ? "-" + sort.field : sort.field;
        }).join(","),
      );
    }
    grid.loading = true;
    grid.error = undefined;
    return request(
      "/resources/" + encodeURIComponent(binding.resource) + "?" +
        params.toString(),
    ).then(function (page) {
      if (grid.__egakeRequestToken !== token) return;
      var records = {};
      grid.rows = (page.items || []).map(function (item) {
        var cells = {};
        (binding.columns || []).forEach(function (column) {
          cells[column.id] = item[column.field || column.id];
        });
        var id = String(item[binding.key || "id"]);
        records[id] = item;
        return {
          id: id,
          cells: cells,
        };
      });
      grid.__egakeRecords = records;
      grid.total = Number(page.total || 0);
    }).catch(function (error) {
      if (grid.__egakeRequestToken === token) grid.error = error.message;
    }).finally(function () {
      if (grid.__egakeRequestToken === token) grid.loading = false;
    });
  }

  function editGrid(grid, bundle, binding, detail) {
    var column = (binding.columns || []).find(function (candidate) {
      return candidate.id === detail.columnId;
    });
    if (!column || !column.field) {
      grid.error = "DataGrid column has no resource field";
      return Promise.resolve();
    }
    var field = (bundle.resources || []).flatMap(function (resource) {
      return resource.name === binding.resource ? (resource.fields || []) : [];
    }).find(function (candidate) {
      return candidate.name === column.field;
    });
    var value = coerceFieldValue(field, detail.value);
    var patch = {};
    patch[column.field] = value;
    patch = normalizeResourceValue(bundle, binding.resource, patch);
    var record = grid.__egakeRecords && grid.__egakeRecords[detail.rowId];
    var descriptor = resourceDescriptor(bundle, binding.resource);
    var resourceKey = descriptor && (descriptor.key || "id");
    var resourceId = record && record[resourceKey] !== undefined
      ? record[resourceKey]
      : detail.rowId;
    grid.loading = true;
    return request(
      "/resources/" + encodeURIComponent(binding.resource) + "/items/" +
        encodeURIComponent(String(resourceId)),
      { method: "PATCH", body: JSON.stringify(patch) },
    ).then(function () {
      grid.request(grid.__egakeQuery || {});
      return grid.__egakeQueryPromise || Promise.resolve();
    }).catch(function (error) {
      grid.error = error.message;
    }).finally(function () {
      grid.loading = false;
    });
  }

  function setFormValues(root, bundle, state, value, form) {
    var draft = value && typeof value === "object" ? value : {};
    var formBinding = form && (bundle.bindings || []).find(function (binding) {
      return binding.kind === "value" && binding.target === form.id &&
        binding.bind;
    });
    var stateKey = formBinding && stateName(formBinding.bind);
    var formValue = Object.assign({}, draft);
    if (stateKey !== null && stateKey !== undefined) {
      state[stateKey] = formValue;
    } else state.draft = formValue;
    (bundle.bindings || []).filter(function (binding) {
      return binding.kind === "field" &&
        (!form || form.contains(targetFor(root, binding.target)));
    }).forEach(function (binding) {
      var target = targetFor(root, binding.target);
      if (target && "value" in target) {
        target.value = formValue[binding.field] ?? "";
      }
    });
  }

  function formTargetForAction(root, context) {
    if (context && context.form) return targetFor(root, context.form);
    var source = context && context.target;
    var closest = source && source.closest && source.closest("ika-form");
    if (closest) return closest;
    var forms = root.querySelectorAll("ika-form");
    return forms.length === 1 ? forms[0] : null;
  }

  function formValueForTarget(bundle, state, form) {
    var binding = form && (bundle.bindings || []).find(function (candidate) {
      return candidate.kind === "value" && candidate.target === form.id &&
        candidate.bind;
    });
    var stateKey = binding && stateName(binding.bind);
    return stateKey ? state[stateKey] : state.draft;
  }

  function selectedRecord(bundle, target, detail) {
    if (!target || !target.rows || !detail) return undefined;
    var row = target.rows.find(function (candidate) {
      return candidate.id === detail.rowId;
    });
    if (!row) return undefined;
    var binding = (bundle.bindings || []).find(function (candidate) {
      return candidate.kind === "resource" && candidate.target === target.id;
    });
    var record = target.__egakeRecords &&
        target.__egakeRecords[detail.rowId]
      ? Object.assign({}, target.__egakeRecords[detail.rowId])
      : {};
    (binding && binding.columns || []).forEach(function (column) {
      record[column.field || column.id] = row.cells[column.id];
    });
    record[(binding && binding.key) || "id"] = row.id;
    return record;
  }

  function formatMatches(value, format) {
    if (format === "email") return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
    if (format === "date") return /^\d{4}-\d{2}-\d{2}$/.test(value);
    if (format === "date-time") {
      return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:\d{2})?$/
        .test(value);
    }
    return true;
  }

  function validationError(fields) {
    var error = new Error("Form values do not match the resource schema");
    error.code = "validation_failed";
    error.fields = fields;
    return error;
  }

  function validateActionTarget(
    bundle,
    state,
    context,
    target,
    resourceName,
    value,
  ) {
    var raw = value === undefined ? actionInput(target, context, state) : value;
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
      return Promise.reject(
        validationError({ form: "editor value must be an object" }),
      );
    }
    var resourceBinding = (bundle.bindings || []).find(function (binding) {
      return binding.kind === "resource";
    });
    var schema = (bundle.resources || []).find(function (resource) {
      return resource.name ===
        (resourceName || context.resource ||
          (resourceBinding && resourceBinding.resource));
    });
    var fields = {};
    (schema && schema.fields || []).forEach(function (field) {
      var value = raw[field.name];
      var empty = value === undefined || value === null || value === "";
      if (field.required && empty) fields[field.name] = "is required";
      if (empty) return;
      if (
        Array.isArray(field.enum) && !field.enum.some(function (expected) {
          return String(expected) === String(value);
        })
      ) fields[field.name] = "must be one of the declared values";
      if (field.format && !formatMatches(String(value), field.format)) {
        fields[field.name] = "has an invalid format";
      }
    });
    return Object.keys(fields).length
      ? Promise.reject(validationError(fields))
      : Promise.resolve();
  }

  function showStatus(root, message, kind, fields) {
    var status = root.querySelector("[data-egake-status]");
    if (!status) {
      status = document.createElement("ika-text");
      status.setAttribute("data-egake-status", "");
      status.setAttribute("role", "status");
      status.setAttribute("aria-live", "polite");
      root.prepend(status);
    }
    var detail = fields && Object.keys(fields).length
      ? " — " + Object.keys(fields).sort().map(function (field) {
        return field + ": " + fields[field];
      }).join(", ")
      : "";
    status.textContent = message + detail;
    status.dataset.kind = kind || "info";
    status.hidden = false;
  }

  function refreshResource(root, bundle, resource, state) {
    return Promise.all(
      gridBindings(root, bundle, resource).map(function (item) {
        var query = Object.assign(
          {},
          item.grid.__egakeQuery || {
            offset: 0,
            limit: 50,
            sort: [],
          },
        );
        var previousFilter = query.filter || "";
        var nextFilter = state.query || "";
        if (previousFilter !== nextFilter) query.offset = 0;
        query.filter = nextFilter || undefined;
        item.grid.request(query);
        return item.grid.__egakeQueryPromise || Promise.resolve();
      }),
    );
  }

  function refreshAllResources(root, bundle, state) {
    var resources = [];
    (bundle.bindings || []).forEach(function (binding) {
      if (
        binding.kind !== "resource" || resources.indexOf(binding.resource) >= 0
      ) {
        return;
      }
      resources.push(binding.resource);
    });
    return Promise.all(resources.map(function (resource) {
      return refreshResource(root, bundle, resource, state);
    }));
  }

  function coerceFieldValue(field, value) {
    if (!field || value === "") return value;
    if (field.field_type === "number" || field.field_type === "integer") {
      var number = Number(value);
      return Number.isFinite(number) ? number : value;
    }
    if (field.field_type === "boolean") {
      if (value === "true") return true;
      if (value === "false") return false;
    }
    if (field.field_type === "json" && typeof value === "string") {
      try {
        return JSON.parse(value);
      } catch (_) { /* provider reports invalid JSON */ }
    }
    return value;
  }

  function normalizeResourceValue(bundle, resource, value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return value;
    }
    var schema = (bundle.resources || []).find(function (candidate) {
      return candidate.name === resource;
    });
    if (!schema || !schema.fields) return value;
    var normalized = Object.assign({}, value);
    schema.fields.forEach(function (field) {
      if (Object.prototype.hasOwnProperty.call(normalized, field.name)) {
        normalized[field.name] = coerceFieldValue(
          field,
          normalized[field.name],
        );
      }
      if (
        field.format === "date-time" &&
        typeof normalized[field.name] === "string" &&
        /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(normalized[field.name])
      ) {
        normalized[field.name] += ":00";
      }
    });
    return normalized;
  }

  function preparedUpsertValue(bundle, resource, value) {
    var binding = resourceDescriptor(bundle, resource);
    value = normalizeResourceValue(bundle, resource, value);
    if (
      !binding || !value || typeof value !== "object" || Array.isArray(value)
    ) return value;
    var key = binding.key || "id";
    if (value[key] !== undefined && value[key] !== null && value[key] !== "") {
      return value;
    }
    var prepared = Object.assign({}, value);
    prepared[key] = "new-" + Date.now().toString(36) + "-" +
      Math.floor(Math.random() * 1000000).toString(36);
    return prepared;
  }

  function upsertResource(bundle, resource, value) {
    var binding = resourceDescriptor(bundle, resource);
    if (!binding || !value || typeof value !== "object") {
      return Promise.reject(
        new Error("upsert requires a JSON object resource value"),
      );
    }
    var key = binding.key || "id";
    var id = value[key];
    var path = "/resources/" + encodeURIComponent(resource);
    var options = { method: "POST", body: JSON.stringify(value) };
    if (id === undefined || id === null || id === "") {
      value = Object.assign({}, value);
      value[key] = "new-" + Date.now().toString(36) + "-" +
        Math.floor(Math.random() * 1000000).toString(36);
      options.body = JSON.stringify(value);
    }
    if (id !== undefined && id !== null && id !== "") {
      path += "/items/" + encodeURIComponent(String(id));
      options.method = "PATCH";
      var patch = Object.assign({}, value);
      delete patch[key];
      options.body = JSON.stringify(patch);
    }
    return request(path, options);
  }

  function deleteResource(root, bundle, state, context, requestedResource) {
    var resource = requestedResource || (context && context.resource);
    if (!resource) {
      return Promise.reject(new Error("delete requires a resource"));
    }
    var binding = resourceDescriptor(bundle, resource);
    var record = context && context.record;
    if (!record && context && context.formValue) record = context.formValue;
    if (!record && context && context.formTarget) {
      record = formValueForTarget(bundle, state, context.formTarget);
    }
    if (!record) record = state.draft;
    var id = record && record[binding && (binding.key || "id")];
    if (!binding || id === undefined || id === null || id === "") {
      return Promise.reject(new Error("Select a record before deleting it"));
    }
    var confirmFn = globalThis.confirm ||
      (globalThis.window && globalThis.window.confirm);
    if (typeof confirmFn !== "function" || !confirmFn("Delete this record?")) {
      return Promise.resolve();
    }
    return request(
      "/resources/" + encodeURIComponent(resource) + "/items/" +
        encodeURIComponent(String(id)),
      { method: "DELETE" },
    ).then(function () {
      closeForms(root, bundle, context);
      showStatus(root, "Deleted", "success");
      return refreshResource(root, bundle, resource, state);
    });
  }

  function closeForms(root, bundle, context) {
    (bundle.bindings || []).filter(function (binding) {
      return binding.kind === "value" && binding.target;
    }).forEach(function (binding) {
      var form = targetFor(root, binding.target);
      if (context && context.formTarget && form !== context.formTarget) return;
      if (!form || form.tagName.toLowerCase() !== "ika-form") return;
      form.hidden = true;
      form.setAttribute("data-open", "false");
    });
  }

  function runAction(root, bundle, state, name, context) {
    var action = findAction(bundle, name);
    if (!action) {
      return Promise.reject(new Error("Unknown Egake action: " + name));
    }
    context = context || {};
    if (context.form) {
      context.formTarget = formTargetForAction(root, context);
      context.formValue = formValueForTarget(bundle, state, context.formTarget);
    }
    var inferredForm = formTargetForAction(root, context);
    var sourceForm = context.target && context.target.closest
      ? context.target.closest("ika-form")
      : null;
    var selectedRow = context.detail && context.detail.rowId !== undefined;
    var opensForm = inferredForm && sourceForm !== inferredForm &&
      (Boolean(context.form) || Boolean(context.record) || selectedRow);
    if (opensForm) {
      context.formTarget = inferredForm;
      context.formValue = formValueForTarget(bundle, state, inferredForm);
      setFormValues(root, bundle, state, context.record || {}, inferredForm);
      inferredForm.hidden = false;
      inferredForm.setAttribute("data-open", "true");
    }
    var chain = Promise.resolve();
    (action.steps || []).forEach(function (step) {
      chain = chain.then(function () {
        var attributes = step.attributes || {};
        if (step.kind === "refresh") {
          return refreshResource(root, bundle, attributes.resource, state);
        }
        if (step.kind === "upsert") {
          var value = actionInput(attributes.value, context, state);
          var requestValue = normalizeResourceValue(
            bundle,
            attributes.resource,
            value,
          );
          var validationValue = preparedUpsertValue(
            bundle,
            attributes.resource,
            value,
          );
          return validateActionTarget(
            bundle,
            state,
            context,
            attributes.value,
            attributes.resource,
            validationValue,
          ).then(function () {
            return upsertResource(
              bundle,
              attributes.resource,
              requestValue,
            );
          });
        }
        if (step.kind === "delete") {
          return deleteResource(
            root,
            bundle,
            state,
            context,
            attributes.resource,
          );
        }
        if (step.kind === "invoke") {
          return request(
            "/resources/" + encodeURIComponent(attributes.resource) +
              "/actions/" + encodeURIComponent(attributes.action),
            {
              method: "POST",
              body: JSON.stringify(
                actionInput(attributes.input, context, state),
              ),
            },
          );
        }
        if (step.kind === "validate") {
          return validateActionTarget(
            bundle,
            state,
            context,
            attributes.target,
          );
        }
        if (step.kind === "toast") {
          showStatus(root, step.text || "Done", "success");
          emit(root, "egake-toast", { message: step.text || "" });
        }
        return undefined;
      });
    });
    return chain.then(function (result) {
      if (
        (action.steps || []).some(function (step) {
          return step.kind === "upsert";
        })
      ) {
        closeForms(root, bundle, context);
        showStatus(root, "Saved", "success");
      }
      emit(root, "egake-action", { action: name, context: context });
      return result;
    });
  }

  function bindState(root, bundle, state, binding) {
    var target = targetFor(root, binding.target);
    if (!target || !binding.event) return;
    target.addEventListener(domEventName(binding.event), function () {
      if (binding.kind === "value" && binding.bind) {
        var name = stateName(binding.bind);
        if (name !== null && "value" in target) {
          state[name] = target.value;
          refreshAllResources(root, bundle, state);
        }
        var reference = formField(binding.bind);
        if (reference && "value" in target) {
          state.draft = Object.assign({}, state.draft || {});
          state.draft[reference.field] = target.value;
        }
      }
      if (binding.kind === "field" && binding.field && "value" in target) {
        var form = target.closest("ika-form");
        var formBinding = (bundle.bindings || []).find(function (candidate) {
          return form && candidate.target === form.id &&
            candidate.kind === "value" && candidate.bind;
        });
        var name = formBinding && stateName(formBinding.bind);
        if (name !== null) {
          state[name] = Object.assign({}, state[name] || {});
          state[name][binding.field] = target.value;
        } else {
          state.draft = Object.assign({}, state.draft || {});
          state.draft[binding.field] = target.value;
        }
      }
    });
  }

  function syncBoundControls(root, bundle, state) {
    (bundle.bindings || []).forEach(function (binding) {
      var target = targetFor(root, binding.target);
      if (!target || !("value" in target)) return;
      if (binding.kind === "value" && binding.bind) {
        var name = stateName(binding.bind);
        if (
          name !== null && state[name] !== null && state[name] !== undefined
        ) {
          target.value = state[name];
        }
        var reference = formField(binding.bind);
        if (
          reference && state.draft && state.draft[reference.field] !== undefined
        ) {
          target.value = state.draft[reference.field];
        }
      }
      if (binding.kind !== "field" || !binding.field) return;
      var form = target.closest("ika-form");
      var formBinding = (bundle.bindings || []).find(function (candidate) {
        return form && candidate.target === form.id &&
          candidate.kind === "value" && candidate.bind;
      });
      var formName = formBinding && stateName(formBinding.bind);
      var formValue = formName !== null ? state[formName] : state.draft;
      if (formValue && formValue[binding.field] !== undefined) {
        target.value = formValue[binding.field];
      }
    });
  }

  function configureSelects(root, bundle) {
    (bundle.bindings || []).filter(function (binding) {
      return binding.kind === "field" && binding.field;
    }).forEach(function (binding) {
      var target = targetFor(root, binding.target);
      if (!target || target.tagName.toLowerCase() !== "ika-select") return;
      var resource = (bundle.resources || []).find(function (candidate) {
        return candidate.fields && candidate.fields.some(function (field) {
          return field.name === binding.field;
        });
      });
      var field = resource && resource.fields.find(function (candidate) {
        return candidate.name === binding.field;
      });
      if (field && Array.isArray(field.enum)) target.options = field.enum;
    });
  }

  function configureFields(root, bundle) {
    (bundle.bindings || []).filter(function (binding) {
      return binding.kind === "field" && binding.field;
    }).forEach(function (binding) {
      var target = targetFor(root, binding.target);
      if (!target) return;
      var resource = (bundle.resources || []).find(function (candidate) {
        return candidate.fields && candidate.fields.some(function (field) {
          return field.name === binding.field;
        });
      });
      var field = resource && resource.fields.find(function (candidate) {
        return candidate.name === binding.field;
      });
      if (!field) return;
      if (field.required) target.setAttribute("required", "");
      else target.removeAttribute("required");
      if (target.tagName.toLowerCase() !== "ika-text-field") return;
      var type = field.format === "email"
        ? "email"
        : field.format === "date"
        ? "date"
        : field.format === "date-time"
        ? "datetime-local"
        : field.field_type === "number" || field.field_type === "integer"
        ? "number"
        : "text";
      target.setAttribute("type", type);
    });
  }

  function bindGrid(grid, bundle, binding, state) {
    grid.columns = (binding.columns || []).map(function (column) {
      return { id: column.id, label: column.label };
    });
    var resource = (bundle.resources || []).find(function (candidate) {
      return candidate.name === binding.resource;
    });
    var capabilities = resource &&
      (resource.capabilities || resource.required_capabilities || []);
    grid.editable = capabilities.indexOf("update") >= 0;
    if (capabilities.indexOf("list") < 0) {
      grid.error = "resource does not expose the list capability";
      return;
    }
    grid.addEventListener("ika-query", function (event) {
      var query = Object.assign({}, event.detail);
      if (!query.filter && state.query) query.filter = state.query;
      grid.__egakeQueryPromise = queryGrid(grid, binding, query);
    });
    if (grid.editable) {
      grid.addEventListener("ika-edit", function (event) {
        grid.__egakeEditPromise = editGrid(grid, bundle, binding, event.detail);
      });
    }
    grid.request({
      offset: 0,
      limit: 50,
      sort: [],
      ...(state.query ? { filter: state.query } : {}),
    });
  }

  function boot() {
    var application = document.getElementById("egake-application");
    var root = document.getElementById("egake-root");
    if (!root || !globalThis.IkasueRuntime) return;
    var inlineBundle = application && application.textContent;
    if (!inlineBundle) {
      fetch("app." + "bundle.json", { credentials: "same-origin" }).then(
        function (response) {
          if (!response.ok) {
            throw new Error("Egake application bundle could not be loaded");
          }
          return response.json();
        },
      ).then(function (bundle) {
        start(root, bundle);
      }).catch(function (error) {
        showStatus(root, error.message, "error");
      });
      return;
    }
    start(root, JSON.parse(inlineBundle));
  }

  function start(root, bundle) {
    var pages = bundle.views || [];
    if (!pages.length) return;
    var state = {};
    (bundle.states || []).forEach(function (entry) {
      state[entry.name] = entry.value;
    });
    pages.forEach(function (page) {
      var viewRoot = globalThis.IkasueRuntime.renderIkaView(root, page.view);
      viewRoot.setAttribute("aria-label", page.title);
      var heading = document.createElement("ika-text");
      heading.setAttribute("role", "heading");
      heading.setAttribute("aria-level", "1");
      heading.setAttribute("data-egake-page-title", "");
      heading.textContent = page.title;
      viewRoot.prepend(heading);
    });
    document.title = bundle.profile && bundle.profile.name ||
      "egake application";
    configureFields(root, bundle);
    configureSelects(root, bundle);
    syncBoundControls(root, bundle, state);
    (bundle.bindings || []).forEach(function (binding) {
      var target = targetFor(root, binding.target);
      if (!target) return;
      if (
        binding.kind === "resource" &&
        target.tagName.toLowerCase() === "ika-data-grid"
      ) {
        bindGrid(target, bundle, binding, state);
      } else if (binding.kind === "value" || binding.kind === "field") {
        bindState(root, bundle, state, binding);
      } else if (binding.kind === "action" && binding.action) {
        target.addEventListener(
          domEventName(binding.event || "action"),
          function (event) {
            var context = {
              target: target,
              detail: event.detail,
              resource: ((bundle.bindings || []).find(function (candidate) {
                return candidate.kind === "resource" &&
                  candidate.target === target.id;
              }) || {}).resource,
              record: selectedRecord(bundle, target, event.detail),
              form: binding.form,
            };
            runAction(root, bundle, state, binding.action, context).catch(
              function (error) {
                showStatus(root, error.message, "error", error.fields);
                emit(root, "egake-error", { message: error.message });
              },
            );
          },
        );
      }
    });
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else boot();
})();

// deno-lint-ignore-file no-var no-this-alias no-inner-declarations
(function () {
  "use strict";
  var ABI = "ikasue-web/1";
  var COMMON_PROPS = new Set([
    "id",
    "label",
    "variant",
    "align",
    "gap",
    "mode",
    "density",
    "editor",
    "role",
    "aria-level",
    "aria-label",
    "aria-labelledby",
    "data-open",
    "required",
    "type",
    "value",
  ]);
  function allowedProp(kind, name) {
    if (
      COMMON_PROPS.has(name) || name.indexOf("aria-") === 0 ||
      name.indexOf("data-") === 0
    ) return true;
    if (kind === "data-grid") {
      return ["columns", "rows", "total", "loading", "error", "editable"]
        .indexOf(name) >= 0;
    }
    return kind === "select" && name === "options";
  }
  function emit(node, type, detail) {
    node.dispatchEvent(
      new CustomEvent(type, {
        detail: detail,
        bubbles: true,
        composed: true,
        cancelable: true,
      }),
    );
  }
  function define(tag, ctor) {
    if (!customElements.get(tag)) customElements.define(tag, ctor);
  }
  class Surface extends HTMLElement {
    static get observedAttributes() {
      return ["density", "variant", "data-open", "mode"];
    }
    constructor() {
      super();
      this._previousActiveElement = null;
      this._backdrop = null;
      this._onKeyDown = function (event) {
        if (
          event.key !== "Escape" || this.getAttribute("data-open") !== "true"
        ) return;
        event.preventDefault();
        this.closeSurface(true);
      }.bind(this);
    }
    connectedCallback() {
      if (!this.getAttribute("role")) this.setAttribute("role", "group");
      if (this.localName !== "ika-form") return;
      var mode = this.getAttribute("mode") || "inline";
      if (mode === "inline") return;
      if (!this.hasAttribute("data-open")) {
        this.setAttribute("data-open", "false");
      }
      if (this.getAttribute("data-open") !== "true") this.hidden = true;
      this.setAttribute("role", "dialog");
      this.setAttribute("aria-modal", "true");
      if (
        !this.hasAttribute("aria-label") &&
        !this.hasAttribute("aria-labelledby")
      ) {
        this.setAttribute("aria-label", "Editor");
      }
      this.ensureCloseButton();
      if (this.getAttribute("data-open") === "true") this.openSurface();
    }
    attributeChangedCallback(name, oldValue, newValue) {
      if (
        !this.isConnected || this.localName !== "ika-form" ||
        oldValue === newValue
      ) return;
      if (name === "data-open" && newValue === "true") this.openSurface();
      if (name === "data-open" && newValue !== "true") this.closeSurface(false);
    }
    disconnectedCallback() {
      document.removeEventListener("keydown", this._onKeyDown);
      if (this._backdrop) this._backdrop.remove();
      this._backdrop = null;
    }
    ensureCloseButton() {
      if (this.querySelector("[data-ika-close]")) return;
      var close = document.createElement("button");
      close.type = "button";
      close.textContent = "×";
      close.setAttribute("aria-label", "Close");
      close.setAttribute("data-ika-close", "");
      var self = this;
      close.addEventListener("click", function () {
        self.closeSurface(true);
      });
      this.prepend(close);
    }
    openSurface() {
      if ((this.getAttribute("mode") || "inline") === "inline") return;
      if (!this._previousActiveElement && document.activeElement) {
        this._previousActiveElement = document.activeElement;
      }
      this.hidden = false;
      this.ensureBackdrop();
      document.addEventListener("keydown", this._onKeyDown);
      var self = this;
      globalThis.setTimeout(function () {
        var focusable = self.querySelector("input, select, textarea, button");
        if (focusable) focusable.focus();
      }, 0);
    }
    closeSurface(announce) {
      var wasOpen = this.getAttribute("data-open") === "true";
      this.hidden = true;
      if (wasOpen) this.setAttribute("data-open", "false");
      if (this._backdrop) this._backdrop.remove();
      this._backdrop = null;
      document.removeEventListener("keydown", this._onKeyDown);
      var opener = this._previousActiveElement;
      this._previousActiveElement = null;
      if (opener && opener.isConnected && opener.focus) opener.focus();
      if (announce && wasOpen) emit(this, "ika-close", {});
    }
    ensureBackdrop() {
      if (this._backdrop || !this.parentElement) return;
      var backdrop = document.createElement("div");
      backdrop.className = "ika-surface-backdrop";
      backdrop.setAttribute("aria-hidden", "true");
      var self = this;
      backdrop.addEventListener("click", function () {
        self.closeSurface(true);
      });
      this.parentElement.insertBefore(backdrop, this);
      this._backdrop = backdrop;
    }
  }
  class TextField extends HTMLElement {
    static get observedAttributes() {
      return ["required", "type", "value"];
    }
    connectedCallback() {
      if (this.firstElementChild) return;
      var multiline = this.getAttribute("editor") === "textarea";
      var input = document.createElement(multiline ? "textarea" : "input");
      if (!multiline) input.type = this.getAttribute("type") || "text";
      input.required = this.hasAttribute("required");
      input.value = this.getAttribute("value") || "";
      var labelText = this.getAttribute("label");
      if (labelText) input.setAttribute("aria-label", labelText);
      var self = this;
      input.addEventListener("input", function () {
        self.dispatchEvent(
          new Event("input", { bubbles: true, composed: true }),
        );
        self.dispatchEvent(
          new Event("change", { bubbles: true, composed: true }),
        );
      });
      if (labelText) {
        var label = document.createElement("label");
        label.textContent = labelText;
        label.appendChild(input);
        this.appendChild(label);
      } else this.appendChild(input);
    }
    attributeChangedCallback(name, _oldValue, newValue) {
      var input = this.querySelector("input, textarea");
      if (!input) return;
      if (name === "type" && input.tagName.toLowerCase() === "input") {
        input.type = newValue || "text";
      }
      if (name === "required") input.required = newValue !== null;
      if (name === "value" && document.activeElement !== input) {
        input.value = newValue || "";
      }
    }
    get value() {
      var input = this.querySelector("input, textarea");
      return input ? input.value : this.getAttribute("value") || "";
    }
    set value(value) {
      var input = this.querySelector("input, textarea");
      if (input) input.value = value == null ? "" : String(value);
      else this.setAttribute("value", value == null ? "" : String(value));
    }
  }
  class IconButton extends HTMLElement {
    connectedCallback() {
      if (this.firstElementChild) return;
      var button = document.createElement("button");
      button.type = "button";
      button.textContent = this.getAttribute("label") || this.textContent ||
        "Action";
      var self = this;
      button.addEventListener("click", function () {
        emit(self, "ika-action", { id: self.id });
      });
      this.replaceChildren(button);
    }
  }
  class Select extends HTMLElement {
    static get observedAttributes() {
      return ["required"];
    }
    constructor() {
      super();
      this._options = [];
    }
    connectedCallback() {
      this.render();
    }
    attributeChangedCallback(name) {
      if (name === "required") this.render();
    }
    set options(value) {
      this._options = value || [];
      this.render();
    }
    get options() {
      return this._options;
    }
    get value() {
      var select = this.querySelector("select");
      return select ? select.value : this.getAttribute("value") || "";
    }
    set value(value) {
      var select = this.querySelector("select");
      if (select) select.value = value == null ? "" : String(value);
      else this.setAttribute("value", value == null ? "" : String(value));
    }
    render() {
      if (!this.isConnected) return;
      var value = this.value,
        select = document.createElement("select");
      this._options.forEach(function (optionValue) {
        if (optionValue === null || typeof optionValue === "object") return;
        var option = document.createElement("option");
        option.value = String(optionValue);
        option.textContent = String(optionValue);
        select.appendChild(option);
      });
      select.value = value;
      select.required = this.hasAttribute("required");
      var self = this;
      select.addEventListener("change", function (event) {
        event.stopPropagation();
        self.dispatchEvent(
          new Event("change", { bubbles: true, composed: true }),
        );
      });
      var labelText = this.getAttribute("label");
      if (labelText) {
        select.setAttribute("aria-label", labelText);
        var label = document.createElement("label");
        label.textContent = labelText;
        label.appendChild(select);
        this.replaceChildren(label);
      } else this.replaceChildren(select);
    }
  }
  class DataGrid extends HTMLElement {
    constructor() {
      super();
      this._columns = [];
      this._rows = [];
      this._total = 0;
      this._loading = false;
      this._error = undefined;
      this._editable = true;
      this._offset = 0;
      this._limit = 50;
      this._rowHeight = 41;
      this._sort = [];
      this._filter = undefined;
      this._onScroll = function () {
        var limit = Math.max(
            1,
            Math.ceil((this.clientHeight || 480) / this._rowHeight) + 20,
          ),
          offset = Math.max(0, Math.floor(this.scrollTop / this._rowHeight));
        if (offset === this._offset && limit === this._limit) return;
        this.request({ offset: offset, limit: limit });
      };
    }
    set columns(value) {
      this._columns = value || [];
      this.render();
    }
    get columns() {
      return this._columns;
    }
    set rows(value) {
      this._rows = value || [];
      this.render();
    }
    get rows() {
      return this._rows;
    }
    set total(value) {
      this._total = Math.max(0, Number(value) || 0);
      this.render();
    }
    get total() {
      return this._total;
    }
    set loading(value) {
      this._loading = !!value;
      this.render();
    }
    get loading() {
      return this._loading;
    }
    set error(value) {
      this._error = value;
      this.render();
    }
    get error() {
      return this._error;
    }
    set editable(value) {
      this._editable = !!value;
    }
    get editable() {
      return this._editable;
    }
    request(query) {
      query = query || {};
      this._offset = Math.max(
        0,
        query.offset == null ? this._offset : query.offset,
      );
      this._limit = Math.max(
        1,
        query.limit == null ? this._limit : query.limit,
      );
      if (Object.prototype.hasOwnProperty.call(query, "sort")) {
        this._sort = query.sort || [];
      }
      if (Object.prototype.hasOwnProperty.call(query, "filter")) {
        this._filter = query.filter;
      }
      var detail = {
        offset: this._offset,
        limit: this._limit,
        sort: this._sort,
      };
      if (this._filter !== undefined) detail.filter = this._filter;
      emit(this, "ika-query", detail);
    }
    connectedCallback() {
      if (!this.getAttribute("role")) this.setAttribute("role", "grid");
      var configuredRowHeight = Number.parseFloat(
        getComputedStyle(this).getPropertyValue("--ika-data-grid-row-height"),
      );
      if (Number.isFinite(configuredRowHeight) && configuredRowHeight > 0) {
        this._rowHeight = configuredRowHeight;
      }
      this.addEventListener("scroll", this._onScroll, { passive: true });
      this.render();
    }
    disconnectedCallback() {
      this.removeEventListener("scroll", this._onScroll);
    }
    render() {
      if (!this.isConnected) return;
      var activeRow = document.activeElement &&
        document.activeElement.closest && document.activeElement.closest("tr");
      var focusedRowId = activeRow && this.contains(activeRow)
        ? activeRow.dataset.rowId
        : undefined;
      var scrollTop = this.scrollTop;
      var table = document.createElement("table"),
        head = table.createTHead().insertRow(),
        self = this;
      table.setAttribute("role", "grid");
      table.setAttribute("aria-rowcount", String(this._total));
      table.setAttribute("aria-busy", String(this._loading));
      this._columns.forEach(function (column) {
        var th = document.createElement("th");
        th.scope = "col";
        th.textContent = column.label;
        head.appendChild(th);
      });
      var body = table.createTBody();
      if (this._offset > 0) {
        var topSpacer = body.insertRow(),
          topCell = topSpacer.insertCell();
        topSpacer.setAttribute("aria-hidden", "true");
        topCell.colSpan = Math.max(1, this._columns.length);
        topCell.style.height = String(this._offset * this._rowHeight) + "px";
      }
      this._rows.forEach(function (row) {
        var tr = body.insertRow();
        tr.dataset.rowId = row.id;
        tr.tabIndex = 0;
        tr.setAttribute("role", "row");
        var selectionTimer;
        var selectRow = function () {
          emit(self, "ika-select", { rowId: row.id });
        };
        tr.addEventListener("click", function () {
          if (selectionTimer !== undefined) {
            globalThis.clearTimeout(selectionTimer);
          }
          selectionTimer = globalThis.setTimeout(selectRow, 350);
        });
        tr.addEventListener("keydown", function (event) {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            emit(self, "ika-select", { rowId: row.id });
            return;
          }
          if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
          event.preventDefault();
          var sibling = event.key === "ArrowDown"
            ? tr.nextElementSibling
            : tr.previousElementSibling;
          if (sibling && sibling.tabIndex >= 0) sibling.focus();
        });
        self._columns.forEach(function (column) {
          var cell = tr.insertCell();
          cell.setAttribute("role", "gridcell");
          cell.textContent = String(
            (row.cells || {})[column.id] == null
              ? ""
              : (row.cells || {})[column.id],
          );
          cell.addEventListener("dblclick", function () {
            if (selectionTimer !== undefined) {
              globalThis.clearTimeout(selectionTimer);
              selectionTimer = undefined;
            }
            if (!self._editable) return;
            var value = globalThis.prompt(
              column.label,
              cell.textContent || "",
            );
            if (value !== null) {
              emit(self, "ika-edit", {
                rowId: row.id,
                columnId: column.id,
                value: value,
              });
            }
          });
        });
      });
      if (this._total > this._offset + this._rows.length) {
        var bottomSpacer = body.insertRow(),
          bottomCell = bottomSpacer.insertCell();
        bottomSpacer.setAttribute("aria-hidden", "true");
        bottomCell.colSpan = Math.max(1, this._columns.length);
        bottomCell.style.height = String(
          Math.max(0, this._total - this._offset - this._rows.length) *
            this._rowHeight,
        ) + "px";
      }
      if (this._error || this._loading) {
        var caption = table.createCaption();
        caption.textContent = this._error || "Loading";
      } else if (!this._rows.length && this._total === 0) {
        var emptyCaption = table.createCaption();
        emptyCaption.textContent = "No records";
      }
      this.replaceChildren(table);
      this.scrollTop = scrollTop;
      if (focusedRowId) {
        var focusedRow = Array.from(table.querySelectorAll("tr")).find(
          function (row) {
            return row.dataset.rowId === focusedRowId;
          },
        );
        if (focusedRow) focusedRow.focus();
      }
    }
  }
  class Stack extends Surface {}
  class Flex extends Surface {}
  class Text extends Surface {}
  class Form extends Surface {}
  class Field extends Surface {}
  class ThemeRoot extends Surface {}
  define("ika-stack", Stack);
  define("ika-flex", Flex);
  define("ika-text", Text);
  define("ika-form", Form);
  define("ika-field", Field);
  define("ika-theme-root", ThemeRoot);
  define("ika-text-field", TextField);
  define("ika-select", Select);
  define("ika-icon-button", IconButton);
  define("ika-data-grid", DataGrid);
  function render(root, view) {
    if (!view || view.version !== ABI) {
      throw new TypeError("unsupported Ikasue view ABI");
    }
    var el = document.createElement(
      view.kind === "data-grid" ? "ika-data-grid" : "ika-" + view.kind,
    );
    Object.keys(view.props || {}).forEach(function (key) {
      if (!allowedProp(view.kind, key)) return;
      if (key === "id") el.id = view.props[key];
      else {
        if (
          typeof view.props[key] === "string" ||
          typeof view.props[key] === "number" ||
          typeof view.props[key] === "boolean"
        ) el.setAttribute(key, String(view.props[key]));
        el[key] = view.props[key];
      }
    });
    if (view.text !== undefined) el.textContent = view.text;
    (view.children || []).forEach(function (child) {
      render(el, child);
    });
    root.appendChild(el);
    return el;
  }
  globalThis.IkasueRuntime = { ABI_VERSION: ABI, renderIkaView: render };
})();

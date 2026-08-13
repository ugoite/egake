import {
  type IkaDataGridColumn,
  type IkaDataGridRow,
  type IkaEdit,
  type IkaQuery,
  type IkaSelect,
  type JsonValue,
} from "./contract.ts";

export const IKASUE_TAGS = [
  "ika-stack",
  "ika-flex",
  "ika-text",
  "ika-text-field",
  "ika-select",
  "ika-form",
  "ika-icon-button",
  "ika-data-grid",
  "ika-field",
  "ika-theme-root",
] as const;

function emit<K extends string>(
  element: HTMLElement,
  type: K,
  detail: object,
): void {
  element.dispatchEvent(
    new CustomEvent(type, {
      detail,
      bubbles: true,
      composed: true,
      cancelable: true,
    }),
  );
}

class IkaSurfaceElement extends HTMLElement {
  static observedAttributes = ["density", "variant", "data-open", "mode"];
  #previousActiveElement: HTMLElement | null = null;
  #backdrop?: HTMLDivElement;
  readonly #onKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== "Escape" || this.getAttribute("data-open") !== "true") {
      return;
    }
    event.preventDefault();
    this.closeSurface(true);
  };

  connectedCallback(): void {
    this.setAttribute("role", this.getAttribute("role") ?? "group");
    if (this.localName !== "ika-form") return;
    const mode = this.getAttribute("mode") ?? "inline";
    if (mode === "inline") return;
    if (!this.hasAttribute("data-open")) {
      this.setAttribute("data-open", "false");
    }
    if (this.getAttribute("data-open") !== "true") this.hidden = true;
    this.setAttribute("role", "dialog");
    this.setAttribute("aria-modal", "true");
    if (
      !this.hasAttribute("aria-label") && !this.hasAttribute("aria-labelledby")
    ) {
      this.setAttribute("aria-label", "Editor");
    }
    this.ensureCloseButton();
    if (this.getAttribute("data-open") === "true") this.openSurface();
  }

  attributeChangedCallback(
    name: string,
    oldValue: string | null,
    newValue: string | null,
  ): void {
    if (
      !this.isConnected || this.localName !== "ika-form" ||
      oldValue === newValue
    ) {
      return;
    }
    if (name === "data-open" && newValue === "true") this.openSurface();
    if (name === "data-open" && newValue !== "true") this.closeSurface(false);
  }

  disconnectedCallback(): void {
    document.removeEventListener("keydown", this.#onKeyDown);
    this.#backdrop?.remove();
    this.#backdrop = undefined;
  }

  private ensureCloseButton(): void {
    if (this.querySelector("[data-ika-close]")) return;
    const close = document.createElement("button");
    close.type = "button";
    close.textContent = "×";
    close.setAttribute("aria-label", "Close");
    close.setAttribute("data-ika-close", "");
    close.addEventListener("click", () => this.closeSurface(true));
    this.prepend(close);
  }

  private openSurface(): void {
    if ((this.getAttribute("mode") ?? "inline") === "inline") return;
    if (
      !this.#previousActiveElement &&
      document.activeElement instanceof HTMLElement
    ) {
      this.#previousActiveElement = document.activeElement;
    }
    this.hidden = false;
    this.ensureBackdrop();
    document.addEventListener("keydown", this.#onKeyDown);
    globalThis.setTimeout(() => {
      const focusable = this.querySelector<HTMLElement>(
        "input, select, textarea, button",
      );
      focusable?.focus();
    }, 0);
  }

  private closeSurface(announce: boolean): void {
    const wasOpen = this.getAttribute("data-open") === "true";
    this.hidden = true;
    if (wasOpen) this.setAttribute("data-open", "false");
    this.#backdrop?.remove();
    this.#backdrop = undefined;
    document.removeEventListener("keydown", this.#onKeyDown);
    const opener = this.#previousActiveElement;
    this.#previousActiveElement = null;
    if (opener && opener.isConnected) opener.focus();
    if (announce && wasOpen) emit(this, "ika-close", {});
  }

  private ensureBackdrop(): void {
    if (this.#backdrop || !this.parentElement) return;
    const backdrop = document.createElement("div");
    backdrop.className = "ika-surface-backdrop";
    backdrop.setAttribute("aria-hidden", "true");
    backdrop.addEventListener("click", () => this.closeSurface(true));
    this.parentElement.insertBefore(backdrop, this);
    this.#backdrop = backdrop;
  }
}

export class IkaTextFieldElement extends HTMLElement {
  static observedAttributes = ["required", "type", "value"];
  #input?: HTMLInputElement | HTMLTextAreaElement;
  connectedCallback(): void {
    if (this.#input) return;
    const multiline = this.getAttribute("editor") === "textarea";
    const input = multiline
      ? document.createElement("textarea")
      : document.createElement("input");
    if (input instanceof HTMLInputElement) {
      input.type = this.getAttribute("type") ?? "text";
    }
    input.required = this.hasAttribute("required");
    input.value = this.getAttribute("value") ?? "";
    const labelText = this.getAttribute("label");
    if (labelText) input.setAttribute("aria-label", labelText);
    input.addEventListener("input", () => {
      this.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
      this.dispatchEvent(
        new Event("change", { bubbles: true, composed: true }),
      );
    });
    this.#input = input;
    if (labelText) {
      const label = document.createElement("label");
      label.textContent = labelText;
      label.append(input);
      this.replaceChildren(label);
    } else {
      this.replaceChildren(input);
    }
  }
  attributeChangedCallback(
    name: string,
    _oldValue: string | null,
    newValue: string | null,
  ): void {
    if (!this.#input) return;
    if (name === "type" && this.#input instanceof HTMLInputElement) {
      this.#input.type = newValue ?? "text";
    }
    if (name === "required") this.#input.required = newValue !== null;
    if (name === "value" && document.activeElement !== this.#input) {
      this.#input.value = newValue ?? "";
    }
  }
  get value(): string {
    return this.#input?.value ?? "";
  }
  set value(value: string) {
    if (this.#input) this.#input.value = value;
    else this.setAttribute("value", value);
  }
}

export class IkaSelectElement extends HTMLElement {
  static observedAttributes = ["required"];
  #select?: HTMLSelectElement;
  #options: readonly JsonValue[] = [];
  connectedCallback(): void {
    this.render();
  }
  set options(value: readonly JsonValue[]) {
    this.#options = value;
    this.render();
  }
  get options(): readonly JsonValue[] {
    return this.#options;
  }
  attributeChangedCallback(
    name: string,
    _oldValue: string | null,
    _newValue: string | null,
  ): void {
    if (name === "required") this.render();
  }
  get value(): string {
    return this.#select?.value ?? this.getAttribute("value") ?? "";
  }
  set value(value: string) {
    if (this.#select) this.#select.value = value;
    else this.setAttribute("value", value);
  }
  render(): void {
    if (!this.isConnected) return;
    const value = this.value;
    const select = document.createElement("select");
    for (const optionValue of this.#options) {
      if (optionValue === null || typeof optionValue === "object") continue;
      const option = document.createElement("option");
      option.value = String(optionValue);
      option.textContent = String(optionValue);
      select.appendChild(option);
    }
    select.value = value;
    select.required = this.hasAttribute("required");
    select.addEventListener("change", (event) => {
      event.stopPropagation();
      this.dispatchEvent(
        new Event("change", { bubbles: true, composed: true }),
      );
    });
    const labelText = this.getAttribute("label");
    if (labelText) select.setAttribute("aria-label", labelText);
    this.#select = select;
    if (labelText) {
      const label = document.createElement("label");
      label.textContent = labelText;
      label.append(select);
      this.replaceChildren(label);
    } else {
      this.replaceChildren(select);
    }
  }
}

export class IkaIconButtonElement extends HTMLElement {
  connectedCallback(): void {
    if (this.firstElementChild) return;
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = this.getAttribute("label") ?? this.textContent ??
      "Action";
    button.addEventListener(
      "click",
      () => emit(this, "ika-action", { id: this.id }),
    );
    this.replaceChildren(button);
  }
}

/** Controlled DataGrid. It owns geometry and interaction, never data access. */
export class IkaDataGridElement extends HTMLElement {
  #columns: readonly IkaDataGridColumn[] = [];
  #rows: readonly IkaDataGridRow[] = [];
  #total = 0;
  #loading = false;
  #error?: string;
  #editable = true;
  #offset = 0;
  #limit = 50;
  #rowHeight = 41;
  #sort: IkaQuery["sort"] = [];
  #filter?: string;
  readonly #onScroll = (): void => {
    const limit = Math.max(
      1,
      Math.ceil((this.clientHeight || 480) / this.#rowHeight) + 20,
    );
    const offset = Math.max(0, Math.floor(this.scrollTop / this.#rowHeight));
    if (offset === this.#offset && limit === this.#limit) return;
    this.request({ offset, limit });
  };

  set columns(value: readonly IkaDataGridColumn[]) {
    this.#columns = value;
    this.render();
  }
  get columns(): readonly IkaDataGridColumn[] {
    return this.#columns;
  }
  set rows(value: readonly IkaDataGridRow[]) {
    this.#rows = value;
    this.render();
  }
  get rows(): readonly IkaDataGridRow[] {
    return this.#rows;
  }
  set total(value: number) {
    this.#total = Math.max(0, value);
    this.render();
  }
  get total(): number {
    return this.#total;
  }
  set loading(value: boolean) {
    this.#loading = value;
    this.render();
  }
  get loading(): boolean {
    return this.#loading;
  }
  set error(value: string | undefined) {
    this.#error = value;
    this.render();
  }
  get error(): string | undefined {
    return this.#error;
  }
  set editable(value: boolean) {
    this.#editable = value;
  }
  get editable(): boolean {
    return this.#editable;
  }

  connectedCallback(): void {
    this.setAttribute("role", this.getAttribute("role") ?? "grid");
    const configuredRowHeight = Number.parseFloat(
      getComputedStyle(this).getPropertyValue("--ika-data-grid-row-height"),
    );
    if (Number.isFinite(configuredRowHeight) && configuredRowHeight > 0) {
      this.#rowHeight = configuredRowHeight;
    }
    this.addEventListener("scroll", this.#onScroll, { passive: true });
    this.render();
  }

  disconnectedCallback(): void {
    this.removeEventListener("scroll", this.#onScroll);
  }

  request(query: Partial<IkaQuery> = {}): void {
    this.#offset = Math.max(0, query.offset ?? this.#offset);
    this.#limit = Math.max(1, query.limit ?? this.#limit);
    if (Object.prototype.hasOwnProperty.call(query, "sort")) {
      this.#sort = query.sort ?? [];
    }
    if (Object.prototype.hasOwnProperty.call(query, "filter")) {
      this.#filter = query.filter;
    }
    const detail = {
      offset: this.#offset,
      limit: this.#limit,
      sort: this.#sort,
      ...(this.#filter === undefined ? {} : { filter: this.#filter }),
    } satisfies IkaQuery;
    emit(
      this,
      "ika-query",
      detail,
    );
  }

  render(): void {
    if (!this.isConnected) return;
    const activeRow = document.activeElement?.closest("tr");
    const focusedRowId = activeRow && this.contains(activeRow)
      ? activeRow.dataset.rowId
      : undefined;
    const scrollTop = this.scrollTop;
    const table = document.createElement("table");
    table.setAttribute("role", "grid");
    table.setAttribute("aria-rowcount", String(this.#total));
    table.setAttribute("aria-busy", String(this.#loading));
    const head = table.createTHead().insertRow();
    for (const column of this.#columns) {
      const cell = document.createElement("th");
      cell.scope = "col";
      cell.textContent = column.label;
      head.appendChild(cell);
    }
    const body = table.createTBody();
    if (this.#offset > 0) {
      const spacer = body.insertRow();
      spacer.setAttribute("aria-hidden", "true");
      const cell = spacer.insertCell();
      cell.colSpan = Math.max(1, this.#columns.length);
      cell.style.height = String(this.#offset * this.#rowHeight) + "px";
    }
    for (const row of this.#rows) {
      const tr = body.insertRow();
      tr.dataset.rowId = row.id;
      tr.tabIndex = 0;
      tr.setAttribute("role", "row");
      let selectionTimer: number | undefined;
      const selectRow = (): void => {
        emit(this, "ika-select", { rowId: row.id } satisfies IkaSelect);
      };
      tr.addEventListener("click", () => {
        if (selectionTimer !== undefined) {
          globalThis.clearTimeout(selectionTimer);
        }
        selectionTimer = globalThis.setTimeout(selectRow, 350);
      });
      tr.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          emit(this, "ika-select", { rowId: row.id } satisfies IkaSelect);
          return;
        }
        if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
        event.preventDefault();
        const sibling = event.key === "ArrowDown"
          ? tr.nextElementSibling
          : tr.previousElementSibling;
        if (sibling instanceof HTMLTableRowElement && sibling.tabIndex >= 0) {
          sibling.focus();
        }
      });
      for (const column of this.#columns) {
        const cell = tr.insertCell();
        cell.setAttribute("role", "gridcell");
        cell.textContent = String(row.cells[column.id] ?? "");
        cell.addEventListener("dblclick", () => {
          if (selectionTimer !== undefined) {
            globalThis.clearTimeout(selectionTimer);
            selectionTimer = undefined;
          }
          if (!this.#editable) return;
          const value = globalThis.prompt(column.label, cell.textContent ?? "");
          if (value !== null) {
            emit(
              this,
              "ika-edit",
              { rowId: row.id, columnId: column.id, value } satisfies IkaEdit,
            );
          }
        });
      }
    }
    if (this.#total > this.#offset + this.#rows.length) {
      const spacer = body.insertRow();
      spacer.setAttribute("aria-hidden", "true");
      const cell = spacer.insertCell();
      cell.colSpan = Math.max(1, this.#columns.length);
      cell.style.height = String(
        Math.max(0, this.#total - this.#offset - this.#rows.length) *
          this.#rowHeight,
      ) + "px";
    }
    if (this.#error) {
      const caption = table.createCaption();
      caption.textContent = this.#error;
    } else if (this.#loading) {
      const caption = table.createCaption();
      caption.textContent = "Loading";
    } else if (!this.#rows.length && this.#total === 0) {
      const caption = table.createCaption();
      caption.textContent = "No records";
    }
    this.replaceChildren(table);
    this.scrollTop = scrollTop;
    if (focusedRowId) {
      const focusedRow = Array.from(table.querySelectorAll("tr")).find(
        (row) => row.dataset.rowId === focusedRowId,
      );
      if (focusedRow instanceof HTMLElement) focusedRow.focus();
    }
  }
}

export function defineIkaSue(
  registry: CustomElementRegistry = globalThis.customElements,
): void {
  const definitions: ReadonlyArray<
    readonly [string, CustomElementConstructor]
  > = [
    ["ika-stack", class IkaStackElement extends IkaSurfaceElement {}],
    ["ika-flex", class IkaFlexElement extends IkaSurfaceElement {}],
    ["ika-text", class IkaTextElement extends IkaSurfaceElement {}],
    ["ika-form", class IkaFormElement extends IkaSurfaceElement {}],
    ["ika-field", class IkaFieldElement extends IkaSurfaceElement {}],
    ["ika-theme-root", class IkaThemeRootElement extends IkaSurfaceElement {}],
    ["ika-text-field", IkaTextFieldElement],
    ["ika-select", IkaSelectElement],
    ["ika-icon-button", IkaIconButtonElement],
    ["ika-data-grid", IkaDataGridElement],
  ];
  for (const [tag, constructor] of definitions) {
    if (!registry.get(tag)) registry.define(tag, constructor);
  }
}

export type IkaDataGridValue = JsonValue;

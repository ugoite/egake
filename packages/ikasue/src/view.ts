import { IKASUE_ABI_VERSION, type IkaView } from "./contract.ts";
import { defineIkaSue } from "./elements.ts";

const KIND_ALIASES: Readonly<Record<string, string>> = {
  "data-grid": "ika-data-grid",
};

const COMMON_PROPS = new Set([
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

function allowedProp(kind: string, name: string): boolean {
  if (
    COMMON_PROPS.has(name) || name.startsWith("aria-") ||
    name.startsWith("data-")
  ) return true;
  if (kind === "data-grid") {
    return ["columns", "rows", "total", "loading", "error", "editable"]
      .includes(name);
  }
  return kind === "select" && name === "options";
}

function tagFor(kind: string): string {
  return KIND_ALIASES[kind] ?? `ika-${kind}`;
}

/** Lower a validated IkaView into the same Custom Elements used by the browser runtime. */
export function renderIkaView(
  root: Element,
  view: IkaView,
  registry?: CustomElementRegistry,
): Element {
  if (view.version !== IKASUE_ABI_VERSION) {
    throw new TypeError(`unsupported Ikasue view ABI: ${view.version}`);
  }
  const ownerDocument = root.ownerDocument;
  const ownerRegistry = registry ??
    ownerDocument.defaultView?.customElements ?? globalThis.customElements;
  defineIkaSue(ownerRegistry);
  const element = ownerDocument.createElement(tagFor(view.kind));
  for (const [name, value] of Object.entries(view.props ?? {})) {
    if (!allowedProp(view.kind, name)) continue;
    if (name === "id" && typeof value === "string") element.id = value;
    else {
      if (
        typeof value === "string" ||
        typeof value === "number" ||
        typeof value === "boolean"
      ) {
        element.setAttribute(name, String(value));
      }
      (element as unknown as Record<string, unknown>)[name] = value;
    }
  }
  if (view.text !== undefined) element.textContent = view.text;
  for (const child of view.children ?? []) {
    renderIkaView(element, child, ownerRegistry);
  }
  root.appendChild(element);
  return element;
}

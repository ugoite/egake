function assert(value: unknown, message = "assertion failed"): asserts value {
  if (!value) throw new Error(message);
}
function assertEquals(actual: unknown, expected: unknown): void {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}
function assertNotEquals(actual: unknown, expected: unknown): void {
  if (actual === expected) throw new Error(`did not expect ${String(actual)}`);
}

import type { ResourcePage, ResourceProvider } from "../src/types.ts";

type Listener = (
  event: { type: string; key?: string; preventDefault?: () => void },
) => void;

class FakeElement {
  readonly children: FakeElement[] = [];
  readonly attributes = new Map<string, string>();
  readonly dataset: Record<string, string> = {};
  readonly listeners = new Map<string, Listener[]>();
  parentNode: FakeElement | null = null;
  hidden = false;
  id = "";
  className = "";
  textContent = "";
  value = "";
  type = "";
  name = "";
  required = false;
  ownerDocument: FakeDocument;
  readonly classList = {
    add: (...names: string[]) => {
      this.className = [
        ...new Set(`${this.className} ${names.join(" ")}`.trim().split(/\s+/)),
      ].join(" ");
    },
  };

  constructor(readonly tagName: string, ownerDocument: FakeDocument) {
    this.ownerDocument = ownerDocument;
  }
  get parentElement(): FakeElement | null {
    return this.parentNode;
  }
  get firstChild(): FakeElement | null {
    return this.children[0] ?? null;
  }
  get rows(): FakeElement[] {
    return this.children.filter((child) => child.tagName === "TR");
  }
  get tBodies(): FakeElement[] {
    return this.children.filter((child) => child.tagName === "TBODY");
  }
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
    if (name === "id") this.id = value;
    if (name.startsWith("data-")) this.dataset[dataName(name)] = value;
  }
  getAttribute(name: string): string | null {
    if (name.startsWith("data-")) return this.dataset[dataName(name)] ?? null;
    return this.attributes.get(name) ?? null;
  }
  append(...nodes: (FakeElement | string)[]): void {
    nodes.forEach((node) => {
      if (typeof node !== "string") this.appendChild(node);
    });
  }
  appendChild(node: FakeElement): FakeElement {
    if (node instanceof FakeDocumentFragment) {
      [...node.children].forEach((child) => this.appendChild(child));
      return node;
    }
    if (node.parentNode) node.parentNode.removeChild(node);
    node.parentNode = this;
    this.children.push(node);
    return node;
  }
  removeChild(node: FakeElement): void {
    const index = this.children.indexOf(node);
    if (index >= 0) this.children.splice(index, 1);
    node.parentNode = null;
  }
  remove(): void {
    this.parentNode?.removeChild(this);
  }
  replaceChildren(...nodes: FakeElement[]): void {
    [...this.children].forEach((child) => this.removeChild(child));
    nodes.forEach((node) => this.appendChild(node));
  }
  addEventListener(type: string, listener: Listener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  dispatchEvent(
    event: { type: string; key?: string; preventDefault?: () => void },
  ): void {
    (this.listeners.get(event.type) ?? []).forEach((listener) =>
      listener(event)
    );
  }
  focus(): void {
    this.ownerDocument.activeElement = this;
  }
  contains(node: FakeElement | null): boolean {
    return node === this || this.children.some((child) => child.contains(node));
  }
  createTHead(): FakeElement {
    const head = this.ownerDocument.createElement("thead");
    this.append(head);
    return head;
  }
  createTBody(): FakeElement {
    const body = this.ownerDocument.createElement("tbody");
    this.append(body);
    return body;
  }
  insertRow(): FakeElement {
    const row = this.ownerDocument.createElement("tr");
    this.append(row);
    return row;
  }
  insertCell(): FakeElement {
    const cell = this.ownerDocument.createElement("td");
    this.append(cell);
    return cell;
  }
  querySelector(selector: string): FakeElement | null {
    return this.querySelectorAll(selector)[0] ?? null;
  }
  querySelectorAll(selector: string): FakeElement[] {
    const result: FakeElement[] = [];
    const visit = (node: FakeElement) =>
      node.children.forEach((child) => {
        if (matches(child, selector)) result.push(child);
        visit(child);
      });
    visit(this);
    return result;
  }
}

class FakeDocumentFragment extends FakeElement {
  constructor(ownerDocument: FakeDocument) {
    super("#fragment", ownerDocument);
  }
}
class FakeInput extends FakeElement {
  constructor(document: FakeDocument) {
    super("INPUT", document);
  }
}
class FakeTextarea extends FakeElement {
  constructor(document: FakeDocument) {
    super("TEXTAREA", document);
  }
}
class FakeSelect extends FakeElement {
  constructor(document: FakeDocument) {
    super("SELECT", document);
  }
}
class FakeButton extends FakeElement {
  constructor(document: FakeDocument) {
    super("BUTTON", document);
  }
}

class FakeDocument extends FakeElement {
  activeElement: FakeElement;
  constructor() {
    super("#document", null as unknown as FakeDocument);
    this.ownerDocument = this;
    this.activeElement = this;
  }
  createElement(tag: string): FakeElement {
    if (tag === "input") return new FakeInput(this);
    if (tag === "textarea") return new FakeTextarea(this);
    if (tag === "select") return new FakeSelect(this);
    if (tag === "button") return new FakeButton(this);
    return new FakeElement(tag.toUpperCase(), this);
  }
  createDocumentFragment(): FakeDocumentFragment {
    return new FakeDocumentFragment(this);
  }
  createTextNode(): FakeElement {
    return this.createElement("text");
  }
}

function dataName(name: string): string {
  return name.slice(5).replace(
    /-([a-z])/g,
    (_, letter) => letter.toUpperCase(),
  );
}
function matches(element: FakeElement, selector: string): boolean {
  return selector.split(",").some((part) => {
    const value = part.trim();
    const classMatch = value.match(/^\.([\w-]+)/);
    if (classMatch && !element.className.split(/\s+/).includes(classMatch[1])) {
      return false;
    }
    if (value.includes("[id]") && !element.id) return false;
    if (value.includes("[data-focus-key]") && !element.dataset.focusKey) {
      return false;
    }
    const role = value.match(/\[role=["']?([^\]"']+)/)?.[1];
    if (role && element.getAttribute("role") !== role) return false;
    const className = value.match(/\.([\w-]+)/)?.[1];
    if (className && !element.className.split(/\s+/).includes(className)) {
      return false;
    }
    return !value.match(/^[a-z]+/i) ||
      value.match(/^[a-z]+/i)?.[0].toUpperCase() === element.tagName;
  });
}

const page = {
  profile: { name: "render test", version: "0.1" as const },
  resources: [{
    name: "tasks",
    schema: "tasks.schema.json",
    capabilities: ["schema", "list"],
    fields: [],
  }],
  states: [],
  actions: [],
  pages: [{
    name: "home",
    title: "Tasks",
    components: [
      {
        kind: "button" as const,
        text: "Refresh",
        attributes: {},
        children: [],
        events: [],
      },
      {
        kind: "data-table" as const,
        attributes: { resource: "tasks" },
        children: [{
          kind: "column" as const,
          attributes: { field: "id" },
          children: [],
          events: [],
        }],
        events: [],
      },
      {
        kind: "form" as const,
        id: "editor",
        attributes: { mode: "dialog" },
        children: [{
          kind: "text-input" as const,
          id: "name",
          attributes: { field: "name" },
          children: [],
          events: [],
        }],
        events: [],
      },
    ],
  }],
};

(globalThis as Record<string, unknown>).HTMLInputElement = FakeInput;
(globalThis as Record<string, unknown>).HTMLTextAreaElement = FakeTextarea;
(globalThis as Record<string, unknown>).HTMLSelectElement = FakeSelect;
(globalThis as Record<string, unknown>).HTMLButtonElement = FakeButton;

const { mountApplication } = await import("../src/application.ts");

Deno.test("public renderer preserves table rows and focus across rerender", async () => {
  const document = new FakeDocument();
  const root = document.createElement("div");
  document.append(root);
  let resolve: ((page: ResourcePage) => void) | undefined;
  const provider: ResourceProvider = {
    schema: () => ({
      name: "tasks",
      fields: [],
      capabilities: ["schema", "list"] as const,
    }),
    list: () =>
      new Promise<ResourcePage>((done) => {
        resolve = done;
      }),
    get: () => Promise.resolve({}),
    create: () => Promise.resolve({}),
    update: () => Promise.resolve({}),
    delete: () => Promise.resolve(),
    invoke: () => Promise.resolve(null),
  };
  const controller = mountApplication(root as unknown as HTMLElement, page, {
    providers: { tasks: provider },
  });
  resolve?.({ items: [{ id: "a" }], total: 1, offset: 0, limit: 50 });
  await Promise.resolve();
  await Promise.resolve();
  const oldRow = root.querySelector("tbody")?.rows[0];
  const button = root.querySelector("button");
  button?.focus();
  controller.rerender();
  const currentRow = root.querySelector("tbody")?.rows[0];
  assert(oldRow);
  assertEquals(currentRow, oldRow);
  assertEquals(document.activeElement.dataset.focusKey, "page-0-0");
  resolve?.({ items: [{ id: "b" }], total: 1, offset: 0, limit: 50 });
  await Promise.resolve();
  await Promise.resolve();
  const dialog = root.querySelector("[role=dialog]");
  assert(dialog);
  assertEquals(dialog.getAttribute("aria-modal"), "true");
  assert(root.querySelector(".egake-backdrop"));
  document.dispatchEvent({
    type: "keydown",
    key: "Escape",
    preventDefault() {},
  });
  assertEquals(dialog.dataset.open, "false");
  assertNotEquals(document.activeElement, null);
});

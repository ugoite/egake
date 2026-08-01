import {
  JsonObject,
  parseApplication,
  ResourceClient,
  ResourceProvider,
  SerializedApplication,
  SerializedComponent,
} from "../../runtime/src/index.ts";

/** A node supplied by the host renderer or the browser DOM. */
export type SvelteNode = unknown;

/**
 * Small imperative boundary for Svelte applications.
 *
 * A Svelte host can implement these callbacks with `document.createElement`,
 * `document.createTextNode`, `append`, and `addEventListener`, or with a
 * renderer/test harness of its own. No Svelte compiler or runtime is bundled
 * into this package.
 */
export interface SvelteHost {
  createElement(type: string): SvelteNode;
  createText(value: string): SvelteNode;
  append(parent: SvelteNode, child: SvelteNode): void;
  clear(parent: SvelteNode): void;
  setAttribute(element: SvelteNode, name: string, value: string): void;
  listen(
    element: SvelteNode,
    event: string,
    listener: (event: unknown) => void,
  ): () => void;
}

/** A mounted host tree with the same lifecycle shape as a Svelte component. */
export interface SvelteMount {
  readonly application: SerializedApplication;
  update(serialized: unknown): void;
  destroy(): void;
}

/** Creates a provider facade without importing Svelte or adding a dependency. */
export function createSvelteResourceProvider<T extends JsonObject = JsonObject>(
  client: ResourceClient,
  name: string,
): ResourceProvider<T> {
  return client.resource<T>(name);
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function action(component: SerializedComponent): string | undefined {
  return text(component.attributes.action) ||
    component.events.find((event) => event.event === "click")?.action;
}

function renderNode(
  host: SvelteHost,
  component: SerializedComponent,
  onAction: SvelteRenderOptions["onAction"],
  cleanups: Array<() => void>,
): SvelteNode {
  const type = component.kind === "text-input"
    ? "input"
    : component.kind === "textarea"
    ? "textarea"
    : component.kind === "select"
    ? "select"
    : component.kind === "button"
    ? "button"
    : component.kind === "text"
    ? "span"
    : component.kind === "data-table"
    ? "table"
    : "div";
  const element = host.createElement(type);
  host.setAttribute(element, "class", `ikashita-${component.kind}`);
  if (component.id) host.setAttribute(element, "id", component.id);
  const label = text(component.attributes.label);
  const field = text(component.attributes.field);
  if (label) host.setAttribute(element, "aria-label", label);
  if (field && ["text-input", "textarea", "select"].includes(component.kind)) {
    host.setAttribute(element, "name", field);
  }

  if (component.kind === "text") {
    host.append(element, host.createText(component.text ?? label));
  } else if (component.kind === "button") {
    host.setAttribute(element, "type", "button");
    host.append(
      element,
      host.createText(component.text ?? (label || "Action")),
    );
    const actionName = action(component);
    if (actionName && onAction) {
      cleanups.push(host.listen(element, "click", (event) => {
        onAction(actionName, event, component);
      }));
    }
  } else if (component.kind === "select") {
    const values = Array.isArray(component.attributes.options)
      ? component.attributes.options.filter((value): value is string =>
        typeof value === "string"
      )
      : [];
    for (const value of values) {
      const option = host.createElement("option");
      host.setAttribute(option, "value", value);
      host.append(option, host.createText(value));
      host.append(element, option);
    }
  } else {
    for (const child of component.children) {
      host.append(element, renderNode(host, child, onAction, cleanups));
    }
  }
  return element;
}

function renderApplication(
  host: SvelteHost,
  application: SerializedApplication,
  options: SvelteRenderOptions,
  cleanups: Array<() => void>,
): SvelteNode {
  const root = host.createElement("div");
  host.setAttribute(root, "class", "ikashita-app");
  for (const page of application.pages) {
    const section = host.createElement("section");
    host.setAttribute(section, "class", "ikashita-page");
    host.setAttribute(section, "aria-label", page.title);
    const heading = host.createElement("h1");
    host.append(heading, host.createText(page.title));
    host.append(section, heading);
    for (const component of page.components) {
      host.append(
        section,
        renderNode(host, component, options.onAction, cleanups),
      );
    }
    host.append(root, section);
  }
  return root;
}

/** Options for `mountSvelteApplication`. */
export interface SvelteRenderOptions {
  readonly onAction?: (
    name: string,
    event: unknown,
    component: SerializedComponent,
  ) => void;
}

/**
 * Mounts Application Profile JSON through host callbacks.
 *
 * The returned lifecycle is intentionally close to a compiled Svelte
 * component (`update`/`destroy`), making this useful from a Svelte component
 * action or wrapper without compiling the profile into HTML.
 */
export function mountSvelteApplication(
  host: SvelteHost,
  target: SvelteNode,
  serialized: unknown,
  options: SvelteRenderOptions = {},
): SvelteMount {
  let application = parseApplication(serialized);
  const cleanups: Array<() => void> = [];
  const render = () => {
    for (const cleanup of cleanups.splice(0)) cleanup();
    host.clear(target);
    host.append(
      target,
      renderApplication(host, application, options, cleanups),
    );
  };
  render();
  return {
    get application() {
      return application;
    },
    update(next: unknown) {
      application = parseApplication(next);
      render();
    },
    destroy() {
      for (const cleanup of cleanups.splice(0)) cleanup();
      host.clear(target);
    },
  };
}

/** Creates a reusable Svelte-style mount function for one host. */
export function createSvelteRenderer(
  host: SvelteHost,
  options: SvelteRenderOptions = {},
): (target: SvelteNode, application: unknown) => SvelteMount {
  return (target, application) =>
    mountSvelteApplication(host, target, application, options);
}

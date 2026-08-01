import {
  JsonObject,
  parseApplication,
  ResourceClient,
  ResourceProvider,
  SerializedApplication,
  SerializedComponent,
} from "../../runtime/src/index.ts";

/** A host-owned Solid node. The adapter never assumes a DOM implementation. */
export type SolidNode = unknown;

/** The function shape accepted by Solid's `createComponent` primitive. */
export type SolidComponent<Props extends object = Record<string, unknown>> = (
  props: Props,
) => SolidNode;

/**
 * The small host boundary needed by this adapter.
 *
 * A browser host can pass Solid's `createElement`, `createComponent`, and
 * `insert` from `solid-js`, plus tiny DOM callbacks for attributes/events.
 * Keeping those callbacks here avoids a dependency on Solid's package while
 * still using Solid's normal ownership and child insertion primitives.
 */
export interface SolidHost {
  createElement(type: string): SolidNode;
  createComponent<Props extends object>(
    component: SolidComponent<Props>,
    props: Props,
  ): SolidNode;
  insert(parent: SolidNode, child: SolidNode | string): void;
  setAttribute(element: SolidNode, name: string, value: string): void;
  listen(
    element: SolidNode,
    event: string,
    listener: (event: unknown) => void,
  ): void;
}

/** Creates a provider facade without importing Solid or adding a dependency. */
export function createSolidResourceProvider<T extends JsonObject = JsonObject>(
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

function setCommonAttributes(
  host: SolidHost,
  element: SolidNode,
  component: SerializedComponent,
): void {
  host.setAttribute(element, "class", `ikashita-${component.kind}`);
  if (component.id) host.setAttribute(element, "id", component.id);
  const label = text(component.attributes.label);
  const field = text(component.attributes.field);
  if (label) host.setAttribute(element, "aria-label", label);
  if (field && ["text-input", "textarea", "select"].includes(component.kind)) {
    host.setAttribute(element, "name", field);
  }
}

function renderNode(
  host: SolidHost,
  component: SerializedComponent,
  onAction?: (
    name: string,
    event: unknown,
    component: SerializedComponent,
  ) => void,
): SolidNode {
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
  setCommonAttributes(host, element, component);

  if (component.kind === "text") {
    host.insert(element, component.text ?? text(component.attributes.label));
  } else if (component.kind === "button") {
    host.setAttribute(element, "type", "button");
    host.insert(
      element,
      component.text ?? (text(component.attributes.label) || "Action"),
    );
    const actionName = action(component);
    if (actionName && onAction) {
      host.listen(
        element,
        "click",
        (event) => onAction(actionName, event, component),
      );
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
      host.insert(option, value);
      host.insert(element, option);
    }
  } else {
    for (const child of component.children) {
      host.insert(element, renderNode(host, child, onAction));
    }
  }
  return element;
}

interface SolidApplicationProps {
  readonly application: SerializedApplication;
  readonly onAction?: (
    name: string,
    event: unknown,
    component: SerializedComponent,
  ) => void;
}

function createApplicationComponent(
  host: SolidHost,
): SolidComponent<SolidApplicationProps> {
  return ({ application, onAction }) => {
    const root = host.createElement("div");
    host.setAttribute(root, "class", "ikashita-app");
    for (const page of application.pages) {
      const section = host.createElement("section");
      host.setAttribute(section, "class", "ikashita-page");
      host.setAttribute(section, "aria-label", page.title);
      const heading = host.createElement("h1");
      host.insert(heading, page.title);
      host.insert(section, heading);
      for (const component of page.components) {
        host.insert(section, renderNode(host, component, onAction));
      }
      host.insert(root, section);
    }
    return root;
  };
}

/** Creates a Solid-compatible renderer for Application Profile JSON. */
export function createSolidRenderer(
  host: SolidHost,
  options: {
    readonly onAction?: (
      name: string,
      event: unknown,
      component: SerializedComponent,
    ) => void;
  } = {},
): (application: unknown) => SolidNode {
  const Application = createApplicationComponent(host);
  return (serialized: unknown) => {
    const application = parseApplication(serialized);
    return host.createComponent(Application, {
      application,
      onAction: options.onAction,
    });
  };
}

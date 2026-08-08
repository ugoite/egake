import {
  JsonObject,
  parseApplication,
  ResourceClient,
  ResourceProvider,
  SerializedApplication,
  SerializedComponent,
} from "../../runtime/src/index.ts";

/** The Vue render primitive used by this adapter; Vue remains a host dependency. */
export interface VueLike {
  h(
    type: string,
    props?: Readonly<Record<string, unknown>> | null,
    children?: unknown,
  ): unknown;
}

/** Creates a provider facade without importing Vue or adding a Vue dependency. */
export function createVueResourceProvider<T extends JsonObject = JsonObject>(
  client: ResourceClient,
  name: string,
): ResourceProvider<T> {
  return client.resource<T>(name);
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function renderNode(
  vue: VueLike,
  component: SerializedComponent,
  onAction?: (
    name: string,
    event: unknown,
    component: SerializedComponent,
  ) => void,
): unknown {
  const props: Record<string, unknown> = {
    class: `egake-${component.kind}`,
  };
  if (component.id) props.id = component.id;
  const label = text(component.attributes.label);
  const field = text(component.attributes.field);
  if (label) props["aria-label"] = label;
  if (field && ["text-input", "textarea", "select"].includes(component.kind)) {
    props.name = field;
  }
  const action = text(component.attributes.action) ||
    component.events.find((event) => event.event === "click")?.action;
  if (component.kind === "button" && action && onAction) {
    props.onClick = (event: unknown) => onAction(action, event, component);
  }
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
  if (component.kind === "text") {
    return vue.h(type, props, component.text ?? label ?? "");
  }
  if (component.kind === "button") {
    return vue.h(type, props, component.text ?? label ?? "Action");
  }
  if (component.kind === "select") {
    const children = Array.isArray(component.attributes.options)
      ? component.attributes.options.filter((value): value is string =>
        typeof value === "string"
      ).map((value) => vue.h("option", { value }, value))
      : [];
    return vue.h(type, props, children);
  }
  return vue.h(
    type,
    props,
    component.children.map((child) => renderNode(vue, child, onAction)),
  );
}

/** Creates a pure Vue VNode renderer for serialized Application Profile JSON. */
export function createVueRenderer(
  vue: VueLike,
  options: {
    readonly onAction?: (
      name: string,
      event: unknown,
      component: SerializedComponent,
    ) => void;
  } = {},
): (application: unknown) => unknown {
  return (serialized: unknown) => {
    const application: SerializedApplication = parseApplication(serialized);
    return vue.h(
      "div",
      { class: "egake-app" },
      application.pages.map((page) =>
        vue.h("section", {
          key: page.name,
          class: "egake-page",
          "aria-label": page.title,
        }, [
          vue.h("h1", null, page.title),
          ...page.components.map((component) =>
            renderNode(vue, component, options.onAction)
          ),
        ])
      ),
    );
  };
}

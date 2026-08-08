import {
  JsonObject,
  parseApplication,
  ResourceClient,
  ResourceProvider,
  SerializedApplication,
  SerializedComponent,
} from "../../runtime/src/index.ts";

/** The small part of React used by this adapter; React is a host dependency. */
export interface ReactLike {
  createElement(
    type: string,
    props?: Readonly<Record<string, unknown>> | null,
    ...children: readonly unknown[]
  ): unknown;
}

/** Creates a provider facade without importing React or adding a React dependency. */
export function createReactResourceProvider<T extends JsonObject = JsonObject>(
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
  react: ReactLike,
  component: SerializedComponent,
  onAction?: (
    name: string,
    event: unknown,
    component: SerializedComponent,
  ) => void,
): unknown {
  const props: Record<string, unknown> = {
    className: `egake-${component.kind}`,
  };
  if (component.id) props.id = component.id;
  const label = text(component.attributes.label);
  const field = text(component.attributes.field);
  if (label) props["aria-label"] = label;
  if (field && ["text-input", "textarea", "select"].includes(component.kind)) {
    props.name = field;
  }
  const actionName = action(component);
  if (component.kind === "button" && actionName && onAction) {
    props.type = "button";
    props.onClick = (event: unknown) => onAction(actionName, event, component);
  }
  if (component.kind === "text") {
    return react.createElement("span", props, component.text ?? label ?? "");
  }
  if (component.kind === "text-input") {
    return react.createElement("input", props);
  }
  if (component.kind === "textarea") {
    return react.createElement("textarea", props);
  }
  if (component.kind === "select") {
    const options = Array.isArray(component.attributes.options)
      ? component.attributes.options.filter((value): value is string =>
        typeof value === "string"
      ).map((value) =>
        react.createElement("option", { key: value, value }, value)
      )
      : [];
    return react.createElement("select", props, ...options);
  }
  if (component.kind === "button") {
    return react.createElement(
      "button",
      props,
      component.text ?? label ?? "Action",
    );
  }
  const children = component.children.map((child) =>
    renderNode(react, child, onAction)
  );
  return react.createElement(
    component.kind === "data-table"
      ? "table"
      : component.kind === "column"
      ? "div"
      : "div",
    props,
    ...children,
  );
}

/** Creates a pure React-element renderer for serialized Application Profile JSON. */
export function createReactRenderer(
  react: ReactLike,
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
    const pages = application.pages.map((page) =>
      react.createElement(
        "section",
        {
          key: page.name,
          className: "egake-page",
          "aria-label": page.title,
        },
        react.createElement("h1", null, page.title),
        ...page.components.map((component) =>
          renderNode(react, component, options.onAction)
        ),
      )
    );
    return react.createElement("div", { className: "egake-app" }, ...pages);
  };
}

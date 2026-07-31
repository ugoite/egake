import { isJsonObject, isJsonValue } from "./merge-patch.ts";
import {
  Capability,
  FieldSchema,
  JsonObject,
  JsonValue,
  MaybePromise,
  RenderOptions,
  ResourceError,
  ResourceProvider,
  RuntimeController,
  SerializedAction,
  SerializedApplication,
  SerializedComponent,
  SerializedEvent,
  SerializedPage,
  SerializedResource,
  SerializedState,
} from "./types.ts";

const COMPONENT_KINDS = new Set<SerializedComponent["kind"]>([
  "column",
  "row",
  "text",
  "text-input",
  "select",
  "textarea",
  "button",
  "data-table",
  "form",
]);
const CAPABILITY_SET = new Set<Capability>([
  "schema",
  "list",
  "get",
  "create",
  "update",
  "delete",
  "invoke",
]);
const COMPONENT_ATTRIBUTES = new Set([
  "label",
  "field",
  "bind",
  "action",
  "resource",
  "key",
  "mode",
  "variant",
  "align",
  "gap",
  "options",
]);

function object(value: unknown, label: string): JsonObject {
  if (!isJsonObject(value)) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must be a JSON object`,
    });
  }
  return value;
}

function string(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must be a string`,
    });
  }
  return value;
}

function array(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must be an array`,
    });
  }
  return value;
}

function stringAttribute(
  attributes: Readonly<Record<string, JsonValue>>,
  name: string,
): string | undefined {
  const value = attributes[name];
  return value === undefined
    ? undefined
    : string(value, `component attribute ${name}`);
}

function parseComponent(value: unknown): SerializedComponent {
  const source = object(value, "component");
  const kind = string(
    source.kind,
    "component kind",
  ) as SerializedComponent["kind"];
  if (!COMPONENT_KINDS.has(kind)) {
    throw new ResourceError({
      code: "validation_failed",
      message: `unknown component kind: ${kind}`,
    });
  }
  const rawAttributes = source.attributes === undefined
    ? {}
    : object(source.attributes, "component attributes");
  const attributes: JsonObject = Object.create(null) as JsonObject;
  for (const [name, attribute] of Object.entries(rawAttributes)) {
    if (!COMPONENT_ATTRIBUTES.has(name) || !isJsonValue(attribute)) {
      throw new ResourceError({
        code: "validation_failed",
        message: `unsupported component attribute: ${name}`,
      });
    }
    attributes[name] = attribute;
  }
  const children =
    (source.children === undefined
      ? []
      : array(source.children, "component children")).map(parseComponent);
  const events =
    (source.events === undefined
      ? []
      : array(source.events, "component events")).map(parseEvent);
  const id = source.id === undefined
    ? undefined
    : string(source.id, "component id");
  const text = source.text === undefined
    ? undefined
    : string(source.text, "component text");
  return {
    kind,
    attributes,
    children,
    events,
    ...(id === undefined ? {} : { id }),
    ...(text === undefined ? {} : { text }),
  };
}

function parseEvent(value: unknown): SerializedEvent {
  const source = object(value, "component event");
  return {
    event: string(source.event, "event name"),
    action: string(source.action, "event action"),
  };
}

function parseField(value: unknown): FieldSchema {
  const source = object(value, "resource field");
  const fieldType = string(source.field_type, "resource field type") as FieldSchema["field_type"];
  if (!new Set(["text", "number", "integer", "boolean", "date", "json"]).has(fieldType)) {
    throw new ResourceError({
      code: "validation_failed",
      message: `unknown resource field type: ${fieldType}`,
    });
  }
  if (typeof source.required !== "boolean") {
    throw new ResourceError({
      code: "validation_failed",
      message: "resource field required must be a boolean",
    });
  }
  const enumValues = source.enum === undefined
    ? undefined
    : array(source.enum, "resource field enum");
  const format = source.format === undefined
    ? undefined
    : string(source.format, "resource field format");
  return {
    name: string(source.name, "resource field name"),
    field_type: fieldType,
    required: source.required,
    ...(enumValues === undefined ? {} : { enum: enumValues as JsonValue[] }),
    ...(format === undefined ? {} : { format }),
  };
}

function parseResource(value: unknown): SerializedResource {
  const source = object(value, "resource declaration");
  const capabilities = array(
    source.required_capabilities ?? source.capabilities ?? [],
    "required_capabilities",
  ).map((item) => {
    const capability = string(item, "resource capability") as Capability;
    if (!CAPABILITY_SET.has(capability)) {
      throw new ResourceError({
        code: "validation_failed",
        message: `unknown resource capability: ${capability}`,
      });
    }
    return capability;
  });
  const fields = source.fields === undefined
    ? undefined
    : array(source.fields, "resource fields").map(parseField);
  return {
    name: string(source.name, "resource name"),
    schema: string(source.schema, "resource schema"),
    required_capabilities: capabilities,
    ...(fields === undefined ? {} : { fields }),
  };
}

function parseState(value: unknown): SerializedState {
  const source = object(value, "state declaration");
  if (!isJsonValue(source.value)) {
    throw new ResourceError({
      code: "validation_failed",
      message: "state value must be JSON",
    });
  }
  return { name: string(source.name, "state name"), value: source.value };
}

function parsePage(value: unknown): SerializedPage {
  const source = object(value, "page declaration");
  return {
    name: string(source.name, "page name"),
    title: string(source.title, "page title"),
    components: array(source.components ?? [], "page components").map(
      parseComponent,
    ),
  };
}

function parseAction(value: unknown): SerializedAction {
  const source = object(value, "action declaration");
  const steps = array(source.steps ?? [], "action steps").map((step) =>
    object(step, "action step")
  );
  return { name: string(source.name, "action name"), steps };
}

/** Validates and normalizes serialized Application Profile v0.1 JSON. */
export function parseApplication(value: unknown): SerializedApplication {
  const source = object(value, "application");
  const profile = object(source.profile, "application profile");
  const version = string(profile.version, "profile version");
  if (version !== "0.1") {
    throw new ResourceError({
      code: "validation_failed",
      message: `unsupported application profile version: ${version}`,
    });
  }
  return {
    profile: { name: string(profile.name, "application name"), version: "0.1" },
    resources: array(source.resources ?? [], "resources").map(parseResource),
    states: array(source.states ?? [], "states").map(parseState),
    pages: array(source.pages ?? [], "pages").map(parsePage),
    actions: array(source.actions ?? [], "actions").map(parseAction),
  };
}

function textValue(value: JsonValue | undefined): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function attr(
  component: SerializedComponent,
  name: string,
): string | undefined {
  return textAttribute(component.attributes, name);
}

function textAttribute(
  attributes: Readonly<Record<string, JsonValue>>,
  name: string,
): string | undefined {
  return textValue(attributes[name]);
}

function fixedClass(kind: SerializedComponent["kind"]): string {
  return `ikashita-${kind.replaceAll("-", "-")}`;
}

/** Maps schema metadata to the native HTML input type used by the renderer. */
export function inputTypeForField(field: FieldSchema | undefined):
  | "text"
  | "email"
  | "date"
  | "datetime-local" {
  if (field?.format === "email") return "email";
  if (field?.format === "date") return "date";
  if (field?.format === "date-time") return "datetime-local";
  return "text";
}

function fieldForComponent(
  application: SerializedApplication,
  component: SerializedComponent,
): FieldSchema | undefined {
  const field = attr(component, "field");
  if (!field) return undefined;
  return application.resources[0]?.fields?.find((candidate) => candidate.name === field);
}

function actionFor(component: SerializedComponent): string | undefined {
  return attr(component, "action") ??
    component.events.find((event) => event.event === "click")?.action;
}

function appendChildren(
  parent: Element,
  children: readonly SerializedComponent[],
  document: Document,
  options: RenderOptions,
  application: SerializedApplication,
): void {
  for (const child of children) {
    parent.append(renderComponent(child, document, options, application));
  }
}

function safeText(value: unknown): string {
  return typeof value === "string"
    ? value
    : value === undefined
    ? ""
    : String(value);
}

function renderComponent(
  component: SerializedComponent,
  document: Document,
  options: RenderOptions,
  application: SerializedApplication,
): Element {
  const element = document.createElement(
    component.kind === "text-input"
      ? "input"
      : component.kind === "text"
      ? "span"
      : component.kind === "data-table"
      ? "table"
      : component.kind === "button"
      ? "button"
      : component.kind === "select"
      ? "select"
      : component.kind === "textarea"
      ? "textarea"
      : "div",
  );
  element.className = fixedClass(component.kind);
  if (component.id) element.id = component.id;
  const label = attr(component, "label");
  const field = attr(component, "field");
  if (label) element.setAttribute("aria-label", label);
  if (
    field &&
    (element instanceof HTMLInputElement ||
      element instanceof HTMLTextAreaElement ||
      element instanceof HTMLSelectElement)
  ) element.name = field;
  const schemaField = fieldForComponent(application, component);
  if (
    schemaField?.required &&
    (element instanceof HTMLInputElement ||
      element instanceof HTMLTextAreaElement ||
      element instanceof HTMLSelectElement)
  ) element.required = true;

  if (component.kind === "text") {
    element.textContent = component.text ?? label ?? "";
  } else if (component.kind === "button") {
    const button = element as HTMLButtonElement;
    button.type = "button";
    button.textContent = component.text ?? label ?? "Action";
    const action = actionFor(component);
    if (action && options.onAction) {
      button.addEventListener(
        "click",
        (event) =>
          void options.onAction?.(action, event, { application, component }),
      );
    }
  } else if (component.kind === "select") {
    const select = element as HTMLSelectElement;
    const values = schemaField?.enum ?? component.attributes.options;
    if (Array.isArray(values)) {
      for (const value of values) {
        if (!isJsonValue(value) || value === null || typeof value === "object") continue;
        const option = document.createElement("option");
        option.value = String(value);
        option.textContent = String(value);
        select.append(option);
      }
    }
  } else if (component.kind === "text-input") {
    const input = element as HTMLInputElement;
    input.type = inputTypeForField(schemaField);
  } else if (component.kind === "data-table") {
    const table = element as HTMLTableElement;
    const head = table.createTHead().insertRow();
    const body = table.createTBody();
    const columns = component.children.filter((child) =>
      child.kind === "column"
    );
    for (const column of columns) {
      const cell = document.createElement("th");
      cell.scope = "col";
      cell.textContent = attr(column, "label") ?? attr(column, "field") ??
        column.text ?? "";
      head.append(cell);
    }
    const providerName = attr(component, "resource");
    const provider = providerName
      ? options.providers?.[providerName]
      : undefined;
    if (provider && columns.length) void hydrateTable(body, columns, provider);
  } else {
    appendChildren(element, component.children, document, options, application);
  }
  return element;
}

async function hydrateTable(
  body: HTMLTableSectionElement,
  columns: readonly SerializedComponent[],
  provider: ResourceProvider,
): Promise<void> {
  try {
    const page = await provider.list({ sort: [], offset: 0, limit: 50 });
    for (const item of page.items) {
      const row = body.insertRow();
      for (const column of columns) {
        const cell = row.insertCell();
        const field = attr(column, "field");
        cell.textContent = field ? safeText(item[field]) : "";
      }
    }
  } catch {
    // Provider details stay in the provider boundary; the renderer exposes no logs or raw errors.
  }
}

/** Renders an application into a new DocumentFragment using safe DOM APIs only. */
export function renderApplication(
  document: Document,
  serialized: unknown,
  options: RenderOptions = {},
): DocumentFragment {
  const application = parseApplication(serialized);
  const fragment = document.createDocumentFragment();
  for (const page of application.pages) {
    const section = document.createElement("section");
    section.className = "ikashita-page";
    section.setAttribute("aria-label", page.title);
    const heading = document.createElement("h1");
    heading.textContent = page.title;
    section.append(heading);
    appendChildren(section, page.components, document, options, application);
    fragment.append(section);
  }
  return fragment;
}

/** Mounts serialized application JSON into a browser element and returns a handle. */
export function mountApplication(
  root: HTMLElement,
  serialized: unknown,
  options: RenderOptions = {},
): RuntimeController {
  const application = parseApplication(serialized);
  const render = () => {
    const document = root.ownerDocument;
    if (!document) {
      throw new ResourceError({
        code: "internal",
        message: "mount root has no owner document",
      });
    }
    root.replaceChildren(renderApplication(document, application, options));
  };
  render();
  return {
    application,
    rerender: render,
    unmount: () => root.replaceChildren(),
  };
}

/** JSON values accepted at the host/provider boundary. */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | JsonObject;

/** A JSON object. Objects are deliberately kept data-only at this boundary. */
export type JsonObject = { [key: string]: JsonValue };

/** A value that may be returned synchronously by an embedded provider. */
export type MaybePromise<T> = T | PromiseLike<T>;

/** Operations that a resource may advertise. */
export const CAPABILITIES = [
  "schema",
  "list",
  "get",
  "create",
  "update",
  "delete",
  "invoke",
] as const;

/** An operation that a resource may advertise. */
export type Capability = (typeof CAPABILITIES)[number];

/** Stable provider error codes from the Resource Contract. */
export type ResourceErrorCode =
  | "validation_failed"
  | "not_found"
  | "conflict"
  | "capability_denied"
  | "unavailable"
  | "internal"
  | (string & Record<never, never>);

/** A structured provider error in its JSON wire representation. */
export interface StructuredError {
  readonly code: ResourceErrorCode;
  readonly message: string;
  readonly fields: Readonly<Record<string, string>>;
  readonly request_id?: string;
}

/** One schema field. `field_type` follows the Rust/JSON provider spelling. */
export interface FieldSchema {
  readonly name: string;
  readonly field_type:
    | "text"
    | "number"
    | "integer"
    | "boolean"
    | "date"
    | "json";
  readonly required: boolean;
  /** JSON Schema enum values, when the field is constrained to a set. */
  readonly enum?: readonly JsonValue[];
  /** Supported JSON Schema format metadata. */
  readonly format?:
    | "email"
    | "date"
    | "date-time"
    | (string & Record<never, never>);
}

/** Schema and capabilities advertised by a resource provider. */
export interface ResourceSchema {
  readonly name: string;
  readonly fields: readonly FieldSchema[];
  readonly capabilities: readonly Capability[];
}

/** One ordered list sort key. */
export interface Sort {
  readonly field: string;
  readonly direction: "asc" | "desc";
}

/** Normalized list query used by providers. */
export interface ListQuery {
  readonly q?: string;
  readonly sort: readonly Sort[];
  readonly offset: number;
  readonly limit: number;
}

/** A paginated provider response. */
export interface ResourcePage<T extends JsonObject = JsonObject> {
  readonly items: readonly T[];
  readonly total: number;
  readonly offset: number;
  readonly limit: number;
}

/** The host-side ResourceProvider contract. */
export interface ResourceProvider<T extends JsonObject = JsonObject> {
  schema(): MaybePromise<ResourceSchema>;
  list(query: ListQuery): MaybePromise<ResourcePage<T>>;
  get(id: string): MaybePromise<T>;
  create(value: T): MaybePromise<T>;
  update(id: string, mergePatch: JsonObject): MaybePromise<T>;
  delete(id: string): MaybePromise<void>;
  invoke(action: string, input: JsonValue): MaybePromise<JsonValue>;
}

/** Options for a same-origin HTTP client. */
export interface ResourceClientOptions {
  /** Relative API root; defaults to `/api/egake/v1`. */
  readonly basePath?: string;
  /** Fetch implementation, useful for deterministic tests and embedded hosts. */
  readonly fetch?: typeof globalThis.fetch;
  /** Optional browser origin used to validate resolved URLs. */
  readonly origin?: string;
  /** A trusted host-generated request ID, or a factory for one. */
  readonly requestId?: string | (() => string);
}

/** Options for the DOM renderer. */
export interface RenderOptions {
  readonly providers?: Readonly<Record<string, ResourceProvider>>;
  readonly onAction?: (
    action: string,
    event: Event,
    context: {
      readonly application: SerializedApplication;
      readonly component: SerializedComponent;
    },
  ) => MaybePromise<void>;
}

/** The serialized application shape consumed by the small runtime renderer. */
export interface SerializedApplication {
  readonly profile: { readonly name: string; readonly version: "0.1" };
  readonly resources: readonly SerializedResource[];
  readonly states: readonly SerializedState[];
  readonly pages: readonly SerializedPage[];
  readonly actions: readonly SerializedAction[];
}

/** A resource declaration in serialized application JSON. */
export interface SerializedResource {
  readonly name: string;
  readonly schema: string;
  readonly required_capabilities: readonly Capability[];
  /** Legacy CLI bundle spelling accepted during the additive transition. */
  readonly capabilities?: readonly Capability[];
  /** Schema metadata embedded by the CLI bundle, when available. */
  readonly fields?: readonly FieldSchema[];
}

/** A named initial state value. */
export interface SerializedState {
  readonly name: string;
  readonly value: JsonValue;
}

/** A page in serialized application JSON. */
export interface SerializedPage {
  readonly name: string;
  readonly title: string;
  readonly components: readonly SerializedComponent[];
}

/** A closed, safe component node from Application Profile v0.1. */
export interface SerializedComponent {
  readonly kind:
    | "column"
    | "row"
    | "text"
    | "text-input"
    | "select"
    | "textarea"
    | "button"
    | "data-table"
    | "form";
  readonly id?: string;
  readonly text?: string;
  readonly attributes: Readonly<Record<string, JsonValue>>;
  readonly children: readonly SerializedComponent[];
  readonly events: readonly SerializedEvent[];
}

/** An event/action pair attached to a component. */
export interface SerializedEvent {
  readonly event: string;
  readonly action: string;
}

/** A declared application action. */
export interface SerializedAction {
  readonly name: string;
  readonly steps: readonly JsonObject[];
}

/** A mounted application handle. */
export interface RuntimeController {
  readonly application: SerializedApplication;
  rerender(): void;
  unmount(): void;
}

/** Error raised by a provider or a host adapter. */
export class ResourceError extends Error {
  readonly code: ResourceErrorCode;
  readonly fields: Readonly<Record<string, string>>;
  readonly requestId?: string;
  readonly status?: number;

  constructor(
    error:
      & Pick<StructuredError, "code" | "message">
      & Partial<Pick<StructuredError, "fields" | "request_id">>,
    options: { readonly status?: number; readonly cause?: unknown } = {},
  ) {
    super(
      error.message,
      options.cause === undefined ? undefined : { cause: options.cause },
    );
    this.name = "ResourceError";
    this.code = error.code;
    this.fields = Object.freeze({ ...(error.fields ?? {}) });
    this.requestId = error.request_id;
    this.status = options.status;
  }
}

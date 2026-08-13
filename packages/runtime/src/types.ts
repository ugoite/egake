/** JSON values accepted by an Egake ResourceProvider. */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | JsonObject;
export type JsonObject = { [key: string]: JsonValue };
export type MaybePromise<T> = T | PromiseLike<T>;

export const CAPABILITIES = [
  "schema",
  "list",
  "get",
  "create",
  "update",
  "delete",
  "invoke",
] as const;
export type Capability = (typeof CAPABILITIES)[number];

export type ResourceErrorCode =
  | "validation_failed"
  | "not_found"
  | "conflict"
  | "capability_denied"
  | "unavailable"
  | "internal"
  | (string & Record<never, never>);

export interface StructuredError {
  readonly code: ResourceErrorCode;
  readonly message: string;
  readonly fields: Readonly<Record<string, string>>;
  readonly request_id?: string;
}

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
  readonly enum?: readonly JsonValue[];
  readonly format?:
    | "email"
    | "date"
    | "date-time"
    | (string & Record<never, never>);
}

export interface ResourceSchema {
  readonly name: string;
  readonly fields: readonly FieldSchema[];
  readonly capabilities: readonly Capability[];
}

export interface Sort {
  readonly field: string;
  readonly direction: "asc" | "desc";
}

export interface ListQuery {
  readonly q?: string;
  readonly sort: readonly Sort[];
  readonly offset: number;
  readonly limit: number;
}

export interface ResourcePage<T extends JsonObject = JsonObject> {
  readonly items: readonly T[];
  readonly total: number;
  readonly offset: number;
  readonly limit: number;
}

/** Egake owns data access; Ikasue never consumes this interface. */
export interface ResourceProvider<T extends JsonObject = JsonObject> {
  schema(): MaybePromise<ResourceSchema>;
  list(query: ListQuery): MaybePromise<ResourcePage<T>>;
  get(id: string): MaybePromise<T>;
  create(value: T): MaybePromise<T>;
  update(id: string, mergePatch: JsonObject): MaybePromise<T>;
  delete(id: string): MaybePromise<void>;
  invoke(action: string, input: JsonValue): MaybePromise<JsonValue>;
}

export interface ResourceClientOptions {
  readonly basePath?: string;
  readonly fetch?: typeof globalThis.fetch;
  readonly origin?: string;
  readonly requestId?: string | (() => string);
}

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

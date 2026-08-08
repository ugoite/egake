import {
  CAPABILITIES,
  Capability,
  JsonObject,
  JsonValue,
  ListQuery,
  ResourceClientOptions,
  ResourceError,
  ResourcePage,
  ResourceProvider,
  ResourceSchema,
  Sort,
  StructuredError,
} from "./types.ts";
import {
  isJsonObject,
  isJsonValue,
  requireObjectPatch,
} from "./merge-patch.ts";

const DEFAULT_BASE_PATH = "/api/egake/v1";
const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 500;
const MAX_QUERY_BYTES = 16 * 1024;
const MAX_JSON_BODY = 2 * 1024 * 1024;
const CAPABILITY_SET = new Set<Capability>(CAPABILITIES);
let requestCounter = 0;

/** Validates the request ID format accepted by the server adapter. */
export function isSafeRequestId(value: string): boolean {
  return value.length >= 1 && value.length <= 128 &&
    /^[A-Za-z0-9._:-]+$/.test(value);
}

/** Creates a local request ID without including request data or credentials. */
export function makeRequestId(): string {
  requestCounter += 1;
  const random = globalThis.crypto?.randomUUID?.();
  return `req-${random ?? `${Date.now().toString(36)}-${requestCounter}`}`;
}

/** Checks whether a schema grants one operation. */
export function hasCapability(
  schema: ResourceSchema,
  capability: Capability,
): boolean {
  return schema.capabilities.includes(capability);
}

/** Raises a structured capability error when an operation is not advertised. */
export function assertCapability(
  schema: ResourceSchema,
  capability: Capability,
): void {
  if (!CAPABILITIES.includes(capability)) {
    throw new ResourceError({
      code: "capability_denied",
      message: `unknown capability: ${capability}`,
    });
  }
  if (!hasCapability(schema, capability)) {
    throw new ResourceError({
      code: "capability_denied",
      message: `resource does not expose the ${capability} capability`,
    });
  }
}

function normalizeLimit(value: number): number {
  if (!Number.isInteger(value) || value < 0) {
    throw new ResourceError({
      code: "validation_failed",
      message: "limit must be a non-negative integer",
      fields: { limit: "must be a non-negative integer" },
    });
  }
  return value === 0 ? 1 : Math.min(value, MAX_LIMIT);
}

function normalizeQuery(query: ListQuery): ListQuery {
  if (typeof query !== "object" || query === null) {
    throw new ResourceError({
      code: "validation_failed",
      message: "list query must be an object",
    });
  }
  if (!Number.isInteger(query.offset) || query.offset < 0) {
    throw new ResourceError({
      code: "validation_failed",
      message: "offset must be a non-negative integer",
      fields: { offset: "must be a non-negative integer" },
    });
  }
  if (query.q !== undefined && typeof query.q !== "string") {
    throw new ResourceError({
      code: "validation_failed",
      message: "query q must be a string",
      fields: { q: "must be a string" },
    });
  }
  if (!Array.isArray(query.sort)) {
    throw new ResourceError({
      code: "validation_failed",
      message: "query sort must be an array",
      fields: { sort: "must be an array" },
    });
  }
  const sort = query.sort.map((item) => {
    if (
      typeof item !== "object" || item === null ||
      typeof (item as Sort).field !== "string" ||
      !((item as Sort).direction === "asc" ||
        (item as Sort).direction === "desc") ||
      (item as Sort).field.trim() === "" ||
      (item as Sort).field.split("").some((char) => char.charCodeAt(0) < 0x20)
    ) {
      throw new ResourceError({
        code: "validation_failed",
        message: "query sort contains an invalid field",
        fields: {
          sort: "fields must be non-empty strings with asc/desc directions",
        },
      });
    }
    return {
      field: (item as Sort).field,
      direction: (item as Sort).direction,
    };
  });
  return {
    q: query.q === "" ? undefined : query.q,
    sort,
    offset: query.offset,
    limit: normalizeLimit(query.limit),
  };
}

function sortQuery(sort: readonly Sort[]): string {
  return sort.map((item) =>
    item.direction === "desc" ? `-${item.field}` : item.field
  ).join(",");
}

function errorFromBody(
  value: unknown,
  status: number,
  requestId?: string,
): ResourceError {
  const candidate =
    typeof value === "object" && value !== null && "error" in value
      ? (value as { error?: unknown }).error
      : value;
  if (typeof candidate === "object" && candidate !== null) {
    const error = candidate as Partial<StructuredError>;
    if (typeof error.code === "string" && typeof error.message === "string") {
      const fields = error.fields && typeof error.fields === "object"
        ? Object.fromEntries(
          Object.entries(error.fields).filter(([, value]) =>
            typeof value === "string"
          ),
        )
        : {};
      const bodyRequestId = typeof error.request_id === "string" &&
          isSafeRequestId(error.request_id)
        ? error.request_id
        : requestId;
      return new ResourceError(
        {
          code: error.code,
          message: error.message,
          fields,
          request_id: bodyRequestId,
        },
        { status },
      );
    }
  }
  const code = status === 400 || status === 422
    ? "validation_failed"
    : status === 401 || status === 403 || status === 405
    ? "capability_denied"
    : status === 404
    ? "not_found"
    : status === 409
    ? "conflict"
    : status === 502 || status === 503 || status === 504 || status === 429
    ? "unavailable"
    : "internal";
  return new ResourceError({
    code,
    message: `resource request failed with HTTP ${status}`,
    request_id: requestId,
  }, { status });
}

function resourceName(value: string): string {
  if (
    typeof value !== "string" || !value || value.trim() !== value ||
    value === "." || value === ".." ||
    value.includes("/") || value.includes("\\") ||
    value.split("").some((char) => char.charCodeAt(0) < 0x20)
  ) {
    throw new ResourceError({
      code: "validation_failed",
      message: "resource name is invalid",
    });
  }
  return encodeURIComponent(value);
}

function pathSegment(value: string, label: string): string {
  if (
    typeof value !== "string" || !value || value === "." || value === ".." ||
    value.includes("\\") ||
    value.split("").some((char) => char.charCodeAt(0) < 0x20)
  ) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} is required`,
    });
  }
  return encodeURIComponent(value).replaceAll(".", "%2E");
}

function validateRelativePath(path: string, label: string): void {
  if (
    typeof path !== "string" || !path.startsWith("/") ||
    path.startsWith("//") || path.includes("\\") ||
    /^[a-z][a-z\d+.-]*:/i.test(path) || path.includes("#")
  ) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must be a same-origin relative path`,
    });
  }
  const pathname = path.split("?", 1)[0];
  if (pathname.includes("//")) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} contains an empty path segment`,
    });
  }
  for (const rawSegment of pathname.split("/")) {
    let segment: string;
    try {
      segment = decodeURIComponent(rawSegment);
    } catch {
      throw new ResourceError({
        code: "validation_failed",
        message: `${label} contains invalid URL encoding`,
      });
    }
    if (
      segment === "." || segment === ".." || segment.includes("\\") ||
      segment.split("").some((char) => char.charCodeAt(0) < 0x20)
    ) {
      throw new ResourceError({
        code: "validation_failed",
        message: `${label} contains a reserved path segment`,
      });
    }
  }
}

function parseResourceSchema(
  value: unknown,
  expectedName: string,
): ResourceSchema {
  if (
    !isJsonObject(value) || typeof value.name !== "string" ||
    value.name !== expectedName || !Array.isArray(value.fields) ||
    !Array.isArray(value.capabilities)
  ) {
    throw new ResourceError({
      code: "internal",
      message: "resource returned an invalid schema",
    });
  }
  const fields = value.fields.map((field) => {
    const enumValues = isJsonObject(field) ? field.enum : undefined;
    const format = isJsonObject(field) ? field.format : undefined;
    if (
      !isJsonObject(field) || typeof field.name !== "string" ||
      field.name.trim() === "" ||
      !["text", "number", "integer", "boolean", "date", "json"].includes(
        String(field.field_type),
      ) ||
      typeof field.required !== "boolean" ||
      (enumValues !== undefined &&
        (!Array.isArray(enumValues) || !enumValues.every(isJsonValue))) ||
      (format !== undefined && typeof format !== "string")
    ) {
      throw new ResourceError({
        code: "internal",
        message: "resource returned an invalid schema",
      });
    }
    return {
      name: field.name,
      field_type: field
        .field_type as ResourceSchema["fields"][number]["field_type"],
      required: field.required,
      ...(enumValues === undefined
        ? {}
        : { enum: enumValues as readonly JsonValue[] }),
      ...(format === undefined ? {} : { format }),
    };
  });
  if (new Set(fields.map((field) => field.name)).size !== fields.length) {
    throw new ResourceError({
      code: "internal",
      message: "resource returned an invalid schema",
    });
  }
  const capabilities = value.capabilities.map((capability) => {
    if (
      typeof capability !== "string" ||
      !CAPABILITY_SET.has(capability as Capability)
    ) {
      throw new ResourceError({
        code: "internal",
        message: "resource returned an invalid schema",
      });
    }
    return capability as Capability;
  });
  if (new Set(capabilities).size !== capabilities.length) {
    throw new ResourceError({
      code: "internal",
      message: "resource returned an invalid schema",
    });
  }
  return {
    name: value.name,
    fields,
    capabilities,
  };
}

function objectResponse<T extends JsonObject>(
  value: unknown,
  operation: string,
): T {
  if (!isJsonObject(value) || !isJsonValue(value)) {
    throw new ResourceError({
      code: "internal",
      message: `resource returned an invalid ${operation} result`,
    });
  }
  return value as T;
}

function pageResponse<T extends JsonObject>(value: unknown): ResourcePage<T> {
  if (
    !isJsonObject(value) || !Array.isArray(value.items) ||
    typeof value.total !== "number" || !Number.isInteger(value.total) ||
    value.total < 0 ||
    typeof value.offset !== "number" || !Number.isInteger(value.offset) ||
    value.offset < 0 ||
    typeof value.limit !== "number" || !Number.isInteger(value.limit) ||
    value.limit < 1 ||
    value.limit > MAX_LIMIT ||
    !value.items.every((item) => isJsonObject(item) && isJsonValue(item))
  ) {
    throw new ResourceError({
      code: "internal",
      message: "resource returned an invalid list page",
    });
  }
  return value as unknown as ResourcePage<T>;
}

function jsonBody(value: JsonValue, label: string): string {
  if (!isJsonValue(value)) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must contain only JSON data`,
    });
  }
  let body: string;
  try {
    body = JSON.stringify(value);
  } catch (cause) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} must contain only JSON data`,
    }, { cause });
  }
  if (new TextEncoder().encode(body).byteLength > MAX_JSON_BODY) {
    throw new ResourceError({
      code: "validation_failed",
      message: "request JSON body is too large",
    });
  }
  return body;
}

/** A browser-safe HTTP client for the documented same-origin JSON API. */
export class ResourceClient {
  readonly basePath: string;
  private readonly fetcher: typeof globalThis.fetch;
  private readonly origin?: string;
  private readonly requestIdOption?: string | (() => string);

  constructor(options: ResourceClientOptions = {}) {
    const basePath = options.basePath ?? DEFAULT_BASE_PATH;
    validateRelativePath(basePath, "basePath");
    if (basePath.includes("?")) {
      throw new ResourceError({
        code: "validation_failed",
        message: "basePath must not contain a query string",
      });
    }
    this.basePath = basePath.replace(/\/+$/, "");
    this.fetcher = options.fetch ?? globalThis.fetch;
    this.origin = options.origin ??
      (typeof globalThis.location === "object"
        ? globalThis.location.origin
        : undefined);
    this.requestIdOption = options.requestId;
    if (this.origin !== undefined) {
      try {
        if (
          new URL(this.basePath, this.origin).origin !==
            new URL(this.origin).origin
        ) {
          throw new Error("origin mismatch");
        }
      } catch {
        throw new ResourceError({
          code: "validation_failed",
          message: "origin must be a valid same-origin URL",
        });
      }
    }
  }

  /** Returns a provider facade for one registered resource. */
  resource<T extends JsonObject = JsonObject>(
    name: string,
  ): ResourceProvider<T> {
    return new RemoteResourceProvider<T>(this, name);
  }

  /** Performs one same-origin JSON request. This is public for embedded adapters. */
  async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    validateRelativePath(path, "request path");
    const relativeUrl = `${this.basePath}${path}`;
    if (relativeUrl.length > MAX_QUERY_BYTES * 2) {
      throw new ResourceError({
        code: "validation_failed",
        message: "request path is too large",
      });
    }
    if (
      this.origin &&
      new URL(relativeUrl, this.origin).origin !== new URL(this.origin).origin
    ) {
      throw new ResourceError({
        code: "validation_failed",
        message: "request path escaped the same origin",
      });
    }
    const url = this.origin
      ? new URL(relativeUrl, this.origin).toString()
      : relativeUrl;
    let supplied: unknown;
    try {
      supplied = typeof this.requestIdOption === "function"
        ? this.requestIdOption()
        : this.requestIdOption;
    } catch {
      supplied = undefined;
    }
    const requestId = typeof supplied === "string" && isSafeRequestId(supplied)
      ? supplied
      : makeRequestId();
    const headers = new Headers(init.headers);
    headers.set("accept", "application/json");
    headers.set("x-request-id", requestId);
    if (init.body !== undefined) {
      if (
        typeof init.body === "string" &&
        new TextEncoder().encode(init.body).byteLength > MAX_JSON_BODY
      ) {
        throw new ResourceError({
          code: "validation_failed",
          message: "request JSON body is too large",
        });
      }
      headers.set("content-type", "application/json");
    }
    let response: Response;
    try {
      response = await this.fetcher(url, {
        ...init,
        headers,
        credentials: "same-origin",
      });
    } catch (cause) {
      throw new ResourceError({
        code: "unavailable",
        message: "resource request was unavailable",
        request_id: requestId,
      }, { cause });
    }
    const responseHeaderRequestId = response.headers.get("x-request-id");
    const responseRequestId = typeof responseHeaderRequestId === "string" &&
        isSafeRequestId(responseHeaderRequestId)
      ? responseHeaderRequestId
      : requestId;
    let text: string;
    try {
      text = await response.text();
    } catch (cause) {
      throw new ResourceError({
        code: "unavailable",
        message: "resource response was unavailable",
        request_id: responseRequestId,
      }, { cause });
    }
    let value: unknown = null;
    if (text) {
      try {
        value = JSON.parse(text) as unknown;
      } catch {
        if (!response.ok) {
          throw errorFromBody(null, response.status, responseRequestId);
        }
        throw new ResourceError({
          code: "internal",
          message: "resource response was not valid JSON",
          request_id: responseRequestId,
        }, { status: response.status });
      }
    }
    if (!response.ok) {
      throw errorFromBody(value, response.status, responseRequestId);
    }
    return value as T;
  }

  /** Builds an encoded resource path without accepting slash-bearing names. */
  resourcePath(resource: string, suffix = ""): string {
    const path = `/resources/${resourceName(resource)}${suffix}`;
    validateRelativePath(path, "resource path");
    return path;
  }
}

class RemoteResourceProvider<T extends JsonObject>
  implements ResourceProvider<T> {
  private schemaCache?: Promise<ResourceSchema>;
  constructor(
    private readonly client: ResourceClient,
    private readonly name: string,
  ) {}

  schema(): Promise<ResourceSchema> {
    return this.schemaCache ??= this.client.request<unknown>(
      this.client.resourcePath(this.name, "/schema"),
    ).then((value) => parseResourceSchema(value, this.name));
  }

  private async require(capability: Capability): Promise<void> {
    assertCapability(await this.schema(), capability);
  }

  async list(query: ListQuery): Promise<ResourcePage<T>> {
    await this.require("list");
    const normalized = normalizeQuery(query);
    const params = new URLSearchParams();
    if (normalized.q) params.set("q", normalized.q);
    if (normalized.sort.length) params.set("sort", sortQuery(normalized.sort));
    params.set("offset", String(normalized.offset));
    params.set("limit", String(normalized.limit));
    if (params.toString().length > MAX_QUERY_BYTES) {
      throw new ResourceError({
        code: "validation_failed",
        message: "list query is too large",
      });
    }
    const page = await this.client.request<unknown>(
      `${this.client.resourcePath(this.name)}?${params.toString()}`,
    );
    return pageResponse<T>(page);
  }

  async get(id: string): Promise<T> {
    await this.require("get");
    const value = await this.client.request<unknown>(
      this.client.resourcePath(this.name, `/items/${pathSegment(id, "id")}`),
    );
    return objectResponse<T>(value, "get");
  }

  async create(value: T): Promise<T> {
    await this.require("create");
    if (!isJsonObject(value) || !isJsonValue(value)) {
      throw new ResourceError({
        code: "validation_failed",
        message: "resource value must be a JSON object",
      });
    }
    const result = await this.client.request<unknown>(
      this.client.resourcePath(this.name),
      {
        method: "POST",
        body: jsonBody(value, "resource value"),
      },
    );
    return objectResponse<T>(result, "create");
  }

  async update(id: string, mergePatch: JsonObject): Promise<T> {
    await this.require("update");
    requireObjectPatch(mergePatch);
    const value = await this.client.request<unknown>(
      this.client.resourcePath(this.name, `/items/${pathSegment(id, "id")}`),
      {
        method: "PATCH",
        body: jsonBody(mergePatch, "resource update patch"),
      },
    );
    return objectResponse<T>(value, "update");
  }

  async delete(id: string): Promise<void> {
    await this.require("delete");
    await this.client.request(
      this.client.resourcePath(this.name, `/items/${pathSegment(id, "id")}`),
      { method: "DELETE" },
    );
  }

  async invoke(action: string, input: JsonValue): Promise<JsonValue> {
    await this.require("invoke");
    if (!isJsonValue(input)) {
      throw new ResourceError({
        code: "validation_failed",
        message: "action input must be JSON",
      });
    }
    const result = await this.client.request<JsonValue>(
      this.client.resourcePath(
        this.name,
        `/actions/${pathSegment(action, "action")}`,
      ),
      {
        method: "POST",
        body: jsonBody(input, "action input"),
      },
    );
    if (!isJsonValue(result)) {
      throw new ResourceError({
        code: "internal",
        message: "resource returned an invalid action result",
      });
    }
    return result;
  }
}

/** Creates the same provider facade as `client.resource(name)`. */
export function createResourceProvider<T extends JsonObject = JsonObject>(
  client: ResourceClient,
  name: string,
): ResourceProvider<T> {
  return client.resource<T>(name);
}

export { DEFAULT_LIMIT, MAX_LIMIT };

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
import { isJsonObject, requireObjectPatch } from "./merge-patch.ts";

const DEFAULT_BASE_PATH = "/api/ikashita/v1";
const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 500;
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
  if (!Number.isInteger(query.offset) || query.offset < 0) {
    throw new ResourceError({
      code: "validation_failed",
      message: "offset must be a non-negative integer",
      fields: { offset: "must be a non-negative integer" },
    });
  }
  return {
    q: query.q === "" ? undefined : query.q,
    sort: query.sort ?? [],
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
      return new ResourceError(
        {
          code: error.code,
          message: error.message,
          fields: error.fields && typeof error.fields === "object"
            ? error.fields
            : {},
          request_id: typeof error.request_id === "string"
            ? error.request_id
            : requestId,
        },
        { status },
      );
    }
  }
  const code = status === 404
    ? "not_found"
    : status === 405
    ? "capability_denied"
    : status >= 500
    ? "unavailable"
    : "internal";
  return new ResourceError({
    code,
    message: `resource request failed with HTTP ${status}`,
    request_id: requestId,
  }, { status });
}

function resourceName(value: string): string {
  if (!value || value.includes("/")) {
    throw new ResourceError({
      code: "validation_failed",
      message: "resource name is invalid",
    });
  }
  return encodeURIComponent(value);
}

function pathSegment(value: string, label: string): string {
  if (!value) {
    throw new ResourceError({
      code: "validation_failed",
      message: `${label} is required`,
    });
  }
  return encodeURIComponent(value);
}

/** A browser-safe HTTP client for the documented same-origin JSON API. */
export class ResourceClient {
  readonly basePath: string;
  private readonly fetcher: typeof globalThis.fetch;
  private readonly origin?: string;
  private readonly requestIdOption?: string | (() => string);

  constructor(options: ResourceClientOptions = {}) {
    const basePath = options.basePath ?? DEFAULT_BASE_PATH;
    if (
      !basePath.startsWith("/") || basePath.startsWith("//") ||
      /^[a-z][a-z\d+.-]*:/i.test(basePath)
    ) {
      throw new ResourceError({
        code: "validation_failed",
        message: "basePath must be a same-origin relative path",
      });
    }
    this.basePath = basePath.replace(/\/+$/, "");
    this.fetcher = options.fetch ?? globalThis.fetch;
    this.origin = options.origin ??
      (typeof globalThis.location === "object"
        ? globalThis.location.origin
        : undefined);
    this.requestIdOption = options.requestId;
    if (this.origin) {
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
    const client = this;
    return new RemoteResourceProvider<T>(client, name);
  }

  /** Performs one same-origin JSON request. This is public for embedded adapters. */
  async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    if (
      !path.startsWith("/") || path.startsWith("//") ||
      /^[a-z][a-z\d+.-]*:/i.test(path)
    ) {
      throw new ResourceError({
        code: "validation_failed",
        message: "request path must be same-origin relative",
      });
    }
    const relativeUrl = `${this.basePath}${path}`;
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
    const supplied = typeof this.requestIdOption === "function"
      ? this.requestIdOption()
      : this.requestIdOption;
    const requestId = supplied && isSafeRequestId(supplied)
      ? supplied
      : makeRequestId();
    const headers = new Headers(init.headers);
    headers.set("accept", "application/json");
    headers.set("x-request-id", requestId);
    if (init.body !== undefined) {
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
    const responseRequestId = response.headers.get("x-request-id") ?? requestId;
    const text = await response.text();
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
    return `/resources/${resourceName(resource)}${suffix}`;
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
    return this.schemaCache ??= this.client.request<ResourceSchema>(
      this.client.resourcePath(this.name, "/schema"),
    );
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
    return await this.client.request<ResourcePage<T>>(
      `${this.client.resourcePath(this.name)}?${params.toString()}`,
    );
  }

  async get(id: string): Promise<T> {
    await this.require("get");
    return await this.client.request<T>(
      this.client.resourcePath(this.name, `/items/${pathSegment(id, "id")}`),
    );
  }

  async create(value: T): Promise<T> {
    await this.require("create");
    if (!isJsonObject(value)) {
      throw new ResourceError({
        code: "validation_failed",
        message: "resource value must be a JSON object",
      });
    }
    return await this.client.request<T>(this.client.resourcePath(this.name), {
      method: "POST",
      body: JSON.stringify(value),
    });
  }

  async update(id: string, mergePatch: JsonObject): Promise<T> {
    await this.require("update");
    requireObjectPatch(mergePatch);
    return await this.client.request<T>(
      this.client.resourcePath(this.name, `/items/${pathSegment(id, "id")}`),
      {
        method: "PATCH",
        body: JSON.stringify(mergePatch),
      },
    );
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
    return await this.client.request<JsonValue>(
      this.client.resourcePath(
        this.name,
        `/actions/${pathSegment(action, "action")}`,
      ),
      {
        method: "POST",
        body: JSON.stringify(input),
      },
    );
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

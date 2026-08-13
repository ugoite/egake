import {
  type Capability,
  type JsonObject,
  type JsonValue,
  ResourceError,
  type ResourcePage,
  type ResourceProvider,
} from "../../packages/runtime/mod.ts";

const STATUS_ITEMS: JsonObject[] = [{
  id: "local",
  name: "Embedded host",
  healthy: true,
}];

function unsupported(operation: string): never {
  throw new ResourceError({
    code: "capability_denied",
    message: `embedded status provider does not expose ${operation}`,
  });
}

/** A deterministic provider fixture for an embedded Egake application host. */
export function createEmbeddedProvider(): ResourceProvider {
  const capabilities: readonly Capability[] = ["schema", "list", "invoke"];
  return {
    schema: () => ({ name: "status", fields: [], capabilities }),
    list: (query): ResourcePage => {
      const needle = query.q?.toLowerCase();
      const items = needle
        ? STATUS_ITEMS.filter((item) =>
          JSON.stringify(item).toLowerCase().includes(needle)
        )
        : STATUS_ITEMS.slice();
      return {
        items,
        total: items.length,
        offset: query.offset,
        limit: query.limit,
      };
    },
    get: () => unsupported("get"),
    create: () => unsupported("create"),
    update: () => unsupported("update"),
    delete: () => unsupported("delete"),
    invoke: (action: string, input: JsonValue) => {
      if (action !== "health") {
        throw new ResourceError({
          code: "not_found",
          message: `action not found: ${action}`,
        });
      }
      return { ok: true, action, input };
    },
  };
}

// An embedded host passes this provider to its own Egake action/resource loop;
// Ikasue only receives the resulting IkaView properties and DOM events.

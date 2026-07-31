import {
  Capability,
  isJsonObject,
  JsonObject,
  JsonValue,
  mountApplication,
  ResourceError,
  ResourcePage,
  ResourceProvider,
  SerializedApplication,
} from "../../packages/runtime/mod.ts";

const STATUS_ITEMS: JsonObject[] = [
  { id: "local", name: "Embedded host", healthy: true },
];

function unsupported(operation: string): never {
  throw new ResourceError({
    code: "capability_denied",
    message: `embedded status provider does not expose ${operation}`,
  });
}

/** A deterministic provider fixture suitable for an embedded browser host. */
export function createEmbeddedProvider(): ResourceProvider {
  const capabilities: readonly Capability[] = ["schema", "list", "invoke"];
  return {
    schema: () => ({
      name: "status",
      fields: [],
      capabilities,
    }),
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
          message: `embedded status action was not found: ${action}`,
        });
      }
      return { ok: true, action, input };
    },
  };
}

/** Executes only declared provider-invoke steps through injected adapters. */
export async function runEmbeddedAction(
  application: SerializedApplication,
  name: string,
  providers: Readonly<Record<string, ResourceProvider>>,
): Promise<JsonValue | undefined> {
  const action = application.actions.find((candidate) =>
    candidate.name === name
  );
  if (!action) {
    throw new ResourceError({
      code: "not_found",
      message: `application action was not found: ${name}`,
    });
  }
  let result: JsonValue | undefined;
  for (const step of action.steps) {
    if (step.kind !== "invoke") continue;
    if (!isJsonObject(step.attributes)) {
      throw new ResourceError({
        code: "validation_failed",
        message: "invoke steps require an attributes object",
      });
    }
    const resource = step.attributes.resource;
    const providerAction = step.attributes.action;
    if (typeof resource !== "string" || typeof providerAction !== "string") {
      throw new ResourceError({
        code: "validation_failed",
        message: "invoke steps require string resource and action attributes",
      });
    }
    const provider = providers[resource];
    if (!provider) {
      throw new ResourceError({
        code: "not_found",
        message: `provider was not injected: ${resource}`,
      });
    }
    result = await provider.invoke(providerAction, step.attributes.input ?? {});
  }
  return result;
}

/** The host supplies providers; application JSON contains no data credentials. */
export function startIkashitaHost(
  root: HTMLElement,
  application: SerializedApplication,
  providers: Readonly<Record<string, ResourceProvider<JsonObject>>>,
) {
  return mountApplication(root, application, {
    providers,
    onAction: async (action) => {
      await runEmbeddedAction(application, action, providers);
    },
  });
}

// A real host can call startIkashitaHost(document.getElementById("app")!, json, {
//   status: createEmbeddedProvider(),
// }) after loading its own serialized application bundle and provider adapters.

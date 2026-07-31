import {
  JsonObject,
  JsonValue,
  ListQuery,
  ResourcePage,
  ResourceProvider,
  ResourceSchema,
} from "../../packages/runtime/mod.ts";

/** The smallest protocol an existing Ugoite client must expose to this adapter. */
export interface UgoiteEntriesClient {
  schema(): Promise<ResourceSchema>;
  list(query: ListQuery): Promise<ResourcePage>;
  get(id: string): Promise<JsonObject>;
  create(value: JsonObject): Promise<JsonObject>;
  update(id: string, mergePatch: JsonObject): Promise<JsonObject>;
  delete(id: string): Promise<void>;
  invoke(action: string, input: JsonValue): Promise<JsonValue>;
}

/** Wraps a host-owned Ugoite entries client without owning auth or transport. */
export function createUgoiteEntriesProvider(
  client: UgoiteEntriesClient,
): ResourceProvider {
  return {
    schema: () => client.schema(),
    list: (query) => client.list(query),
    get: (id) => client.get(id),
    create: (value) => client.create(value),
    update: (id, mergePatch) => client.update(id, mergePatch),
    delete: (id) => client.delete(id),
    invoke: (action, input) => client.invoke(action, input),
  };
}

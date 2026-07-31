import {
  JsonObject,
  JsonValue,
  ListQuery,
  ResourcePage,
  ResourceSchema,
} from "../../packages/runtime/mod.ts";
import { createUgoiteEntriesProvider, UgoiteEntriesClient } from "./adapter.ts";

class FakeUgoiteClient implements UgoiteEntriesClient {
  schema(): Promise<ResourceSchema> {
    return Promise.resolve({
      name: "entries",
      fields: [],
      capabilities: [
        "schema",
        "list",
        "get",
        "create",
        "update",
        "delete",
        "invoke",
      ],
    });
  }

  list(_query: ListQuery): Promise<ResourcePage> {
    return Promise.resolve({
      items: [{ id: "1", title: "Offline entry" }],
      total: 1,
      offset: 0,
      limit: 50,
    });
  }

  get(_id: string): Promise<JsonObject> {
    return Promise.resolve({ id: "1", title: "Offline entry" });
  }

  create(value: JsonObject): Promise<JsonObject> {
    return Promise.resolve(value);
  }

  update(_id: string, mergePatch: JsonObject): Promise<JsonObject> {
    return Promise.resolve(mergePatch);
  }

  delete(_id: string): Promise<void> {
    return Promise.resolve();
  }

  invoke(action: string, input: JsonValue): Promise<JsonValue> {
    return Promise.resolve({ action, input });
  }
}

Deno.test("Ugoite adapter delegates the full resource contract", async () => {
  const provider = createUgoiteEntriesProvider(new FakeUgoiteClient());
  const page = await provider.list({ sort: [], offset: 0, limit: 50 });
  if (page.items[0].title !== "Offline entry") {
    throw new Error("list was not delegated");
  }
  const result = await provider.invoke("publish", { dry_run: true });
  if (
    JSON.stringify(result) !==
      JSON.stringify({ action: "publish", input: { dry_run: true } })
  ) {
    throw new Error("invoke was not delegated");
  }
});

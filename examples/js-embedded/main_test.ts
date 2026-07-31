import application from "./application.json" with { type: "json" };
import { createEmbeddedProvider, runEmbeddedAction } from "./main.ts";
import { SerializedApplication } from "../../packages/runtime/mod.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

Deno.test("embedded provider supports deterministic list/search and invoke", async () => {
  const provider = createEmbeddedProvider();
  const page = await provider.list({
    q: "embedded",
    sort: [],
    offset: 0,
    limit: 50,
  });
  assert(page.total === 1, "expected one matching embedded item");
  const result = await runEmbeddedAction(
    application as SerializedApplication,
    "health-check",
    { status: provider },
  );
  assert(
    JSON.stringify(result) === JSON.stringify({
      ok: true,
      action: "health",
      input: { source: "embedded-example" },
    }),
    "provider-defined action result changed",
  );
});

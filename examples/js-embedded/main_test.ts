import { createEmbeddedProvider } from "./main.ts";

Deno.test("embedded provider supports deterministic list/search", async () => {
  const provider = createEmbeddedProvider();
  const page = await provider.list({
    q: "embedded",
    sort: [],
    offset: 0,
    limit: 50,
  });
  if (page.total !== 1 || page.items[0]?.id !== "local") {
    throw new Error("embedded provider changed");
  }
});

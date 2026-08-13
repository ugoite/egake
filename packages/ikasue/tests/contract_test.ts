import {
  type IkaQuery,
  IKASUE_ABI_VERSION,
  type IkaView,
} from "../src/contract.ts";

Deno.test("Ikasue Web ABI is versioned and data-grid queries are JSON data", () => {
  const view: IkaView = {
    version: IKASUE_ABI_VERSION,
    kind: "data-grid",
    props: { id: "contacts" },
  };
  const query: IkaQuery = {
    offset: 400,
    limit: 80,
    sort: [{ field: "name", direction: "asc" }],
  };
  if (
    view.version !== "ikasue-web/1" || query.offset !== 400 ||
    query.limit !== 80
  ) {
    throw new Error("Ikasue contract changed");
  }
  if (
    JSON.stringify(view).includes("resource") ||
    JSON.stringify(view).includes("action")
  ) {
    throw new Error("Egake binding leaked into IkaView");
  }
});

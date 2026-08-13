---
title: JavaScript embedding
description: Inject a ResourceProvider from a Deno or TypeScript host and run egake.
sidebar:
  label: JavaScript embedding
---

<!-- i18n-sync: id=guide/usage/javascript digest=448b1304a1f2741f457958683337027568407bf11bd9d08cfe18ab8393d3f2c9 -->

JavaScript embedding separates the application definition from the provider. `packages/runtime` provides the Resource Contract, while the host owns the provider map and Egake action loop. When UI is needed, `packages/ikasue` lowers IkaView values to Custom Elements and the host handles semantic DOM events.

## Check the checked-in example

```sh
deno check examples/js-embedded/main.ts
deno test examples/js-embedded/main_test.ts
```

`createEmbeddedProvider()` in `examples/js-embedded/main.ts` advertises only `schema`, `list`, and `invoke` for the `status` resource. The provider owns data access and provider actions; the host owns the application action loop.

```ts
const capabilities: readonly Capability[] = ["schema", "list", "invoke"];

const provider: ResourceProvider = {
  schema: () => ({ name: "status", fields: [], capabilities }),
  list: (query) => ({
    items,
    total: items.length,
    offset: query.offset,
    limit: query.limit,
  }),
  get: () => unsupported("get"),
  create: () => unsupported("create"),
  update: () => unsupported("update"),
  delete: () => unsupported("delete"),
  invoke: (action, input) => ({ ok: true, action, input }),
};
```

This is the provider boundary; the complete fixture is in `examples/js-embedded/main.ts`.

## Connect to the UI runtime

In the browser, the host loads the generated bundle. Egake owns providers, state, and the action loop, then passes IkaView values and bindings to Ikasue. Ikasue lowers the same IkaView values to Custom Elements and sends semantic DOM events such as `ika-query`, `ika-select`, `ika-edit`, and `ika-action` back to the host.

For a custom host, use `renderIkaView` and the Custom Element properties from `packages/ikasue`. The DOM ABI is the Web Platform itself; there is no Egake-specific UI adapter or SerializedComponent layer.

## Use an HTTP provider

`ResourceClient` accepts a same-origin relative API root, validates request IDs, queries, schemas, and structured errors. The authoritative route list is in the [standalone HTTP adapter specification](../../../spec/#standalone-http-adapter).

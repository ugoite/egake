---
title: JavaScript embedding
description: Inject a ResourceProvider from a Deno or TypeScript host and run egake.
sidebar:
  label: JavaScript embedding
---

<!-- i18n-sync: id=guide/usage/javascript digest=1ce6111c1978af1d492ea89ba89d8c7a4d5bc2da00340bafa5ca5b00f1a6bd68 -->

JavaScript embedding separates the application definition from the provider. `packages/runtime` uses only Deno/TypeScript built-ins, while the host owns the provider map.

## Check the checked-in example

```sh
deno check examples/js-embedded/main.ts
deno test examples/js-embedded/main_test.ts
```

`createEmbeddedProvider()` in `examples/js-embedded/main.ts` advertises only `schema`, `list`, and `invoke` for the `status` resource. `runEmbeddedAction()` reads the Application Profile `invoke` step and calls only the injected provider’s `invoke` method.

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

## Mount into the DOM

The host loads the bundle and passes a root element plus provider map.

```ts
startEgakeHost(document.getElementById("app")!, application, {
  status: createEmbeddedProvider(),
});
```

The runtime creates DOM nodes and writes values through `textContent` or DOM properties. It does not use arbitrary HTML strings, `eval`, or remote assets.

## Use an HTTP provider

`ResourceClient` accepts a same-origin relative API root, validates request IDs, queries, schemas, and structured errors. The authoritative route list is in the [standalone HTTP adapter specification](../../../spec/#standalone-http-adapter).

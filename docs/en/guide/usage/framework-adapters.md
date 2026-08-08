---
title: React / Vue / Solid / Svelte adapters
description: Shipped React/Vue adapters and the generic runtime boundary for Solid/Svelte.
sidebar:
  label: Framework adapters
---

<!-- i18n-sync: id=guide/usage/framework-adapters digest=8882a03a29bcaf6830561af491fb3cb739e31f6077a39e8628c3c8b0c4f1be6f -->

Framework adapters are thin layers that translate an Application Profile JSON value into a framework element or VNode. egake does not add the framework as a dependency; the host supplies render primitives.

## Current support

| Framework | Status in this checkout | Public identifier                                     |
| --------- | ----------------------- | ----------------------------------------------------- |
| React     | Adapter shipped         | `createReactRenderer` / `createReactResourceProvider` |
| Vue       | Adapter shipped         | `createVueRenderer` / `createVueResourceProvider`     |
| Solid     | No dedicated adapter    | Connect `packages/runtime` to the DOM lifecycle       |
| Svelte    | No dedicated adapter    | Connect `packages/runtime` to the DOM lifecycle       |

Do not document an npm package or import that this checkout does not ship. If a dedicated adapter is needed, build a thin translation layer around `SerializedApplication`, `SerializedComponent`, and `ResourceProvider`, and add its implementation and tests in the same change.

## React

React receives `createElement` from the host. The adapter itself does not import React.

```ts
import {
  createReactRenderer,
  createReactResourceProvider,
} from "./packages/react/mod.ts";

const contacts = createReactResourceProvider(client, "contacts");
const renderApplication = createReactRenderer(React, {
  onAction: (action) => void contacts.invoke(action, null),
});
const element = renderApplication(applicationJson);
```

The implementation is in `packages/react/src/index.ts`; its README is `packages/react/README.md`. Children use normal React children and never `dangerouslySetInnerHTML`.

## Vue

Vue receives `h` from the host.

```ts
import {
  createVueRenderer,
  createVueResourceProvider,
} from "./packages/vue/mod.ts";

const contacts = createVueResourceProvider(client, "contacts");
const renderApplication = createVueRenderer(Vue, {
  onAction: (action) => void contacts.invoke(action, null),
});
const vnode = renderApplication(applicationJson);
```

Here `Vue` is the host value that implements `h`; the adapter does not depend on Vue.

## Solid / Svelte as a generic boundary

1. Load `application.json` or a data-only CLI bundle in the host.
2. Create a `ResourceProvider` that maps the framework store, loader, or API client.
3. Let the framework own creation and destruction of the DOM root.
4. Preserve provider capabilities and structured errors; do not turn them into HTML strings.

The safety and contract details are in the [TypeScript/Deno browser runtime specification](../../../spec/#typescriptdeno-browser-runtime).

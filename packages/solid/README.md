# `@ikashita/solid`

This adapter is dependency-free. It uses the host's Solid primitives instead of
importing `solid-js`, so the application keeps ownership of its Solid version
and renderer.

```ts
import { createComponent, createElement, insert } from "solid-js/web";
import { ResourceClient } from "../runtime/mod.ts";
import { createSolidRenderer, createSolidResourceProvider } from "./mod.ts";

const host = {
  createElement,
  createComponent,
  insert,
  setAttribute: (element: unknown, name: string, value: string) =>
    (element as Element).setAttribute(name, value),
  listen: (
    element: unknown,
    event: string,
    listener: (event: unknown) => void,
  ) => (element as Element).addEventListener(event, listener as EventListener),
};
const client = new ResourceClient();
const contacts = createSolidResourceProvider(client, "contacts");
const renderApplication = createSolidRenderer(host, {
  onAction: (action) => void contacts.invoke(action, null),
});
const tree = renderApplication(applicationJson);
```

Pass `tree` to the host's normal Solid `render` call. Values from the
application profile are inserted as text or ordinary attributes; the adapter
does not use HTML strings or `innerHTML`. `createComponent` is used for the
application boundary and `createElement`/`insert` preserve nested children.

# `@ikashita/svelte`

The package does not depend on the Svelte compiler or runtime. It exposes a
small host callback boundary, which works from a Svelte action, a wrapper
component, or a deterministic test host.

```ts
import { ResourceClient } from "../runtime/mod.ts";
import { createSvelteRenderer, createSvelteResourceProvider } from "./mod.ts";

const host = {
  createElement: (type: string) => document.createElement(type),
  createText: (value: string) => document.createTextNode(value),
  append: (parent: Node, child: Node) => parent.appendChild(child),
  clear: (parent: Element) => parent.replaceChildren(),
  setAttribute: (element: Element, name: string, value: string) =>
    element.setAttribute(name, value),
  listen: (
    element: Element,
    event: string,
    listener: (event: unknown) => void,
  ) => {
    const callback = listener as EventListener;
    element.addEventListener(event, callback);
    return () => element.removeEventListener(event, callback);
  },
};
const client = new ResourceClient();
const contacts = createSvelteResourceProvider(client, "contacts");
const renderApplication = createSvelteRenderer(host, {
  onAction: (action) => void contacts.invoke(action, null),
});
const mounted = renderApplication(
  document.querySelector("#app")!,
  applicationJson,
);
```

Call `mounted.update(nextApplication)` when the profile changes and
`mounted.destroy()` in the Svelte action's destroy callback. Text and children
are created with host primitives, so serialized strings never become HTML and
the adapter never uses `innerHTML`.

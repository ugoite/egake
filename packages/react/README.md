# `@ikashita/react`

This is a thin, dependency-free adapter. React stays a host application
dependency; the package only expects a value with `createElement`.

```ts
import { ResourceClient } from "../runtime/mod.ts";
import { createReactRenderer, createReactResourceProvider } from "./mod.ts";

const client = new ResourceClient();
const contacts = createReactResourceProvider(client, "contacts");
const renderApplication = createReactRenderer(React, {
  onAction: (action) => void contacts.invoke(action, null),
});
const element = renderApplication(applicationJson);
```

Children are passed as ordinary React children, never as an HTML string or
`dangerouslySetInnerHTML` value.

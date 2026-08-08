# Ugoite entries adapter

This example defines a client protocol and wraps it as an egake
`ResourceProvider`. The host supplies the existing Ugoite client. egake does not
own Ugoite URLs, authentication, cookies, storage, types, or a checkout; the
example intentionally contains none of those.

The adapter is runnable without Ugoite itself:

```sh
deno check examples/ugoite-entries/adapter.ts
deno test examples/ugoite-entries/adapter_test.ts
```

The test uses a deterministic in-memory client and verifies that a
provider-defined action is passed through unchanged.

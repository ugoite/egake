# Embedded JavaScript host

`main.ts` shows the injection boundary: the host owns provider construction and
passes a provider map to `mountApplication`. The example provider is an
in-memory deterministic fixture with list/search and a provider-defined `health`
action. The application bundle is data-only JSON. No credentials, cookies,
remote assets, or checkout-specific client is part of this example.

Run its offline checks with:

```sh
deno check examples/js-embedded/main.ts
deno test examples/js-embedded/main_test.ts
```

The exported `runEmbeddedAction` function is the host-owned action boundary: the
application can name an action, but only the injected provider decides what
`health` does.

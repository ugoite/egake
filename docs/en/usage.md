---
title: Acceptance matrix and common commands
description: Offline verification workflows for ikashita using the checked-in examples.
sidebar:
  label: Acceptance matrix
---

<!-- i18n-sync: id=usage digest=0d35b22ba5041d2c537294f5ef31a58f47e8277cc786227c5919b82eba600032 -->

This page is the executable usage and acceptance reference. For the beginner path, start with the [short quickstart](../guide/quickstart/). Providers own data and actions; the browser receives data-only application metadata; and the CLI never evaluates Rhai, shell commands, JavaScript, or remote code.

## Acceptance matrix

| Workflow                       | Example or implementation                                                                                                                                              | Offline acceptance command                                                               | Covered behavior                                                                                 |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Read-only CSV list/search      | [`examples/csv-readonly`](https://github.com/ikashita/ikashita/tree/main/examples/csv-readonly)                                                                        | `cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada` | CSV without an `id`, schema/list-only capabilities, case-insensitive search, sorting, pagination |
| Multi-resource project         | [`examples/multi-resource`](https://github.com/ikashita/ikashita/tree/main/examples/multi-resource)                                                                    | `cargo run -p ikashita-cli -- test examples/multi-resource`                              | Two resources, two schemas/data files, deterministic bundle and validation                       |
| Standalone HTML build          | CLI integration test                                                                                                                                                   | `cargo test -p ikashita-cli --test usage_examples`                                       | Single-HTML and directory builds, no external runtime/application assets                         |
| Browser/JS embedded provider   | [`examples/js-embedded`](https://github.com/ikashita/ikashita/tree/main/examples/js-embedded)                                                                          | `deno test examples/js-embedded/main_test.ts`                                            | Host-owned provider injection, list/search, and declared provider action invoke                  |
| Solid/Svelte host adapters     | [`packages/solid`](https://github.com/ikashita/ikashita/tree/main/packages/solid), [`packages/svelte`](https://github.com/ikashita/ikashita/tree/main/packages/svelte) | `mise run deno:test`                                                                     | Safe recursive children, host primitive boundaries, provider helpers, lifecycle                  |
| Python ASGI/FastAPI provider   | [`examples/python-fastapi`](https://github.com/ikashita/ikashita/tree/main/examples/python-fastapi)                                                                    | `mise run python:test`                                                                   | Standard-library ASGI routes and optional FastAPI bridge, including invoke                       |
| Ugoite client adapter          | [`examples/ugoite-entries`](https://github.com/ikashita/ikashita/tree/main/examples/ugoite-entries)                                                                    | `deno test examples/ugoite-entries/adapter_test.ts`                                      | Transparent CRUD/action delegation to a host-owned client                                        |
| Provider-defined action invoke | Rust server, JS, and Python adapters                                                                                                                                   | `cargo test -p ikashita-server && mise run deno:test && mise run python:test`            | `/actions/:action`, safe browser invoke, and deterministic host adapters                         |

The complete local acceptance suite is:

```sh
mise install
mise run setup
mise run fmt:check
mise run lint
mise run check
mise run test
mise run build
mise run deno:fmt:check
mise run deno:check
mise run deno:test
mise run python:test
./.githooks/pre-commit
```

These checks use fixture files only. They do not need network access, credentials, FastAPI, Ugoite, a browser, or a running server. The optional FastAPI host can be started after installing FastAPI and an ASGI server:

```sh
PYTHONPATH=python uvicorn app:app --app-dir examples/python-fastapi
```

## Common commands

Validate, inspect, build, list, and test a project without starting a server:

```sh
cargo run -p ikashita-cli -- validate examples/contacts-crud
cargo run -p ikashita-cli -- inspect examples/multi-resource
cargo run -p ikashita-cli -- build examples/multi-resource
cargo run -p ikashita-cli -- build examples/multi-resource --format single-html --output dist/multi-resource.html
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada --sort title
cargo run -p ikashita-cli -- test examples/multi-resource
```

`list` opens the configured local CSV directly. With `--json`, it emits the contract page object (`items`, `total`, `offset`, and `limit`); the default output emits one JSON record per line after a short page summary.

The default build is directory-style and produces four files. Use `--format single-html` or `--single-html` when a deployment accepts only one file. The standalone document has inline CSS, runtime JS, and application JSON, with no `runtime.js`, `runtime.css`, or `app.bundle.json` fetch/link/script reference. The JSON data block is non-executable and escapes script-sensitive characters; the inline executable/style blocks are protected by CSP hashes.

To exercise the HTTP server locally, use a loopback address and query the same contract routes with a browser or `curl`:

```sh
cargo run -p ikashita-cli -- run examples/contacts-crud --port 8787
curl 'http://127.0.0.1:8787/api/ikashita/v1/resources/contacts?q=Ada&limit=10'
```

The server's provider-defined action route is:

```text
POST /api/ikashita/v1/resources/:name/actions/:action
```

Hosts provide the action implementation. A declarative `invoke` step only selects a declared provider and action and passes JSON input; it is not a code execution hook.

## Development and resource configuration

`dev` is explicitly no-watch in this MVP. It validates the project, builds an in-memory static bundle, opens its configured providers, and serves on loopback. Editing source files requires stopping and restarting `dev`; no filesystem watcher or hot reload is implied.

Resource configuration has one source per project. If `resources.kdl` exists, it is selected and `[resources.<name>]` tables must not also be present. Otherwise the TOML tables are used. Supplying both forms is a hard configuration error, and both forms resolve paths relative to the project while rejecting absolute paths and `..` components.

```toml
[resources.catalog]
path = "data/catalog.csv"
writable = false
```

```kdl
/- kdl-version 2
resources {
    csv "catalog" path="data/catalog.csv"
}
```

The KDL form is used by the checked-in examples because it makes multiple providers easy to review. `writable` defaults to false and the conventional `id` key is optional for read-only CSVs; a CSV with no `id` advertises only schema/list capabilities.

### Solid and Svelte host connections

The optional framework adapters are `packages/solid` and `packages/svelte`. They have no Solid, Svelte, compiler, or runtime dependency. Import the renderer and connect the host primitives explicitly:

```ts
import { createSolidRenderer } from "./packages/solid/mod.ts";
const tree = createSolidRenderer(solidHost)(applicationJson);
```

```ts
import { createSvelteRenderer } from "./packages/svelte/mod.ts";
const mounted = createSvelteRenderer(svelteHost)(target, applicationJson);
mounted.destroy();
```

Both adapters recurse through children using host primitives and preserve serialized strings as text. Add the resource-provider helper when the host needs a `ResourceClient`; the provider remains outside the adapter and owns authentication and data access.

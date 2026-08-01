---
title: 受け入れマトリクスと共通コマンド
description: チェックアウト済みexampleを使った、ikashitaのオフライン検証手順。
sidebar:
  label: 受け入れマトリクス
---

This page is the repository's executable usage and acceptance reference. For
the beginner path, start with [最短クイックスタート](guide/quickstart.mdx).

This page is the executable follow-up to the original draft plan. The plan's
user-facing workflows are constrained by the decisions in
[`spec.md`](spec.md): providers own data and actions, the browser receives
data-only application metadata, and the CLI never evaluates Rhai, shell
commands, JavaScript, or remote code.

## Acceptance matrix

| Workflow | Example or implementation | Offline acceptance command | Covered behavior |
| --- | --- | --- | --- |
| Read-only CSV list/search | [`examples/csv-readonly`](../examples/csv-readonly) | `cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada` | CSV without an `id`, schema/list-only capabilities, case-insensitive search, sorting, pagination |
| Multi-resource project | [`examples/multi-resource`](../examples/multi-resource) | `cargo run -p ikashita-cli -- test examples/multi-resource` | Two declared resources, two schemas/data files, deterministic bundle and validation |
| Browser/JS embedded provider | [`examples/js-embedded`](../examples/js-embedded) | `deno test examples/js-embedded/main_test.ts` | Host-owned provider injection, list/search, declared provider action invoke |
| Python ASGI/FastAPI provider | [`examples/python-fastapi`](../examples/python-fastapi) | `mise run python:test` | Standard-library ASGI routes and optional FastAPI bridge, including invoke |
| Ugoite client adapter | [`examples/ugoite-entries`](../examples/ugoite-entries) | `deno test examples/ugoite-entries/adapter_test.ts` | Host-owned Ugoite client protocol and transparent CRUD/action delegation |
| Provider-defined action invoke | Rust server, JS, and Python adapters | `cargo test -p ikashita-server && mise run deno:test && mise run python:test` | `/actions/:action`, safe browser invoke step, and deterministic host adapters |

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

These checks use fixture files only. They do not need network access,
credentials, FastAPI, Ugoite, a browser, or a running server. The optional
FastAPI host can be started after installing FastAPI and an ASGI server:

```sh
PYTHONPATH=python uvicorn app:app --app-dir examples/python-fastapi
```

## Common commands

Validate, inspect, build, list, and test a project without starting a server:

```sh
cargo run -p ikashita-cli -- validate examples/contacts-crud
cargo run -p ikashita-cli -- inspect examples/multi-resource
cargo run -p ikashita-cli -- build examples/multi-resource
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada --sort title
cargo run -p ikashita-cli -- test examples/multi-resource
```

`list` opens the configured local CSV directly. `--json` emits the contract
page object (`items`, `total`, `offset`, and `limit`) for scripts; the default
output emits one JSON record per line after a short page summary.

To exercise the HTTP server locally, use a loopback address and query the same
contract routes with a browser or `curl`:

```sh
cargo run -p ikashita-cli -- run examples/contacts-crud --port 8787
curl 'http://127.0.0.1:8787/api/ikashita/v1/resources/contacts?q=Ada&limit=10'
```

The server's provider-defined action route is:

```text
POST /api/ikashita/v1/resources/:name/actions/:action
```

Hosts provide the action implementation. A declarative `invoke` step only
selects a declared provider and action and passes JSON input; it is not a code
execution hook.

## Development and resource configuration

`dev` is explicitly no-watch in this increment. It validates the project,
builds an in-memory static bundle, opens its configured providers, and serves
on loopback. Editing source files requires stopping and restarting `dev`; no
filesystem watcher or hot reload is implied.

Resource configuration has one source per project. If `resources.kdl` exists,
it is the selected KDL source and `[resources.<name>]` tables must not also be
present. If it does not exist, the TOML tables are used. Supplying both forms
is a hard configuration error rather than an override or merge, so there is no
ambiguous per-field precedence. Both forms resolve paths relative to the
project and reject absolute paths and `..` components.

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

The KDL form is used by the checked-in examples because it makes multiple
providers easy to review. `writable` defaults to false and the conventional
`id` key is optional for read-only CSVs; a CSV with no `id` advertises only
schema/list capabilities.

# ikashita

ikashita is an MIT-licensed Rust/WASM-oriented low-code UI runtime. The
project is being delivered incrementally around a transport-neutral Resource
Contract and a KDL Application Profile.

This repository currently contains the standalone data/API increment:

- `ikashita-resource` defines the shared resource query, schema, page,
  structured-error types, generic provider trait, JSON provider boundary, and
  merge-patch helper.
- `ikashita-spec` owns the versioned Application Profile metadata.
- `ikashita-csv` provides a locked, atomic-write local CSV Resource Provider.
- `ikashita-server` provides the localhost HTTP router, provider registry, and
  static-bundle configuration.
- `ikashita-cli` provides the versioned command-line entry point.
- `packages/runtime` provides the dependency-free Deno/TypeScript browser
  client, provider types, merge-patch helper, and safe JSON renderer.
- `packages/react` and `packages/vue` provide framework-thin element/VNode
  adapters without installing either framework.
- `python/ikashita` provides the standard-library Resource protocol/base class,
  ASGI adapter, and optional FastAPI bridge.

The executable MVP decisions are recorded in [`docs/spec.md`](docs/spec.md).
Ugoite integration will be an adapter boundary and is not a workspace
dependency.

## Tooling

Install [mise](https://mise.jdx.dev/) and run commands from the repository
root. `mise.toml` pins Rust 1.94.0, Deno 2.8.3, and the shared target directory.
The setup task only reports available tools and configures the local Git hook;
it does not require Docker, a browser, or a download step.

```sh
mise run setup
mise run fmt:check
mise run check
mise run test
mise run ci
```

Host-specific checks are also available as `mise run deno:check`,
`mise run deno:test`, and `mise run python:test`. The Deno tasks use only
built-ins; FastAPI is optional and is not needed by the Python tests.

Without mise, the equivalent Rust checks are:

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

To enable the repository-local pre-commit hook manually:

```sh
git config core.hooksPath .githooks
```

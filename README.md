# ikashita

ikashita is an MIT-licensed Rust/WASM-oriented low-code UI runtime. It is
delivered incrementally around a transport-neutral Resource Contract and a KDL
Application Profile.

This repository currently contains the standalone data/API increment and a
usable local CLI/runtime:

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
- `ikashita-cli` provides project scaffolding, deterministic validation,
  inspection/build output, and the localhost browser runtime.

The beginner-oriented documentation starts at [`docs/index.mdx`](docs/index.mdx)
and is rendered by the Astro Starlight site in [`docsite/`](docsite/). The
repository-level `docs/` directory is the single source of truth: the site
loads it directly and does not contain a copied content tree.

For the executable contract, see [`docs/spec.md`](docs/spec.md). For the
offline acceptance matrix and repository workflows, see
[`docs/usage.md`](docs/usage.md). The CLI and host adapters remain dependency
free at runtime; Ugoite integration is an adapter boundary, not a workspace
dependency.

The shortest path for a new user is the site's
[quick start](docs/guide/quickstart.mdx). It uses the checked-in
`examples/csv-readonly` fixture, so every command in that path is runnable
from this checkout.

## Tooling

Install [mise](https://mise.jdx.dev/) and run commands from the repository
root. `mise.toml` pins Rust 1.94.0, Deno 2.8.3, Python 3.13.5, Node 22.14.0,
Ruff 0.16.1, uv 0.11.7, and the shared target directory. ty 0.0.65 is pinned
in `pyproject.toml` and `uv.lock`. The docsite uses npm with the committed
`docsite/package-lock.json`.
The setup task installs the locked Python quality environment and configures
the local Git hook; it does not require Docker or a browser.

```sh
mise install
mise run setup
mise run fmt:check
mise run lint
mise run check
mise run test
mise run docs:check
mise run ci

# Documentation site only
mise run docs:install
mise run docs:check
mise run docs:build
```

Host-specific checks are also available as `mise run deno:check`,
`mise run deno:test`, `mise run python:lint`, `mise run python:typecheck`, and
`mise run python:test`. The Deno tasks use only built-ins; FastAPI is optional
and is not needed by the Python tests. The same `mise run ci` task is executed
by GitHub Actions.

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

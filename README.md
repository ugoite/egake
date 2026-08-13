# egake

egake is an MIT-licensed Rust/WASM-oriented low-code UI runtime. It is
delivered incrementally around a transport-neutral Resource Contract and a KDL
Application Profile.

This repository currently contains the standalone data/API increment and a
usable local CLI/runtime:

- `egake-resource` defines the shared resource query, schema, page,
  structured-error types, generic provider trait, JSON provider boundary, and
  merge-patch helper.
- `egake-spec` owns the versioned Application Profile metadata.
- `egake-data` provides generic local data resources with CSV and Parquet backends.
- `egake-server` provides the localhost HTTP router, provider registry, and
  static-bundle configuration.
- `egake-cli` provides the versioned command-line entry point.
- `packages/runtime` provides the Egake-side dependency-free Deno/TypeScript
  ResourceProvider client and merge-patch helper.
- `packages/ikasue` is the UI runtime: the versioned `IkaView` contract,
  Custom Elements, semantic DOM events, geometry, interaction, and theme.
- `python/egake` provides the standard-library Resource protocol/base class,
  ASGI adapter, and optional FastAPI bridge.
- `egake-cli` provides project scaffolding, deterministic validation,
  inspection/build output, and the localhost browser runtime.

The beginner-oriented documentation starts at [`docs/index.mdx`](docs/index.mdx)
and is rendered by the Astro Starlight site in [`docsite/`](docsite/). The
repository-level `docs/` directory is the single source of truth: the site
loads it directly and does not contain a copied content tree.

For the executable contract, see [`docs/spec.md`](docs/spec.md). For the
offline acceptance matrix and repository workflows, see
[`docs/usage.md`](docs/usage.md). Egake owns application/data behavior and
lowers KDL to `IkaView + bindings`; Ikasue owns UI vocabulary, rendering,
geometry, interaction, accessibility, and theme. Ugoite remains a host
boundary.

The shortest path for a new user is the site's
[quick start](docs/guide/quickstart.mdx). It uses the checked-in
`examples/csv-readonly` fixture, so every command in that path is runnable
from this checkout.

## CLI quick start

From the repository root, scaffold or use the checked-in example:

```sh
cargo run -p egake-cli -- new my-contacts
cargo run -p egake-cli -- validate my-contacts
cargo run -p egake-cli -- build my-contacts
cargo run -p egake-cli -- build my-contacts --format single-html --output dist/contacts.html
cargo run -p egake-cli -- run my-contacts
cargo run -p egake-cli -- list examples/csv-readonly --resource catalog --query ada
```

The commands are `new`, `validate`, `inspect`, `build`, `run`, `dev`, `test`,
and `list`. `validate`, `inspect`, `build`, `test`, and `list` accept `--json`; project
directories may be positional or passed with `--project`. `run` and `dev`
listen on `127.0.0.1:8787` by default. A non-loopback `--host` requires the
explicit `--allow-external` flag and prints a warning because this MVP has no
authentication. CORS is disabled by default.

Projects contain `egake.toml` and `app.ui.kdl`. Resource providers use
exactly one configuration source: `resources.kdl` when present, otherwise
`[resources.<name>]` tables in TOML. Supplying both is an error; they are never
merged or overridden. The preferred KDL convention is:

```kdl
/- kdl-version 2
resources {
    resource "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

The format is inferred from `.csv` or `.parquet`; use `format="csv"` or
`format="parquet"` when a path has no supported extension. CSV resources can
be writable, while Parquet resources are read-only.

Paths are project-relative and may not contain `..`. `dev` is a validated
no-watch development server; restart it after editing source files.
`actions.rhai` is emitted
by `new` as a documentation placeholder only; this CLI does not execute Rhai,
shell commands, or arbitrary JavaScript. `build` writes `dist/index.html`,
`ikasue.js`, `ikasue.css`, `egake.js`, and `app.bundle.json`. The bundle
contains `views` and Egake-owned `bindings`, not provider data or credentials.

Use `build --format single-html` (or `build --single-html`) for a one-file
artifact. With the default `--output dist`, it writes `dist/index.html`; when
the output ends in `.html`, that path is used directly. CSS, runtime JavaScript,
and application metadata are inline, with a CSP hash for the executable/style
blocks and a non-executable `application/json` script for metadata. JSON
characters that could close a script element, including `<`, U+2028, and
U+2029, are escaped before embedding. `run` and `dev` retain the directory-style
in-memory bundle and API behavior.

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

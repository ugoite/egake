# ikashita

ikashita is an MIT-licensed Rust/WASM-oriented low-code UI runtime. The
project is being delivered incrementally around a transport-neutral Resource
Contract and a KDL Application Profile.

This repository currently contains the standalone data/API increment and a
usable local CLI/runtime:

- `ikashita-resource` defines the shared resource query, schema, page,
  structured-error types, generic provider trait, JSON provider boundary, and
  merge-patch helper.
- `ikashita-spec` owns the versioned Application Profile metadata.
- `ikashita-csv` provides a locked, atomic-write local CSV Resource Provider.
- `ikashita-server` provides the localhost HTTP router, provider registry, and
  static-bundle configuration.
- `ikashita-cli` provides project scaffolding, deterministic validation,
  inspection/build output, and the localhost browser runtime.

The executable MVP decisions are recorded in [`docs/spec.md`](docs/spec.md).
Ugoite integration will be an adapter boundary and is not a workspace
dependency.

## CLI quick start

From the repository root, scaffold or use the checked-in example:

```sh
cargo run -p ikashita-cli -- new my-contacts
cargo run -p ikashita-cli -- validate my-contacts
cargo run -p ikashita-cli -- build my-contacts
cargo run -p ikashita-cli -- run my-contacts
```

The commands are `new`, `validate`, `inspect`, `build`, `run`, `dev`, and
`test`. `validate`, `inspect`, `build`, and `test` accept `--json`; project
directories may be positional or passed with `--project`. `run` and `dev`
listen on `127.0.0.1:8787` by default. A non-loopback `--host` requires the
explicit `--allow-external` flag and prints a warning because this MVP has no
authentication. CORS is disabled by default.

Projects contain `ikashita.toml` and `app.ui.kdl`. Resource providers are
declared in either `resources.kdl` or `[resources.<name>]` tables in the TOML
file, but not both. The preferred KDL convention is:

```kdl
/- kdl-version 2
resources {
    csv "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

Paths are project-relative and may not contain `..`. `actions.rhai` is emitted
by `new` as a documentation placeholder only; this CLI does not execute Rhai,
shell commands, or arbitrary JavaScript. `build` writes `dist/index.html`,
`runtime.js`, `runtime.css`, and `app.bundle.json`. The bundle contains the
validated application definition, not provider data or credentials.

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

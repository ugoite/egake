# Contributing

Thanks for helping build ikashita. Keep changes focused on the current
increment and preserve the adapter boundaries described in
[`docs/spec.md`](docs/spec.md).

## Local validation

Use mise from the repository root:

```sh
mise run setup
mise run fmt
mise run ci
```

The documentation site is built from the repository-level `docs/` directory;
do not copy pages into `docsite/src/content/`. Its locked Node workflow is:

```sh
mise run docs:install
mise run docs:fmt:check
mise run docs:check
mise run docs:build
```

`mise run install-hooks` configures `.githooks/pre-commit`. The hook runs
formatting, workspace checks, and tests with Cargo offline; it never downloads
dependencies or browser tooling. If a change introduces a dependency, update
the lockfile intentionally and update `THIRD_PARTY_NOTICES.md` as needed.

Host changes should also pass `mise run deno:fmt:check`,
`mise run deno:check`, `mise run deno:test`, and `mise run python:test`.

Please use concise Conventional Commit messages, keep public Rust APIs
documented, and add tests for observable behavior. Do not commit generated
`target/`, `docsite/node_modules/`, or `docsite/dist/` output. When a user-facing
behavior changes, update the canonical page in `docs/` and, when it changes the
executable contract, update `docs/spec.md` as well.

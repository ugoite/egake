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

`mise run install-hooks` configures `.githooks/pre-commit`. The hook runs
formatting, workspace checks, and tests with Cargo offline; it never downloads
dependencies or browser tooling. If a change introduces a dependency, update
the lockfile intentionally and update `THIRD_PARTY_NOTICES.md` as needed.

Host changes should also pass `mise run deno:fmt:check`,
`mise run deno:check`, `mise run deno:test`, and `mise run python:test`.

Please use concise Conventional Commit messages, keep public Rust APIs
documented, and add tests for observable behavior. Do not commit generated
`target/` or `dist/` output.

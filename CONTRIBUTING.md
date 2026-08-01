# Contributing

Thanks for helping build ikashita. Keep changes focused on the current
increment and preserve the adapter boundaries described in
[`docs/spec.md`](docs/spec.md).

## Local validation

Use the pinned toolchain from the repository root:

```sh
mise install
mise run setup
mise run fmt:check
mise run lint
mise run check
mise run test
mise run docs:check
mise run ci
```

`mise run setup` installs the locked Python quality environment and configures
`.githooks/pre-commit`. The hook is intentionally limited to formatting,
Rust/Deno checks, and Python Ruff/ty checks so it stays suitable for frequent
commits. It fails if a required tool is unavailable; there are no optional
success paths for quality checks.

The top-level tasks have these responsibilities:

- `mise run fmt:check`: Rust, Deno, and Python formatting.
- `mise run lint`: Rust Clippy, Deno lint, and `ruff check`.
- `mise run check`: Rust check, Deno check, and `ty check`.
- `mise run test`: Rust, Deno, and Python tests.
- `mise run docs:check`: local Markdown links, plus
  `mise run docsite:check` when a future docsite worktree is materialized at
  `docsite/`; that worktree must provide its own pinned `mise.toml` and
  `check` task.
- `mise run ci`: all of the above plus the Rust workspace build.

Python uses Ruff `0.16.1` from mise and ty `0.0.65` from the locked uv
development environment. Run `mise run python:install` after intentional
changes to `pyproject.toml`; commit the resulting `uv.lock` update. The
FastAPI bridge remains an optional runtime integration and is not required by
the standard-library test suite.

Please use concise Conventional Commit messages, keep public Rust APIs
documented, and add tests for observable behavior. Do not commit generated
`target/`, `.venv/`, or `dist/` output. If a change introduces a dependency,
update the relevant lockfile intentionally and update
`THIRD_PARTY_NOTICES.md` as needed.

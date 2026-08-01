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
- `mise run docs:check`: local Markdown links and the Starlight site check;
  `mise run docs:build` produces the static site from the canonical `docs/`.
- `mise run ci`: all of the above plus the Rust workspace build.

## GitHub automation

The `CI` and `Quality` workflows run the pinned repository checks on pushes and
pull requests. `Public safety` audits tracked and reachable history for
generated/local-only paths and high-confidence credential, private-key, and
token patterns. The `Deploy documentation to GitHub Pages` workflow runs for
each push to `main` (and on manual dispatch), builds `docs/` through
`mise run docs:build`, and publishes the site at
<https://ugoite.github.io/ikashita/>.

This repository intentionally has no release workflow. Releases, tags, and
package publication remain explicit, separately reviewed operations.

Python uses Ruff `0.16.1` from mise and ty `0.0.65` from the locked uv
development environment. Run `mise run python:install` after intentional
changes to `pyproject.toml`; commit the resulting `uv.lock` update. The
FastAPI bridge remains an optional runtime integration and is not required by
the standard-library test suite.

Please use concise Conventional Commit messages, keep public Rust APIs
documented, and add tests for observable behavior. Do not commit generated
`target/`, `docsite/node_modules/`, or `docsite/dist/` output. When a user-facing
behavior changes, update the canonical page in `docs/` and, when it changes the
executable contract, update `docs/spec.md` as well.
Do not commit `.venv/`, `dist/`, or generated cache output. If a change
introduces a dependency, update the relevant lockfile intentionally and update
`THIRD_PARTY_NOTICES.md` as needed.

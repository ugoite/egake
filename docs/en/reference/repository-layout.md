---
title: Repository and documentation site layout
description: Where the code, examples, canonical docs, and Starlight build shell live.
sidebar:
  label: Repository layout
---

<!-- i18n-sync: id=reference/repository-layout digest=65c58e2105cf7da99e53e17c2835ef0db59afdf8428f71914d9a6d0c24a8a837 -->

```text
.
├── crates/                 Rust Resource / spec / CSV / server / CLI
├── packages/               Ikasue UI runtime and Egake host runtime
├── python/                 stdlib Resource and ASGI/FastAPI bridge
├── examples/               Offline fixtures
├── docs/                   Canonical documentation read by GitHub and the site
├── docsite/                Astro Starlight build shell and SSOT symlink
└── mise.toml               Pinned Rust/Deno/Python/Node tasks
```

## Documentation single source of truth

`docsite/src/content/docs` is a symlink to `docs/`. Starlight’s `docsLoader()` reads that canonical tree, while `docsite/src/docs-ssot.mjs` gives the processing and sidebar configuration the same path. The site does not copy page bodies.

The following check verifies this arrangement:

```sh
mise run docs:check
```

It checks the `docsLoader()` configuration, the exact symlink target, title frontmatter, locale pairs, and the Astro type check.

## Where changes belong

- Implementation rules, numbers, routes, and errors → [`docs/spec.md`](../../spec/).
- Executable workflows and acceptance → [`docs/usage.md`](../../usage/).
- Beginner explanations → `docs/guide/`; link to the specification instead of redefining it.
- Site configuration, sidebar, and CSS → `docsite/`; product behavior does not belong there.

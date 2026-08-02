---
title: Repository and documentation site layout
description: Where the code, examples, canonical docs, and Starlight build shell live.
sidebar:
  label: Repository layout
---

<!-- i18n-sync: id=reference/repository-layout digest=014f937eded5a318d59b5dec2f6db8d295ea5b411c56dea24544c8445ac06c87 -->

```text
.
├── crates/                 Rust Resource / spec / CSV / server / CLI
├── packages/               Deno runtime and React/Vue adapters
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

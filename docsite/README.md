# egake documentation site

This directory is the Astro Starlight build shell. The authored documentation
lives in the repository-level [`../docs/`](../docs/) directory. The
`src/content/docs` entry is a symlink to that directory, so Starlight can use
its standard `docsLoader()` and MDX components without creating a second copy
of the pages.

Run from the repository root:

```sh
mise run docs:install
mise run docs:check
mise run docs:build
```

`docs:check` verifies the symlink target and the Starlight frontmatter before
running `astro check`. `docs:build` is a static, local-assets-only build. The
generated `dist/`, `.astro/`, and `node_modules/` directories are ignored and
must not be committed.

---
title: What is egake?
description: The purpose of egake and the responsibilities of UI definitions, data contracts, and hosts.
sidebar:
  label: What is egake?
---

<!-- i18n-sync: id=guide/what-is-egake digest=82059ad20418ce24f035222b1ca0cd99ced19f898d1c6dac410ceabf6d90ccb7 -->

egake is a Rust/WASM-oriented low-code UI runtime that keeps the **screen definition** separate from **how data is read and written**.

You can begin with a CSV table, then pass the same Resource Contract to a JavaScript host, Python ASGI app, Ugoite client, or framework adapter. Fix the data boundary first; choose the presentation host second.

## Three responsibilities

<div class="egake-diagram" role="img" aria-label="An Application Profile becomes a bundle and displays provider data through the host">
  <div><strong>Application Profile</strong><br />`app.ui.kdl` — screen, state, action, and resource declarations</div>
  <div class="arrow" aria-hidden="true">↓ validate / build</div>
  <div><strong>Static bundle</strong><br />Application metadata and schema metadata; no records or credentials</div>
  <div class="arrow" aria-hidden="true">↓ provider injection / HTTP</div>
  <div><strong>Resource Provider</strong><br />CSV, an existing API, Ugoite, or Python implements the Resource Contract</div>
</div>

The separation keeps a UI definition independent of a database or authentication method. The provider validates data and owns authentication and authorization. The MVP does not add authentication automatically.

## What is defined?

| File / layer            | Responsibility                                 | Example                                       |
| ----------------------- | ---------------------------------------------- | --------------------------------------------- |
| `app.ui.kdl`            | KDL Application Profile v0.1                   | `page`, `data-table`, `action`                |
| schema JSON             | Field types, required fields, enum, and format | `schemas/catalog.schema.json`                 |
| `resources.kdl` or TOML | Provider connection configuration              | CSV path, `writable`                          |
| provider                | Real data and operations                       | `list`, `get`, `update`                       |
| `dist/`                 | `egake build` output                           | `index.html`, `runtime.js`, `app.bundle.json` |

## What egake does not do

- It does not execute `actions.rhai`; the file created by `new` is documentation-only.
- It does not use `eval`, arbitrary HTML injection, or CDN runtime assets.
- It does not put CSV records, cookies, or credentials in a bundle.
- It does not own authentication, URLs, or storage for Ugoite or FastAPI.

Next, read the [mental model](../mental-model/) and then run the [quickstart](../quickstart/).

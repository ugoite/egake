---
title: Ikasue Web ABI
description: The boundary between the Egake application runtime and ResourceProvider.
---

<!-- i18n-sync: id=guide/usage/framework-adapters digest=79dc23a8f35831c4958462d5992e8e8eae54d98b63cc1887988138b036d11ec3 -->

Egake has no framework-specific UI renderer adapters. It validates KDL and
lowers it to Ikasue `IkaView` values plus Egake-owned `bindings`. Ikasue renders
the same view through Custom Elements under `ikasue-web/1`.

## Boundary

Egake owns KDL, state, actions, schema, ResourceProvider, CRUD, and bindings.
Ikasue owns UI vocabulary, DOM rendering, keyboard behavior, accessibility,
theme, and DataGrid geometry.

`IkaView.props` never contains `resource`, `action`, a provider, or a fetch
client. Those values stay in bundle `bindings`; the Egake browser host handles
the semantic DOM events.

## Controlled DataGrid

`ika-data-grid` receives `columns`, `rows`, `total`, `loading`, and `error`, and
emits `ika-query`, `ika-select`, and `ika-edit`. `ika-query` is
`{ offset, limit, sort, filter? }`; `ika-edit` is
`{ rowId, columnId, value }`. Ikasue knows no ResourceProvider or DataSource.

The host sends `ika-query` to `ResourceProvider.list` and writes the returned
page back as properties. Ikasue calculates virtual-scroll offset/limit;
Egake owns data access, stale responses, and error handling.

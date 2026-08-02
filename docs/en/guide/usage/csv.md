---
title: Standalone CSV
description: Use local CSV as a read-only or writable resource with ikashita-cli.
sidebar:
  label: Standalone CSV
---

<!-- i18n-sync: id=guide/usage/csv digest=66f7e52c036b2b7af7b6b027f31c9fd675937c1bf771bad956a4d319fd20b7c5 -->

CSV is the smallest way to exercise the provider boundary. The checked-in [`examples/csv-readonly`](https://github.com/ikashita/ikashita/tree/main/examples/csv-readonly) shape contains `ikashita.toml`, `app.ui.kdl`, `resources.kdl`, schema JSON, and CSV data.

## Minimal read-only layout

```text
examples/csv-readonly/
├── app.ui.kdl
├── ikashita.toml
├── resources.kdl
├── schemas/catalog.schema.json
└── data/catalog.csv
```

`resources.kdl` uses a project-relative path.

```kdl
/- kdl-version 2
resources {
    csv "catalog" path="data/catalog.csv"
}
```

The resource declaration in `app.ui.kdl` names the schema and required capability.

```kdl
resource "catalog" schema="schemas/catalog.schema.json" {
    require "list"
}
```

A read-only CSV without an `id` advertises only `schema` and `list`. Do not describe `get`, `create`, `update`, or `delete` when the provider cannot implement them.

## Commands

```sh
cargo run -p ikashita-cli -- validate examples/csv-readonly
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada --sort title
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --json
cargo run -p ikashita-cli -- test examples/csv-readonly
```

Search is a case-insensitive substring match across fields. Sorting is stable lexicographic order. `--offset` and `--limit` follow the Resource Contract pagination rules.

## Moving to writable CSV

Writable CSV needs a fixed schema and a primary key.

```kdl
/- kdl-version 2
resources {
    csv "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

The provider rejects duplicate or empty `id` values, undeclared columns, and path traversal. `update` is RFC 7396 merge-patch, not full replacement. See the [CSV provider specification](../../../spec/#csv-provider).

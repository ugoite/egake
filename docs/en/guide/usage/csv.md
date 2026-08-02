---
title: Local data provider
description: Use local CSV or Parquet resources with ikashita-cli.
sidebar:
  label: Local data
---

<!-- i18n-sync: id=guide/usage/csv digest=87d3eeca7d8bc8a13da235ea09e1ed684f6b79a4202a161b89663b3176cb5c7e -->

The local data provider is the smallest way to exercise the provider boundary. The checked-in [`examples/csv-readonly`](https://github.com/ugoite/ikashita/tree/main/examples/csv-readonly) shape contains `ikashita.toml`, `app.ui.kdl`, `resources.kdl`, schema JSON, and CSV data. A `.parquet` extension opens the same kind of resource as read-only Parquet.

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
    resource "catalog" path="data/catalog.csv"
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

## Explicit Parquet format

When the extension is unavailable or you want an explicit setting, specify `format`.

```kdl
/- kdl-version 2
resources {
    resource "catalog" path="data/catalog.parquet" format="parquet"
}
```

Parquet resources are read-only. Arrow columns become JSON numbers, booleans, arrays, or objects; a string `id` column also advertises `get`.

## Moving to writable CSV

Writable CSV needs a fixed schema and a primary key.

```kdl
/- kdl-version 2
resources {
    resource "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

The data provider rejects duplicate or empty `id` values, undeclared columns, and path traversal. CSV `update` is RFC 7396 merge-patch, not full replacement. See the [local data provider specification](../../../spec/#local-data-provider).

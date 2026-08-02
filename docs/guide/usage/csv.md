---
title: local data provider
description: ikashita-cliでローカルCSVまたはParquetをresourceとして使う。
sidebar:
  label: local data
---

<!-- i18n-sync: id=guide/usage/csv digest=87d3eeca7d8bc8a13da235ea09e1ed684f6b79a4202a161b89663b3176cb5c7e -->

ローカルdata providerは、provider境界を最小のファイルで試す入口です。既存の[`examples/csv-readonly`](https://github.com/ugoite/ikashita/tree/main/examples/csv-readonly)相当の構成は、`ikashita.toml`、`app.ui.kdl`、`resources.kdl`、schema JSON、CSVからなります。拡張子が`.parquet`なら同じ設定でParquetをread-onlyで開けます。

## read-onlyの最小構成

```text
examples/csv-readonly/
├── app.ui.kdl
├── ikashita.toml
├── resources.kdl
├── schemas/catalog.schema.json
└── data/catalog.csv
```

`resources.kdl`はpathをプロジェクト相対で指定します。

```kdl
/- kdl-version 2
resources {
    resource "catalog" path="data/catalog.csv"
}
```

`app.ui.kdl`側のresource宣言は、schemaと必要capabilityを明示します。

```kdl
resource "catalog" schema="schemas/catalog.schema.json" {
    require "list"
}
```

`id`列がないread-only CSVでは`schema`と`list`だけが広告されます。`get`、`create`、`update`、`delete`を説明に足してもproviderが対応しないため、宣言しません。

## 実行できるコマンド

```sh
cargo run -p ikashita-cli -- validate examples/csv-readonly
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --query ada --sort title
cargo run -p ikashita-cli -- list examples/csv-readonly --resource catalog --json
cargo run -p ikashita-cli -- test examples/csv-readonly
```

`list`の検索は全フィールドに対するcase-insensitive substringです。sortは安定したlexicographic sortで、`--offset`と`--limit`はResource Contractのpaginationに従います。

## formatを明示するParquet

拡張子から判定できない場合や設定を明示したい場合は`format`を指定します。

```kdl
/- kdl-version 2
resources {
    resource "catalog" path="data/catalog.parquet" format="parquet"
}
```

Parquet resourceはread-onlyで、列のArrow型をJSONのnumber、boolean、配列、objectなどへ変換します。`id`列が文字列なら`get`も広告されます。

## writable CSVに進むとき

書き込みを有効にする場合は、固定列とprimary keyが必要です。

```kdl
/- kdl-version 2
resources {
    resource "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

data providerは`id`の重複・空値、schemaにない不足列、path traversalを許可しません。CSVの`update`は完全置換ではなくRFC 7396 merge-patchです。詳細は[local data providerの仕様](../../../spec/#local-data-provider)を参照してください。

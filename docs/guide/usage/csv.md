---
title: standalone CSV
description: ikashita-cliでローカルCSVをread-onlyまたはwritable resourceとして使う。
sidebar:
  label: standalone CSV
---

<!-- i18n-sync: id=guide/usage/csv digest=66f7e52c036b2b7af7b6b027f31c9fd675937c1bf771bad956a4d319fd20b7c5 -->

CSVは、provider境界を最小のファイルで試す入口です。既存の[`examples/csv-readonly`](https://github.com/ikashita/ikashita/tree/main/examples/csv-readonly)相当の構成は、`ikashita.toml`、`app.ui.kdl`、`resources.kdl`、schema JSON、CSVからなります。

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
    csv "catalog" path="data/catalog.csv"
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

## writable CSVに進むとき

書き込みを有効にする場合は、固定列とprimary keyが必要です。

```kdl
/- kdl-version 2
resources {
    csv "contacts" path="data/contacts.csv" key="id" writable=#true backup-count=2
}
```

CSV providerは`id`の重複・空値、schemaにない不足列、path traversalを許可しません。`update`は完全置換ではなくRFC 7396 merge-patchです。詳細は[CSV providerの仕様](../../../spec/#csv-provider)を参照してください。

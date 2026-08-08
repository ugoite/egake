---
title: egakeとは何か
description: egakeの目的と、UI定義・データ契約・ホストの責任分担。
sidebar:
  label: egakeとは何か
---

<!-- i18n-sync: id=guide/what-is-egake digest=82059ad20418ce24f035222b1ca0cd99ced19f898d1c6dac410ceabf6d90ccb7 -->

egakeは、**画面の定義**と**データを読む・書く方法**を別々に扱う、Rust/WASM志向のlow-code UI runtimeです。

「CSVをそのまま一覧にする」ことから始められますが、同じResource ContractをJavaScript、Python ASGI、Ugoiteなどの既存ホストにも渡せます。最初から大きなアプリケーションフレームワークを選ぶのではなく、データ境界を固定してから表示方法を選ぶのが基本です。

## 3つの役割

<div class="egake-diagram" role="img" aria-label="KDLのApplication Profileがbundleになり、host providerを通じてデータを画面に表示する関係">
  <div><strong>Application Profile</strong><br />`app.ui.kdl` — 画面、状態、action、resource宣言</div>
  <div class="arrow" aria-hidden="true">↓ validate / build</div>
  <div><strong>静的bundle</strong><br />アプリ定義とschema metadata。レコードや資格情報は含めない</div>
  <div class="arrow" aria-hidden="true">↓ provider injection / HTTP</div>
  <div><strong>Resource Provider</strong><br />CSV、既存API、Ugoite、PythonなどがResource Contractを実装</div>
</div>

この分離により、UI定義はデータベースや認証方式に引きずられません。逆に、providerは自分のデータ検証と認証・認可を担当します。egakeのMVPは認証を自動で追加しません。

## 何を定義するのか

| ファイル / 層               | 役割                                 | 例                                            |
| --------------------------- | ------------------------------------ | --------------------------------------------- |
| `app.ui.kdl`                | Application Profile v0.1             | `page`、`data-table`、`action`                |
| schema JSON                 | フィールド型、required、enum、format | `schemas/catalog.schema.json`                 |
| `resources.kdl` または TOML | providerの接続設定                   | CSV path、`writable`                          |
| provider                    | 実データと操作                       | `list`、`get`、`update`                       |
| `dist/`                     | `egake build`の生成物                | `index.html`、`runtime.js`、`app.bundle.json` |

## egakeがしないこと

- `actions.rhai`を実行しない。`new`が作るファイルは説明用のplaceholderです。
- `eval`、任意のHTML注入、CDNのruntime読み込みをしない。
- bundleにCSVレコード、cookie、credentialを埋め込まない。
- UgoiteやFastAPIなどの認証・URL・保存方式を所有しない。

次は[メンタルモデル](../mental-model/)でデータの流れを把握し、その後[最短クイックスタート](../quickstart/)で実際に確認してください。

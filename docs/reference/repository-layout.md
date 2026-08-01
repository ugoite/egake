---
title: リポジトリとドキュメントサイトの構成
description: docs/を正本にした、ikashitaのコード・example・docsiteの配置。
sidebar:
  label: リポジトリ構成
---

```text
.
├── crates/                 RustのResource / spec / CSV / server / CLI
├── packages/               Deno runtimeとReact/Vue adapter
├── python/                 stdlib ResourceとASGI/FastAPI bridge
├── examples/               オフラインで実行できるfixture
├── docs/                   正本。GitHubとdocsiteが直接読む
├── docsite/                Astro Starlightのbuild shellとSSOT symlink
└── mise.toml               Rust/Deno/Python/Nodeと検証taskの固定点
```

## 文書のsingle source of truth

`docsite/src/content/docs`は`docs/`へのsymlinkです。`docsite/src/content.config.ts`のStarlight `docsLoader()`はこのsymlinkを通じて正本を読み、`docsite/src/docs-ssot.mjs`は追加の処理対象とsidebarが同じパスを使うためのhelperです。本文をsiteへコピーする方式ではありません。

この構成を壊していないことは次のcheckで機械的に確認します。

```sh
mise run docs:check
```

checkは、content configが`docsLoader()`を使うこと、symlinkが正確に`docs/`を指すこと、`docs/`の全MarkdownにStarlight `title` frontmatterがあることを確認してから`astro check`を実行します。

## 変更の置き場所

- 実装の仕様・数値・route・error → [`docs/spec.md`](../spec.md)。
- 実行コマンドとacceptance → [`docs/usage.md`](../usage.md)。
- 初心者向けの説明 → `docs/guide/`。仕様を再定義せず、正本へのリンクを置く。
- サイト設定・sidebar・CSS → `docsite/`。製品仕様の本文は置かない。

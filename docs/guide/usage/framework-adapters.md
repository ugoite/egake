---
title: React / Vue / Solid / Svelte adapter
description: 実装済みのReact/Vue adapterと、Solid/Svelteで守るべきgeneric runtime境界。
sidebar:
  label: Framework adapters
---

<!-- i18n-sync: id=guide/usage/framework-adapters digest=8882a03a29bcaf6830561af491fb3cb739e31f6077a39e8628c3c8b0c4f1be6f -->

framework adapterは、Application Profile JSONをそのframeworkのelement/VNodeへ変換する薄い層です。framework本体をegakeが依存に追加せず、hostからrender primitiveを受け取る設計です。

## 現在の対応状況

| Framework | このcheckoutの状態 | 公開識別子                                            |
| --------- | ------------------ | ----------------------------------------------------- |
| React     | adapter実装済み    | `createReactRenderer` / `createReactResourceProvider` |
| Vue       | adapter実装済み    | `createVueRenderer` / `createVueResourceProvider`     |
| Solid     | 専用adapter未提供  | `packages/runtime`をDOM lifecycleへ接続する           |
| Svelte    | 専用adapter未提供  | `packages/runtime`をDOM lifecycleへ接続する           |

Solid/Svelteについては、存在しないnpm package名やimportを案内しません。専用adapterが必要なら、既存のruntimeの`SerializedApplication`、`SerializedComponent`、`ResourceProvider`の契約を使って、各frameworkの正式なmount lifecycleに薄い変換層を実装してください。

## React

Reactはhostから`createElement`を渡します。adapter自身はReactをimportしません。

```ts
import {
  createReactRenderer,
  createReactResourceProvider,
} from "./packages/react/mod.ts";

const contacts = createReactResourceProvider(client, "contacts");
const renderApplication = createReactRenderer(React, {
  onAction: (action) => void contacts.invoke(action, null),
});
const element = renderApplication(applicationJson);
```

実際のadapterは`packages/react/src/index.ts`、READMEは`packages/react/README.md`です。childrenは通常のReact childrenで渡され、`dangerouslySetInnerHTML`は使いません。

## Vue

Vueはhostから`h`を渡します。

```ts
import {
  createVueRenderer,
  createVueResourceProvider,
} from "./packages/vue/mod.ts";

const contacts = createVueResourceProvider(client, "contacts");
const renderApplication = createVueRenderer(Vue, {
  onAction: (action) => void contacts.invoke(action, null),
});
const vnode = renderApplication(applicationJson);
```

ここで`Vue`は`h`を実装するhostの値です。adapter packageはVueを依存に含めません。実装のsourceは`packages/vue/src/index.ts`です。

## Solid / Svelteでの考え方

このincrementで提供するのは専用adapterではなく、generic runtimeとの境界です。

1. `application.json`またはCLIが作ったdata-only bundleをhostで読み込む。
2. `ResourceProvider`を作り、Solid/Svelteのstore・load関数・API clientをそこへ写像する。
3. DOM rootの生成・破棄をframeworkのlifecycleで所有する。
4. providerの`capabilities`とstructured errorを維持し、HTML文字列へ変換しない。

この段階で必要な安全性と契約の詳細は[TypeScript/Deno browser runtime](../../../spec/#typescriptdeno-browser-runtime)にあります。専用adapterの追加は、実装とtestを同じcommitに含めてから、この表と仕様を更新します。

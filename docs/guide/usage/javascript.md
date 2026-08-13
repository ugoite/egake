---
title: JS埋め込み
description: Deno/TypeScriptのhostがResourceProviderを注入してegakeを動かす。
sidebar:
  label: JS埋め込み
---

<!-- i18n-sync: id=guide/usage/javascript digest=448b1304a1f2741f457958683337027568407bf11bd9d08cfe18ab8393d3f2c9 -->

JavaScript埋め込みでは、アプリケーション定義とproviderを分けます。`packages/runtime`はResource Contractだけを提供し、hostがprovider mapとEgake action loopを所有します。UIを使う場合、`packages/ikasue`がIkaViewをCustom Elementsへlowerし、hostはsemantic DOM eventを処理します。

## チェックアウト済みexampleを確認する

```sh
deno check examples/js-embedded/main.ts
deno test examples/js-embedded/main_test.ts
```

`examples/js-embedded/main.ts`の`createEmbeddedProvider()`は`status` resourceに`schema`、`list`、`invoke`だけを広告します。providerはデータアクセスとprovider actionだけを担当し、アプリケーション側のaction loopはhostが担当します。

```ts
const capabilities: readonly Capability[] = ["schema", "list", "invoke"];

const provider: ResourceProvider = {
  schema: () => ({ name: "status", fields: [], capabilities }),
  list: (query) => ({
    items,
    total: items.length,
    offset: query.offset,
    limit: query.limit,
  }),
  get: () => unsupported("get"),
  create: () => unsupported("create"),
  update: () => unsupported("update"),
  delete: () => unsupported("delete"),
  invoke: (action, input) => ({ ok: true, action, input }),
};
```

上はproviderの責任範囲を示す実装の一部です。実際の完全なfixtureは`examples/js-embedded/main.ts`にあります。

## UI runtimeへ接続する

ブラウザでは、hostが生成済みbundleをロードします。Egakeはprovider、state、action loopを所有し、IkasueへIkaViewとbindingを渡します。Ikasueは同じIkaViewをCustom Elementsへlowerし、`ika-query`、`ika-select`、`ika-edit`、`ika-action`などのsemantic DOM eventをhostへ返します。

独自hostからIkasueを使う場合は、`packages/ikasue`の`renderIkaView`とCustom Element propertiesを利用してください。DOM ABIはWeb Platformそのものであり、Egake専用のUI adapterやSerializedComponentはありません。

## HTTP providerを使う場合

同じruntimeには`ResourceClient`もあります。API rootはsame-originの相対pathだけを受け付け、request ID、query、schema、structured errorを検証します。HTTP routeの正本は[Standalone HTTP adapter](../../../spec/#standalone-http-adapter)です。

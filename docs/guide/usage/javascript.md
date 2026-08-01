---
title: JS埋め込み
description: Deno/TypeScriptのhostがResourceProviderを注入してikashitaを動かす。
sidebar:
  label: JS埋め込み
---

JavaScript埋め込みでは、アプリケーション定義とproviderを分けます。`packages/runtime`はDeno/TypeScriptのbuilt-inだけで動き、hostがprovider mapを所有します。

## チェックアウト済みexampleを確認する

```sh
deno check examples/js-embedded/main.ts
deno test examples/js-embedded/main_test.ts
```

`examples/js-embedded/main.ts`の`createEmbeddedProvider()`は`status` resourceに`schema`、`list`、`invoke`だけを広告します。`runEmbeddedAction()`はApplication Profileの`invoke` stepを読み、注入されたproviderの`invoke`だけを呼びます。

```ts
const capabilities: readonly Capability[] = ["schema", "list", "invoke"];

const provider: ResourceProvider = {
  schema: () => ({ name: "status", fields: [], capabilities }),
  list: (query) => ({ items, total: items.length, offset: query.offset, limit: query.limit }),
  get: () => unsupported("get"),
  create: () => unsupported("create"),
  update: () => unsupported("update"),
  delete: () => unsupported("delete"),
  invoke: (action, input) => ({ ok: true, action, input }),
};
```

上はproviderの責任範囲を示す実装の一部です。実際の完全なfixtureは`examples/js-embedded/main.ts`にあります。

## DOMへmountする

実行時にはhostがbundleをロードし、root elementとprovider mapを渡します。

```ts
startIkashitaHost(
  document.getElementById("app")!,
  application,
  { status: createEmbeddedProvider() },
);
```

この呼び出しはexampleに定義された`startIkashitaHost`の形です。runtimeはDOM APIで要素を作り、値を`textContent`やDOM propertyへ渡します。任意のHTML文字列、`eval`、remote assetは使いません。

## HTTP providerを使う場合

同じruntimeには`ResourceClient`もあります。API rootはsame-originの相対pathだけを受け付け、request ID、query、schema、structured errorを検証します。HTTP routeの正本は[Standalone HTTP adapter](../../spec.md#standalone-http-adapter)です。

---
title: Ikasue Web ABI
description: EgakeのUI runtimeとResourceProviderの境界。
---

<!-- i18n-sync: id=guide/usage/framework-adapters digest=79dc23a8f35831c4958462d5992e8e8eae54d98b63cc1887988138b036d11ec3 -->

EgakeはUIを描画する別のframework adapterを持ちません。KDLを検証し、
Ikasueの`IkaView`とEgakeの`bindings`へlowerします。Ikasueは
`ikasue-web/1`のCustom Elementsとして同じviewを描画します。

## 境界

Egakeが持つものはKDL、state、action、schema、ResourceProvider、CRUDと
bindingです。Ikasueが持つものはUI vocabulary、DOM rendering、keyboard、
accessibility、theme、DataGridのgeometryです。

`IkaView.props`には`resource`、`action`、provider、fetch clientを入れません。
それらはbundleの`bindings`に残し、Egake hostがDOM eventを処理します。

## Controlled DataGrid

`ika-data-grid`は`columns`、`rows`、`total`、`loading`、`error`を受け取り、
`ika-query`、`ika-select`、`ika-edit`をemitします。`ika-query`は
`{ offset, limit, sort, filter? }`、`ika-edit`は`{ rowId, columnId, value }`
です。IkasueはResourceProviderやDataSourceを知りません。

hostは`ika-query`をResourceProvider.listへ渡し、返ったpageをpropertiesへ
戻します。virtual scrollのoffset/limit計算はIkasue、データ取得とstale/error
処理はEgakeです。

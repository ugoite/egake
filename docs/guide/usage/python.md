---
title: Python ASGI
description: 標準ライブラリのResourceASGIAppと任意のFastAPI bridgeを使う。
sidebar:
  label: Python ASGI
---

<!-- i18n-sync: id=guide/usage/python digest=6bcbdeb0e413e1409948e43aafce5bf6466e605411f1175f638359a7b4a68c0e -->

Python hostでは、`python/ikashita`がResource ContractをASGI appとして公開します。coreは標準ライブラリだけで動き、FastAPIは`create_fastapi_app`を呼んだときだけ必要です。

## 実行できるexample

```sh
PYTHONPATH=python python -m unittest discover -s python/tests -t python
PYTHONPATH=python python examples/python-fastapi/app.py
```

exampleの`Contacts(ResourceBase)`は、`schema`、`list`、`get`、`create`、`update`、`delete`、`invoke`を実装しています。`update`は`apply_merge_patch`を使うため、既存値の一部だけを更新できます。

```python
from ikashita import ResourceASGIApp

def create_asgi_app() -> ResourceASGIApp:
    return ResourceASGIApp({"contacts": Contacts()})
```

## FastAPIはoptional

FastAPIがインストール済みの環境だけで、同じproviderをbridgeできます。

```sh
PYTHONPATH=python uvicorn app:app --app-dir examples/python-fastapi
```

このcommandはoptional依存を自動インストールしません。未インストールでも標準ライブラリのtestは通り、認証middlewareが必要な場合はdeployment hostが追加します。ASGIのpath、request ID、body/query上限、structured errorは[Python host boundary](../../../spec/#hostruntime-adapters)を正本とします。

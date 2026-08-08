---
title: Python ASGI
description: Use the standard-library ResourceASGIApp and an optional FastAPI bridge.
sidebar:
  label: Python ASGI
---

<!-- i18n-sync: id=guide/usage/python digest=a65740996ce1cccb32f3e3e54fb1842c09034f92af87773a040b1b114c0c4177 -->

The `python/egake` package exposes the Resource Contract as an ASGI application. The core uses only the standard library; FastAPI is imported only when `create_fastapi_app` is called.

## Run the example

```sh
PYTHONPATH=python python -m unittest discover -s python/tests -t python
PYTHONPATH=python python examples/python-fastapi/app.py
```

The example’s `Contacts(ResourceBase)` implements `schema`, `list`, `get`, `create`, `update`, `delete`, and `invoke`. Its `update` uses `apply_merge_patch`, so a caller can change only part of an existing record.

```python
from egake import ResourceASGIApp

def create_asgi_app() -> ResourceASGIApp:
    return ResourceASGIApp({"contacts": Contacts()})
```

## FastAPI is optional

When FastAPI is installed, the same provider can be bridged:

```sh
PYTHONPATH=python uvicorn app:app --app-dir examples/python-fastapi
```

This command does not install optional dependencies. Standard-library tests pass without FastAPI, and a deployment host can add its own authentication middleware. ASGI paths, request IDs, body/query limits, and structured errors are defined in the [Python host boundary](../../../spec/#hostruntime-adapters).

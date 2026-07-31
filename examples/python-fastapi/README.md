# Optional FastAPI host

Set `PYTHONPATH=python` and run the dependency-free example probe with the
repository's Python command:

```sh
PYTHONPATH=python python3 -m unittest discover -s python/tests -t python
PYTHONPATH=python python3 examples/python-fastapi/app.py
```

`app.py` injects a standard-library `ResourceBase` into `ResourceASGIApp`.
When FastAPI is installed, `create_fastapi_application()` exposes the same
provider through `create_fastapi_app`; an ASGI server can run it with:

```sh
PYTHONPATH=python uvicorn app:app --app-dir examples/python-fastapi
```

The adapter does not implement auth; add host middleware when the deployment
needs it. The example's action is deterministic and has no network or
credential dependency.

The core tests use `ResourceASGIApp` and do not require FastAPI, network access,
or an installed optional dependency.

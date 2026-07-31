# Python host adapter

Set `PYTHONPATH=python` when using the package directly from this repository.
`ikashita.Resource` is the standard-library provider protocol and
`ikashita.ResourceBase` supplies an explicit abstract base with a structured
unsupported-invoke default. `ikashita.ResourceASGIApp` is the dependency-free
ASGI router. FastAPI support is optional and loaded only through
`ikashita.fastapi.create_fastapi_app`.

Core tests run with:

```sh
mise run python:test
```

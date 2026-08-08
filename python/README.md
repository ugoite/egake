# Python host adapter

Set `PYTHONPATH=python` when using the package directly from this repository.
`egake.Resource` is the standard-library provider protocol and
`egake.ResourceBase` supplies an explicit abstract base with a structured
unsupported-invoke default. `egake.ResourceASGIApp` is the dependency-free
ASGI router. FastAPI support is optional and loaded only through
`egake.fastapi.create_fastapi_app`.

Core tests run with:

```sh
mise run python:test
```

The reproducible Python quality gates are:

```sh
mise run python:install
mise run python:lint
mise run python:typecheck
```

Ruff is supplied by mise, while ty is installed from the pinned `uv.lock`
environment. Both commands fail when their tool is unavailable.

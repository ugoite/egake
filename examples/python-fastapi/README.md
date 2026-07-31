# Optional FastAPI host

Install FastAPI in the host environment, set `PYTHONPATH=python`, and run the
example with an ASGI server. `app.py` injects a standard-library
`ResourceBase` into `create_fastapi_app`. The adapter does not implement auth;
add host middleware when the deployment needs it.

The core tests use `ResourceASGIApp` and do not require FastAPI, network access,
or an installed optional dependency.

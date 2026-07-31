"""Optional FastAPI bridge.

The core adapter has no FastAPI dependency. This module imports FastAPI only
when ``create_fastapi_app`` is called in an environment that installed it.
"""

from __future__ import annotations

from typing import Any, Mapping

from .asgi import ResourceASGIApp
from .resource import Resource


def create_fastapi_app(resources: Mapping[str, Resource], title: str = "ikashita host") -> Any:
    """Create a FastAPI app that delegates the Resource Contract to ASGI.

    Authentication, authorization, and middleware remain the FastAPI host's
    responsibility and are intentionally outside this adapter.
    """

    try:
        from fastapi import FastAPI, Request, Response
    except ImportError as exc:  # pragma: no cover - depends on optional host setup
        raise RuntimeError("FastAPI integration requires the optional fastapi package") from exc

    adapter = ResourceASGIApp(resources)
    app = FastAPI(title=title)

    @app.api_route("/{path:path}", methods=["GET", "POST", "PATCH", "DELETE"], include_in_schema=False)
    async def resource_boundary(request: Request) -> Response:
        messages = []

        async def send(message: Any) -> None:
            messages.append(message)

        await adapter(request.scope, request.receive, send)
        start = next(item for item in messages if item["type"] == "http.response.start")
        body = next(item for item in messages if item["type"] == "http.response.body")["body"]
        headers = {key.decode("ascii"): value.decode("ascii") for key, value in start.get("headers", [])}
        return Response(content=body, status_code=start["status"], headers=headers, media_type=None)

    return app

"""Optional FastAPI host example; core ResourceASGIApp needs no FastAPI."""

from __future__ import annotations

from typing import Dict

from ikashita import (
    CAPABILITIES,
    FieldSchema,
    ListQuery,
    ResourceBase,
    ResourceASGIApp,
    ResourcePage,
    ResourceSchema,
    apply_merge_patch,
)
from ikashita.fastapi import create_fastapi_app


class Contacts(ResourceBase):
    def __init__(self) -> None:
        self.items: Dict[str, dict] = {"1": {"id": "1", "name": "Ada"}}

    def schema(self) -> ResourceSchema:
        return ResourceSchema(
            "contacts",
            (FieldSchema("id", "text", True), FieldSchema("name", "text")),
            CAPABILITIES,
        )

    def list(self, query: ListQuery) -> ResourcePage:
        values = list(self.items.values())
        if query.q:
            values = [item for item in values if query.q.lower() in item["name"].lower()]
        return ResourcePage(tuple(values[query.offset:query.offset + query.limit]), len(values), query.offset, query.limit)

    def get(self, resource_id: str):
        return self.items.get(resource_id)

    def create(self, value):
        self.items[value["id"]] = value
        return value

    def update(self, resource_id: str, merge_patch):
        self.items[resource_id] = apply_merge_patch(self.items[resource_id], merge_patch)
        return self.items[resource_id]

    def delete(self, resource_id: str) -> None:
        del self.items[resource_id]

    def invoke(self, action, value):
        return {"action": action, "input": value}


def create_asgi_app() -> ResourceASGIApp:
    """Build the dependency-free ASGI application used by the tests."""

    return ResourceASGIApp({"contacts": Contacts()})


def create_fastapi_application():
    """Build the optional FastAPI application around the same provider."""

    return create_fastapi_app({"contacts": Contacts()})


try:
    app = create_fastapi_application()
except RuntimeError:
    # The example remains importable and runnable with only the stdlib adapter.
    app = create_asgi_app()


if __name__ == "__main__":
    print("ASGI app ready: use `uvicorn app:app --app-dir examples/python-fastapi` with an ASGI server")

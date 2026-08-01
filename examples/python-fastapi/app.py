"""Optional FastAPI host example; core ResourceASGIApp needs no FastAPI."""

from __future__ import annotations

from typing import Any

from ikashita import (
    CAPABILITIES,
    FieldSchema,
    JsonObject,
    JsonValue,
    ListQuery,
    ResourceASGIApp,
    ResourceBase,
    ResourcePage,
    ResourceSchema,
    apply_merge_patch,
)
from ikashita.fastapi import create_fastapi_app


class Contacts(ResourceBase):
    def __init__(self) -> None:
        self.items: dict[str, JsonObject] = {"1": {"id": "1", "name": "Ada"}}

    def schema(self) -> ResourceSchema:
        return ResourceSchema(
            "contacts",
            (FieldSchema("id", "text", True), FieldSchema("name", "text")),
            CAPABILITIES,
        )

    def list(self, query: ListQuery) -> ResourcePage:
        values = list(self.items.values())
        search = query.q
        if search is not None:
            values = [item for item in values if _matches_name(item, search)]
        return ResourcePage(
            tuple(values[query.offset : query.offset + query.limit]), len(values), query.offset, query.limit
        )

    def get(self, resource_id: str) -> JsonObject | None:
        return self.items.get(resource_id)

    def create(self, value: JsonObject) -> JsonObject:
        resource_id = value.get("id")
        if not isinstance(resource_id, str):
            raise ValueError("contacts require a string id")
        self.items[resource_id] = value
        return value

    def update(self, resource_id: str, merge_patch: JsonObject) -> JsonObject:
        updated = apply_merge_patch(self.items[resource_id], merge_patch)
        if not isinstance(updated, dict):
            raise TypeError("resource update must return a JSON object")
        self.items[resource_id] = updated
        return self.items[resource_id]

    def delete(self, resource_id: str) -> None:
        del self.items[resource_id]

    def invoke(self, action: str, value: JsonValue) -> JsonValue:
        return {"action": action, "input": value}


def _matches_name(item: JsonObject, search: str) -> bool:
    name = item.get("name")
    return isinstance(name, str) and search.lower() in name.lower()


def create_asgi_app() -> ResourceASGIApp:
    """Build the dependency-free ASGI application used by the tests."""

    return ResourceASGIApp({"contacts": Contacts()})


def create_fastapi_application() -> Any:
    """Build the optional FastAPI application around the same provider."""

    return create_fastapi_app({"contacts": Contacts()})


try:
    app = create_fastapi_application()
except RuntimeError:
    # The example remains importable and runnable with only the stdlib adapter.
    app = create_asgi_app()


if __name__ == "__main__":
    print("ASGI app ready: use `uvicorn app:app --app-dir examples/python-fastapi` with an ASGI server")

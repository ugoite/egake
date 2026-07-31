"""Dependency-free ASGI adapter for the ikashita Resource Contract."""

from __future__ import annotations

import inspect
import json
import os
import re
import threading
from typing import Any, Awaitable, Callable, Dict, Mapping, MutableMapping, Tuple
from urllib.parse import parse_qsl, unquote

from .resource import (
    CAPABILITIES,
    DEFAULT_PAGE_LIMIT,
    FieldSchema,
    JsonValue,
    ListQuery,
    Resource,
    ResourceError,
    ResourcePage,
    ResourceSchema,
    Sort,
    is_safe_request_id,
    normalize_limit,
    require_object_patch,
)

API_PREFIX = "/api/ikashita/v1"
MAX_JSON_BODY = 2 * 1024 * 1024
MAX_QUERY_BYTES = 16 * 1024
_request_counter = 0
_request_lock = threading.Lock()


def make_request_id() -> str:
    """Create a process-local ID without reading or formatting request data."""

    global _request_counter
    with _request_lock:
        _request_counter += 1
        return "req-{0}-{1}".format(os.getpid(), _request_counter)


def parse_list_query(query_string: str) -> ListQuery:
    """Parse q/sort/offset/limit and ignore unknown keys."""

    if not isinstance(query_string, str):
        raise ResourceError("validation_failed", "invalid list query")
    if len(query_string.encode("utf-8")) > MAX_QUERY_BYTES:
        raise ResourceError("validation_failed", "request query is too large")
    if query_string == "":
        return ListQuery()
    try:
        pairs = parse_qsl(
            query_string,
            keep_blank_values=True,
            strict_parsing=True,
            encoding="utf-8",
            errors="strict",
        )
    except (UnicodeDecodeError, ValueError) as exc:
        raise ResourceError(
            "validation_failed",
            "invalid list query",
            {"query": "invalid encoding or query pair"},
        ) from exc
    values: Dict[str, str] = {}
    for key, value in pairs:
        if key in {"q", "sort", "offset", "limit"}:
            values[key] = value
    search = values.get("q") or None
    sort_value = values.get("sort", "")
    sort = []
    for raw in (item for item in sort_value.split(",") if item):
        if raw.startswith("-"):
            field_name, direction = raw[1:], "desc"
        elif raw.endswith(":desc"):
            field_name, direction = raw[:-5], "desc"
        elif raw.endswith(":asc"):
            field_name, direction = raw[:-4], "asc"
        else:
            field_name, direction = raw, "asc"
        if not field_name.strip() or any(ord(character) < 0x20 for character in field_name):
            raise ResourceError("validation_failed", "invalid list query", {"sort": "sort fields must not be empty"})
        sort.append(Sort(field_name, direction))
    offset_value = values.get("offset", "0")
    if not offset_value.isascii() or not offset_value.isdigit():
        raise ResourceError("validation_failed", "invalid list query", {"offset": "must be a non-negative integer"})
    limit_value = values.get("limit", str(DEFAULT_PAGE_LIMIT))
    if not limit_value.isascii() or not limit_value.isdigit():
        raise ResourceError("validation_failed", "invalid list query", {"limit": "must be a positive integer"})
    return ListQuery(search, tuple(sort), int(offset_value), normalize_limit(int(limit_value)))


def _status(error: ResourceError) -> int:
    return {
        "validation_failed": 400,
        "not_found": 404,
        "conflict": 409,
        "capability_denied": 405,
        "unavailable": 503,
        "internal": 500,
    }.get(error.code, 500)


def _safe_error(error: ResourceError, request_id: str) -> ResourceError:
    if error.code == "internal":
        return ResourceError("internal", "internal server error", request_id=request_id)
    if error.code == "unavailable":
        return ResourceError("unavailable", "resource provider is unavailable", request_id=request_id)
    return error.with_request_id(request_id)


def _json_value(raw: bytes) -> JsonValue:
    if not raw:
        raise ResourceError("validation_failed", "request body is missing or empty")

    def reject_constant(value: str) -> None:
        raise ValueError(value)

    try:
        value = json.loads(raw.decode("utf-8"), parse_constant=reject_constant)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as exc:
        raise ResourceError("validation_failed", "request body is not valid JSON", {"json": "expected one JSON value"}) from exc
    if not _is_json_value(value):
        raise ResourceError("validation_failed", "request body is not valid JSON")
    return value


def _is_json_value(value: Any) -> bool:
    if value is None or isinstance(value, (str, bool, int, float)):
        return not isinstance(value, float) or value == value and value not in (float("inf"), float("-inf"))
    if isinstance(value, list):
        return all(_is_json_value(item) for item in value)
    if isinstance(value, dict):
        return all(isinstance(key, str) and _is_json_value(item) for key, item in value.items())
    return False


async def _maybe_await(value: Any) -> Any:
    if inspect.isawaitable(value):
        return await value
    return value


class ResourceASGIApp:
    """ASGI app dispatching Resource Contract operations to named resources."""

    def __init__(self, resources: Mapping[str, Resource], max_body_bytes: int = MAX_JSON_BODY) -> None:
        if not isinstance(max_body_bytes, int) or isinstance(max_body_bytes, bool) or max_body_bytes <= 0:
            raise ValueError("max_body_bytes must be a positive integer")
        for name in resources:
            if not _is_safe_route_name(name):
                raise ValueError("resource names must be safe single path segments")
        self.resources = dict(resources)
        self.max_body_bytes = max_body_bytes

    async def __call__(self, scope: MutableMapping[str, Any], receive: Callable[[], Awaitable[MutableMapping[str, Any]]], send: Callable[[MutableMapping[str, Any]], Awaitable[None]]) -> None:
        if scope.get("type") != "http":
            await self._send(send, 404, {"code": "not_found", "message": "route was not found", "fields": {}}, make_request_id())
            return
        request_id = self._request_id(scope)
        try:
            result, status = await self._dispatch(scope, receive)
            await self._send_json(send, status, result, request_id)
        except ResourceError as error:
            safe_error = _safe_error(error, request_id)
            await self._send(send, _status(safe_error), safe_error.as_dict(), request_id)
        except Exception:
            error = ResourceError("internal", "resource operation failed", request_id=request_id)
            await self._send(send, 500, error.as_dict(), request_id)

    async def _dispatch(self, scope: Mapping[str, Any], receive: Callable[[], Awaitable[MutableMapping[str, Any]]]) -> Tuple[Any, int]:
        method = str(scope.get("method", "GET")).upper()
        path = scope.get("path", "")
        if not isinstance(path, str):
            raise ResourceError("not_found", "API route was not found")
        if not path.startswith(API_PREFIX):
            raise ResourceError("not_found", "API route was not found")
        tail = path[len(API_PREFIX):].strip("/")
        segments = _decode_route_segments(tail)
        if len(segments) < 2 or segments[0] != "resources":
            raise ResourceError("not_found", "API route was not found")
        name = segments[1]
        resource = self.resources.get(name)
        if resource is None:
            raise ResourceError("not_found", "resource was not found")
        schema = await _maybe_await(resource.schema())
        _validate_schema(name, schema)
        if segments == ["resources", name, "schema"]:
            self._require(schema, "schema")
            return _schema_json(schema), 200
        if segments == ["resources", name]:
            if method == "GET":
                self._require(schema, "list")
                query_bytes = scope.get("query_string", b"")
                if not isinstance(query_bytes, bytes):
                    raise ResourceError("validation_failed", "request query is not valid UTF-8")
                try:
                    query_text = query_bytes.decode("utf-8")
                except UnicodeDecodeError as exc:
                    raise ResourceError("validation_failed", "request query is not valid UTF-8") from exc
                query = parse_list_query(query_text)
                page = await _maybe_await(resource.list(query))
                if not isinstance(page, ResourcePage):
                    raise ResourceError("internal", "resource returned an invalid page")
                return page.as_dict(), 200
            if method == "POST":
                self._require(schema, "create")
                value = _json_value(await self._body(receive))
                if not isinstance(value, dict):
                    raise ResourceError("validation_failed", "resource value must be a JSON object")
                return _object_result(await _maybe_await(resource.create(value)), "create"), 201
            raise self._method_error()
        if len(segments) == 4 and segments[0:2] == ["resources", name] and segments[2] == "items":
            resource_id = segments[3]
            if method == "GET":
                self._require(schema, "get")
                value = await _maybe_await(resource.get(resource_id))
                if value is None:
                    raise ResourceError("not_found", "resource item was not found")
                return _object_result(value, "get"), 200
            if method == "PATCH":
                self._require(schema, "update")
                patch = require_object_patch(_json_value(await self._body(receive)))
                return _object_result(await _maybe_await(resource.update(resource_id, patch)), "update"), 200
            if method == "DELETE":
                self._require(schema, "delete")
                await _maybe_await(resource.delete(resource_id))
                return {"ok": True}, 200
            raise self._method_error()
        if len(segments) == 4 and segments[0:2] == ["resources", name] and segments[2] == "actions":
            if method != "POST":
                raise self._method_error()
            self._require(schema, "invoke")
            result = await _maybe_await(resource.invoke(segments[3], _json_value(await self._body(receive))))
            if not _is_json_value(result):
                raise ResourceError("internal", "resource returned an invalid action result")
            return result, 200
        raise ResourceError("not_found", "API route was not found")

    async def _body(self, receive: Callable[[], Awaitable[MutableMapping[str, Any]]]) -> bytes:
        chunks = []
        length = 0
        while True:
            message = await receive()
            if message.get("type") == "http.disconnect":
                raise ResourceError("validation_failed", "request body was disconnected")
            if message.get("type") != "http.request":
                continue
            chunk = message.get("body", b"")
            if not isinstance(chunk, bytes):
                raise ResourceError("validation_failed", "request body is not valid bytes")
            length += len(chunk)
            if length > self.max_body_bytes:
                raise ResourceError("validation_failed", "request JSON body is too large")
            chunks.append(chunk)
            if not message.get("more_body", False):
                return b"".join(chunks)

    def _require(self, schema: ResourceSchema, capability: str) -> None:
        if capability not in CAPABILITIES or not schema.has_capability(capability):
            raise ResourceError("capability_denied", "resource does not expose the {0} capability".format(capability))

    @staticmethod
    def _method_error() -> ResourceError:
        return ResourceError("capability_denied", "HTTP method is not supported for this route")

    @staticmethod
    def _request_id(scope: Mapping[str, Any]) -> str:
        for key, value in scope.get("headers", ()):
            if not isinstance(key, bytes) or not isinstance(value, bytes):
                continue
            try:
                header = key.decode("ascii").lower()
                candidate = value.decode("ascii")
            except UnicodeDecodeError:
                continue
            if header == "x-request-id" and is_safe_request_id(candidate):
                return candidate
        return make_request_id()

    @staticmethod
    async def _send_json(send: Callable[[MutableMapping[str, Any]], Awaitable[None]], status: int, value: Any, request_id: str) -> None:
        try:
            body = json.dumps(
                value,
                ensure_ascii=False,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as exc:
            error = ResourceError("internal", "could not encode JSON response", request_id=request_id)
            await ResourceASGIApp._send(send, 500, error.as_dict(), request_id)
            return
        await send({"type": "http.response.start", "status": status, "headers": [(b"content-type", b"application/json"), (b"x-request-id", request_id.encode("ascii"))]})
        await send({"type": "http.response.body", "body": body})

    @staticmethod
    async def _send(send: Callable[[MutableMapping[str, Any]], Awaitable[None]], status: int, error: Mapping[str, Any], request_id: str) -> None:
        await ResourceASGIApp._send_json(send, status, {"error": dict(error)}, request_id)


def _schema_json(schema: ResourceSchema) -> Dict[str, Any]:
    return {
        "name": schema.name,
        "fields": [
            {
                "name": field.name,
                "field_type": field.field_type,
                "required": field.required,
                **({"enum": list(field.enum_values)} if field.enum_values is not None else {}),
                **({"format": field.format} if field.format is not None else {}),
            }
            for field in schema.fields
        ],
        "capabilities": list(schema.capabilities),
    }


_INVALID_PERCENT = re.compile(r"%(?![0-9A-Fa-f]{2})")


def _is_safe_route_name(value: str) -> bool:
    return isinstance(value, str) and bool(value) and value.strip() == value and value not in {".", ".."} and not any(
        character in "/\\" or ord(character) < 0x20 for character in value
    )


def _decode_route_segments(tail: str) -> list[str]:
    if not tail:
        return []
    raw_segments = tail.split("/")
    if any(not segment for segment in raw_segments):
        raise ResourceError("not_found", "API route was not found")
    segments = []
    for raw_segment in raw_segments:
        if _INVALID_PERCENT.search(raw_segment):
            raise ResourceError("validation_failed", "URL path contains invalid encoding")
        try:
            segment = unquote(raw_segment, encoding="utf-8", errors="strict")
        except UnicodeDecodeError as exc:
            raise ResourceError("validation_failed", "URL path is not valid UTF-8") from exc
        if not segment or segment in {".", ".."} or any(ord(character) < 0x20 for character in segment):
            raise ResourceError("not_found", "API route was not found")
        segments.append(segment)
    if segments[0] != "resources" or len(segments) < 2 or not _is_safe_route_name(segments[1]):
        raise ResourceError("not_found", "API route was not found")
    return segments


def _validate_schema(name: str, schema: Any) -> None:
    if not isinstance(schema, ResourceSchema) or schema.name != name:
        raise ResourceError("internal", "resource returned an invalid schema")
    field_names = set()
    allowed_field_types = {"text", "number", "integer", "boolean", "date", "json"}
    for field in schema.fields:
        if not isinstance(field, FieldSchema):
            raise ResourceError("internal", "resource returned an invalid schema")
        if (
            not isinstance(field.name, str)
            or not field.name.strip()
            or field.name in field_names
            or field.field_type not in allowed_field_types
            or not isinstance(field.required, bool)
            or (
                field.enum_values is not None
                and (
                    not isinstance(field.enum_values, (tuple, list))
                    or not all(_is_json_value(value) for value in field.enum_values)
                )
            )
            or (field.format is not None and not isinstance(field.format, str))
        ):
            raise ResourceError("internal", "resource returned an invalid schema")
        field_names.add(field.name)
    if (
        any(capability not in CAPABILITIES for capability in schema.capabilities)
        or len(set(schema.capabilities)) != len(schema.capabilities)
    ):
        raise ResourceError("internal", "resource returned an invalid schema")


def _object_result(value: Any, operation: str) -> Dict[str, Any]:
    if not isinstance(value, dict) or not _is_json_value(value):
        raise ResourceError("internal", "resource returned an invalid {0} result".format(operation))
    return value

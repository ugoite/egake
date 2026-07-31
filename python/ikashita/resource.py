"""Standard-library Resource Contract types and merge-patch helpers."""

from __future__ import annotations

import copy
import re
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Dict, Iterable, List, Optional, Protocol, Tuple, Union

JsonValue = Union[None, bool, int, float, str, List["JsonValue"], Dict[str, "JsonValue"]]
JsonObject = Dict[str, JsonValue]

CAPABILITIES: Tuple[str, ...] = ("schema", "list", "get", "create", "update", "delete", "invoke")
ERROR_CODES: Tuple[str, ...] = (
    "validation_failed",
    "not_found",
    "conflict",
    "capability_denied",
    "unavailable",
    "internal",
)
DEFAULT_PAGE_LIMIT = 50
MAX_PAGE_LIMIT = 500


@dataclass(frozen=True)
class FieldSchema:
    """A field declaration used by hosts for validation and rendering."""

    name: str
    field_type: str
    required: bool = False


@dataclass(frozen=True)
class ResourceSchema:
    """The fields and capabilities advertised by one resource."""

    name: str
    fields: Tuple[FieldSchema, ...] = ()
    capabilities: Tuple[str, ...] = ()

    def has_capability(self, capability: str) -> bool:
        return capability in self.capabilities


@dataclass(frozen=True)
class Sort:
    """One ordered sort field."""

    field: str
    direction: str = "asc"


@dataclass(frozen=True)
class ListQuery:
    """Normalized list query using q/sort/offset/limit semantics."""

    q: Optional[str] = None
    sort: Tuple[Sort, ...] = ()
    offset: int = 0
    limit: int = DEFAULT_PAGE_LIMIT


@dataclass(frozen=True)
class ResourcePage:
    """A page of JSON object records."""

    items: Tuple[JsonObject, ...]
    total: int
    offset: int
    limit: int

    def __post_init__(self) -> None:
        if not isinstance(self.total, int) or isinstance(self.total, bool) or self.total < 0:
            raise ResourceError("validation_failed", "page total must be a non-negative integer")
        if not isinstance(self.offset, int) or isinstance(self.offset, bool) or self.offset < 0:
            raise ResourceError("validation_failed", "page offset must be a non-negative integer")
        if not isinstance(self.items, (tuple, list)) or any(
            not isinstance(item, dict) or not _is_json_value(item) for item in self.items
        ):
            raise ResourceError("internal", "resource returned an invalid page")
        object.__setattr__(self, "limit", normalize_limit(self.limit))

    def as_dict(self) -> Dict[str, Any]:
        return {"items": list(self.items), "total": self.total, "offset": self.offset, "limit": self.limit}


@dataclass
class ResourceError(Exception):
    """A structured provider failure safe for transport adapters."""

    code: str
    message: str
    fields: Dict[str, str] = field(default_factory=dict)
    request_id: Optional[str] = None

    def __post_init__(self) -> None:
        Exception.__init__(self, self.message)

    def as_dict(self) -> Dict[str, Any]:
        value: Dict[str, Any] = {"code": self.code, "message": self.message, "fields": dict(self.fields)}
        if self.request_id is not None:
            value["request_id"] = self.request_id
        return value

    def with_request_id(self, request_id: str) -> "ResourceError":
        return ResourceError(self.code, self.message, dict(self.fields), request_id)


class Resource(Protocol):
    """Protocol implemented by a host resource.

    Methods may be synchronous or return awaitables; the ASGI adapter supports
    both so a host can wrap an existing async client without changing this
    value contract.
    """

    def schema(self) -> ResourceSchema:
        ...

    def list(self, query: ListQuery) -> ResourcePage:
        ...

    def get(self, resource_id: str) -> Optional[JsonObject]:
        ...

    def create(self, value: JsonObject) -> JsonObject:
        ...

    def update(self, resource_id: str, merge_patch: JsonObject) -> JsonObject:
        ...

    def delete(self, resource_id: str) -> None:
        ...

    def invoke(self, action: str, value: JsonValue) -> JsonValue:
        ...


class ResourceBase(ABC):
    """Convenience base class with an explicit unsupported-invoke default."""

    @abstractmethod
    def schema(self) -> ResourceSchema:
        raise NotImplementedError

    @abstractmethod
    def list(self, query: ListQuery) -> ResourcePage:
        raise NotImplementedError

    @abstractmethod
    def get(self, resource_id: str) -> Optional[JsonObject]:
        raise NotImplementedError

    @abstractmethod
    def create(self, value: JsonObject) -> JsonObject:
        raise NotImplementedError

    @abstractmethod
    def update(self, resource_id: str, merge_patch: JsonObject) -> JsonObject:
        raise NotImplementedError

    @abstractmethod
    def delete(self, resource_id: str) -> None:
        raise NotImplementedError

    def invoke(self, action: str, value: JsonValue) -> JsonValue:
        raise ResourceError("capability_denied", "resource does not expose actions")


def _is_json_value(value: Any) -> bool:
    if value is None or isinstance(value, (str, bool, int, float)):
        return not isinstance(value, float) or value == value and value not in (float("inf"), float("-inf"))
    if isinstance(value, list):
        return all(_is_json_value(item) for item in value)
    if isinstance(value, dict):
        return all(isinstance(key, str) and _is_json_value(item) for key, item in value.items())
    return False


def apply_merge_patch(target: JsonValue, patch: JsonValue) -> JsonValue:
    """Apply RFC 7396 semantics without mutating either input value."""

    if not _is_json_value(target) or not _is_json_value(patch):
        raise ResourceError("validation_failed", "merge patch values must be JSON")
    if not isinstance(patch, dict):
        return copy.deepcopy(patch)
    result: JsonObject = copy.deepcopy(target) if isinstance(target, dict) else {}
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = apply_merge_patch(result.get(key), value)
    return result


def require_object_patch(value: JsonValue) -> JsonObject:
    """Require the object-shaped patch mandated for resource updates."""

    if not isinstance(value, dict):
        raise ResourceError("validation_failed", "resource update patch must be a JSON object", {"patch": "expected a JSON object"})
    return value


def normalize_limit(limit: int) -> int:
    if not isinstance(limit, int) or isinstance(limit, bool) or limit < 0:
        raise ResourceError("validation_failed", "limit must be a non-negative integer", {"limit": "must be a non-negative integer"})
    return 1 if limit == 0 else min(limit, MAX_PAGE_LIMIT)


def normalize_page(items: Iterable[JsonObject], total: int, offset: int, limit: int) -> ResourcePage:
    """Construct a page with the contract's limit bounds."""

    if not isinstance(offset, int) or isinstance(offset, bool) or offset < 0:
        raise ResourceError("validation_failed", "offset must be a non-negative integer", {"offset": "must be a non-negative integer"})
    return ResourcePage(tuple(items), total, offset, normalize_limit(limit))


_SAFE_REQUEST_ID = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")


def is_safe_request_id(value: str) -> bool:
    """Return whether a request ID is safe for response/header propagation."""

    return bool(_SAFE_REQUEST_ID.fullmatch(value))

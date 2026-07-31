import asyncio
import json
import unittest

from ikashita import (
    CAPABILITIES,
    FieldSchema,
    ListQuery,
    ResourceASGIApp,
    ResourceBase,
    ResourcePage,
    ResourceSchema,
    apply_merge_patch,
)


class MemoryResource(ResourceBase):
    def __init__(self):
        self.items = {"1": {"id": "1", "name": "Ada", "profile": {"team": "math"}}}

    def schema(self):
        return ResourceSchema(
            "contacts",
            (
                FieldSchema("id", "text", True),
                FieldSchema("name", "text"),
                FieldSchema("status", "text", False, ("active", "paused")),
                FieldSchema("email", "text", False, None, "email"),
            ),
            CAPABILITIES,
        )

    def list(self, query: ListQuery):
        values = list(self.items.values())
        if query.q:
            values = [item for item in values if query.q.lower() in json.dumps(item).lower()]
        return ResourcePage(tuple(values[query.offset:query.offset + query.limit]), len(values), query.offset, query.limit)

    def get(self, resource_id):
        return self.items.get(resource_id)

    def create(self, value):
        self.items[value["id"]] = value
        return value

    def update(self, resource_id, merge_patch):
        self.items[resource_id] = apply_merge_patch(self.items[resource_id], merge_patch)
        return self.items[resource_id]

    def delete(self, resource_id):
        del self.items[resource_id]

    def invoke(self, action, value):
        return {"action": action, "input": value}


def call(app, method, path, body=b"", headers=()):
    messages = []
    request_sent = False

    async def receive():
        nonlocal request_sent
        if request_sent:
            return {"type": "http.disconnect"}
        request_sent = True
        return {"type": "http.request", "body": body, "more_body": False}

    async def send(message):
        messages.append(message)

    scope = {
        "type": "http",
        "method": method,
        "path": path,
        "query_string": b"",
        "headers": list(headers),
    }
    asyncio.run(app(scope, receive, send))
    start = messages[0]
    return start["status"], json.loads(messages[1]["body"]), dict(start["headers"])


class ResourceASGITest(unittest.TestCase):
    def setUp(self):
        self.app = ResourceASGIApp({"contacts": MemoryResource()})

    def test_crud_and_invoke_routes(self):
        status, schema, _ = call(self.app, "GET", "/api/ikashita/v1/resources/contacts/schema")
        self.assertEqual(status, 200)
        self.assertEqual(schema["name"], "contacts")
        self.assertEqual(schema["fields"][2]["enum"], ["active", "paused"])
        self.assertEqual(schema["fields"][3]["format"], "email")

        status, page, _ = call(self.app, "GET", "/api/ikashita/v1/resources/contacts")
        self.assertEqual(status, 200)
        self.assertEqual(page["items"][0]["name"], "Ada")

        status, created, _ = call(self.app, "POST", "/api/ikashita/v1/resources/contacts", b'{"id":"2","name":"Grace"}')
        self.assertEqual((status, created["id"]), (201, "2"))

        status, updated, _ = call(self.app, "PATCH", "/api/ikashita/v1/resources/contacts/items/2", b'{"name":"Grace Hopper"}')
        self.assertEqual((status, updated["name"]), (200, "Grace Hopper"))

        status, result, _ = call(self.app, "POST", "/api/ikashita/v1/resources/contacts/actions/echo", b'{"ok":true}')
        self.assertEqual((status, result["action"]), (200, "echo"))

        status, deleted, _ = call(self.app, "DELETE", "/api/ikashita/v1/resources/contacts/items/2")
        self.assertEqual((status, deleted["ok"]), (200, True))

    def test_structured_errors_and_request_id(self):
        status, error, headers = call(
            self.app,
            "GET",
            "/api/ikashita/v1/resources/missing",
            headers=((b"x-request-id", b"host-request-1"),),
        )
        self.assertEqual(status, 404)
        self.assertEqual(error["error"]["code"], "not_found")
        self.assertEqual(error["error"]["request_id"], "host-request-1")
        self.assertEqual(headers[b"x-request-id"], b"host-request-1")

        status, error, _ = call(self.app, "POST", "/api/ikashita/v1/resources/contacts", b"not json")
        self.assertEqual((status, error["error"]["code"]), (400, "validation_failed"))

import importlib.util
import json
import pathlib
import unittest
from urllib.parse import urlsplit


def load_example():
    path = pathlib.Path(__file__).parents[2] / "examples" / "python-fastapi" / "app.py"
    spec = importlib.util.spec_from_file_location("ikashita_python_fastapi_example", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def call(app, method, path, body=b""):
    import asyncio

    messages = []
    sent = False

    async def receive():
        nonlocal sent
        if sent:
            return {"type": "http.disconnect"}
        sent = True
        return {"type": "http.request", "body": body, "more_body": False}

    async def send(message):
        messages.append(message)

    url = urlsplit(path)
    scope = {
        "type": "http",
        "method": method,
        "path": url.path,
        "query_string": url.query.encode("ascii"),
        "headers": [],
    }
    asyncio.run(app(scope, receive, send))
    return messages[0]["status"], json.loads(messages[1]["body"])


class PythonExampleTest(unittest.TestCase):
    def test_stdlib_asgi_example_lists_and_invokes(self):
        module = load_example()
        app = module.create_asgi_app()
        status, page = call(app, "GET", "/api/ikashita/v1/resources/contacts?q=Ada")
        self.assertEqual(status, 200)
        self.assertEqual(page["items"][0]["name"], "Ada")
        status, result = call(
            app,
            "POST",
            "/api/ikashita/v1/resources/contacts/actions/health",
            b'{"source":"test"}',
        )
        self.assertEqual(status, 200)
        self.assertEqual(result, {"action": "health", "input": {"source": "test"}})


if __name__ == "__main__":
    unittest.main()

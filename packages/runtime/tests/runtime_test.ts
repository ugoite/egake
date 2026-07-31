import {
  applyMergePatch,
  assertCapability,
  inputTypeForField,
  isSafeRequestId,
  parseApplication,
  ResourceClient,
  ResourceError,
} from "../mod.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function assertEquals(
  actual: unknown,
  expected: unknown,
  message = "values differ",
): void {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `${message}: ${JSON.stringify(actual)} !== ${JSON.stringify(expected)}`,
    );
  }
}

Deno.test("merge patch is recursive and non-mutating", () => {
  const target = {
    name: "Ada",
    profile: { team: "math", active: true },
    tags: ["old"],
  };
  const merged = applyMergePatch(target, {
    profile: { team: "science", active: null },
    tags: ["new"],
  });
  assertEquals(merged, {
    name: "Ada",
    profile: { team: "science" },
    tags: ["new"],
  });
  assertEquals(target.profile.team, "math");
});

Deno.test("client uses relative same-origin requests, capabilities, and request IDs", async () => {
  const calls: { input: RequestInfo | URL; init?: RequestInit }[] = [];
  const client = new ResourceClient({
    requestId: "host-request-1",
    fetch: async (input, init) => {
      calls.push({ input, init });
      const url = String(input);
      if (url.endsWith("/schema")) {
        return new Response(
          JSON.stringify({
            name: "contacts",
            fields: [],
            capabilities: ["schema", "list"],
          }),
          { status: 200, headers: { "x-request-id": "host-request-1" } },
        );
      }
      return new Response(
        JSON.stringify({ items: [], total: 0, offset: 0, limit: 50 }),
        { status: 200 },
      );
    },
  });
  const provider = client.resource("contacts");
  const page = await provider.list({ sort: [], offset: 0, limit: 50 });
  assertEquals(page.total, 0);
  assertEquals(
    String(calls[0].input),
    "/api/ikashita/v1/resources/contacts/schema",
  );
  assertEquals(calls[0].init?.credentials, "same-origin");
  assertEquals(
    new Headers(calls[0].init?.headers).get("x-request-id"),
    "host-request-1",
  );
  await assertRejectsCode(
    async () => await provider.get("1"),
    "capability_denied",
    calls,
    client,
  );
});

async function assertRejectsCode(
  operation: () => Promise<unknown>,
  code: string,
  calls: readonly unknown[],
  _client: ResourceClient,
): Promise<void> {
  try {
    await operation();
  } catch (error) {
    assert(error instanceof ResourceError, "expected ResourceError");
    assertEquals(error.code, code);
    assert(calls.length === 2, "capability check must not issue a get request");
    return;
  }
  throw new Error("expected operation to reject");
}

Deno.test("request IDs and serialized app parsing are constrained", () => {
  assert(isSafeRequestId("req-123._:"), "safe request ID rejected");
  assert(!isSafeRequestId("contains spaces"), "unsafe request ID accepted");
  const application = parseApplication({
    profile: { name: "safe", version: "0.1" },
    resources: [{
      name: "contacts",
      schema: "contacts.schema.json",
      capabilities: ["schema", "list"],
      fields: [
        { name: "email", field_type: "text", required: true, format: "email" },
        {
          name: "status",
          field_type: "text",
          required: false,
          enum: ["active", "paused"],
        },
      ],
    }],
    states: [],
    actions: [],
    pages: [{
      name: "home",
      title: "<text>",
      components: [{
        kind: "text",
        text: "<script>not executable</script>",
        attributes: {},
        children: [],
        events: [],
      }],
    }],
  });
  assertEquals(application.resources[0].required_capabilities, ["schema", "list"]);
  assertEquals(application.resources[0].fields?.[1].enum, ["active", "paused"]);
  assertEquals(inputTypeForField(application.resources[0].fields?.[0]), "email");
  assertEquals(inputTypeForField({ name: "when", field_type: "date", required: false, format: "date" }), "date");
  assertEquals(
    application.pages[0].components[0].text,
    "<script>not executable</script>",
  );
  try {
    parseApplication({
      profile: { name: "bad", version: "0.1" },
      resources: [],
      states: [],
      actions: [],
      pages: [{
        name: "x",
        title: "x",
        components: [{
          kind: "text",
          attributes: { html: "bad" },
          children: [],
          events: [],
        }],
      }],
    });
  } catch (error) {
    assert(
      error instanceof ResourceError,
      "invalid application should be structured",
    );
    return;
  }
  throw new Error("arbitrary component attribute was accepted");
});

Deno.test("capability checks preserve structured errors", () => {
  try {
    assertCapability({
      name: "read-only",
      fields: [],
      capabilities: ["schema"],
    }, "delete");
  } catch (error) {
    assert(error instanceof ResourceError, "expected ResourceError");
    assertEquals(error.code, "capability_denied");
    return;
  }
  throw new Error("missing capability was accepted");
});

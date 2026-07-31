import { createReactRenderer } from "../mod.ts";

function assert(value: boolean): void {
  if (!value) throw new Error("assertion failed");
}

function assertEquals(actual: unknown, expected: unknown): void {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

const fakeReact = {
  createElement(
    type: string,
    props: Readonly<Record<string, unknown>> | null,
    ...children: unknown[]
  ) {
    return { type, props, children };
  },
};

Deno.test("React adapter renders application text as children", () => {
  const render = createReactRenderer(fakeReact);
  const tree = render({
    profile: { name: "safe", version: "0.1" },
    resources: [],
    states: [],
    actions: [],
    pages: [{
      name: "home",
      title: "<not markup>",
      components: [{
        kind: "text",
        text: "<script>bad()</script>",
        attributes: {},
        children: [],
        events: [],
      }],
    }],
  }) as { children: unknown[] };
  assert(tree.children.length === 1);
  const section = tree.children[0] as { children: unknown[] };
  const textNode = section.children[1] as { children: unknown[] };
  assertEquals(textNode.children[0], "<script>bad()</script>");
});

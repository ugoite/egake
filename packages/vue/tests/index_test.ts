import { createVueRenderer } from "../mod.ts";

function assertEquals(actual: unknown, expected: unknown): void {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, got ${String(actual)}`);
  }
}

const fakeVue = {
  h(
    type: string,
    props: Readonly<Record<string, unknown>> | null,
    children?: unknown,
  ) {
    return { type, props, children };
  },
};

Deno.test("Vue adapter keeps application strings in VNode children", () => {
  const render = createVueRenderer(fakeVue);
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
  const section = tree.children[0] as { children: unknown[] };
  assertEquals(
    (section.children[0] as { children: unknown }).children,
    "<not markup>",
  );
});

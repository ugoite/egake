import { createSvelteRenderer } from "../mod.ts";

interface Node {
  type: string;
  attributes: Record<string, string>;
  children: Node[] | string[];
}

function assert(value: boolean): void {
  if (!value) throw new Error("assertion failed");
}

const fakeSvelte = {
  createElement(type: string): Node {
    return { type, attributes: {}, children: [] };
  },
  createText(value: string): string {
    return value;
  },
  append(parent: unknown, child: unknown): void {
    (parent as Node).children.push(child as never);
  },
  clear(parent: unknown): void {
    (parent as Node).children = [];
  },
  setAttribute(element: unknown, name: string, value: string): void {
    (element as Node).attributes[name] = value;
  },
  listen(): () => void {
    return () => undefined;
  },
};

Deno.test("Svelte adapter mounts safe text and supports lifecycle", () => {
  const target: Node = { type: "target", attributes: {}, children: [] };
  const mount = createSvelteRenderer(fakeSvelte)(target, {
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
  });
  const app = target.children[0] as Node;
  const section = app.children[0] as Node;
  const text = section.children[1] as Node;
  assert(text.children[0] === "<script>bad()</script>");
  mount.destroy();
  assert(target.children.length === 0);
});

import { createSolidRenderer } from "../mod.ts";

interface Node {
  type: string;
  attributes: Record<string, string>;
  children: unknown[];
}

function assert(value: boolean): void {
  if (!value) throw new Error("assertion failed");
}

const fakeSolid = {
  createElement(type: string): Node {
    return { type, attributes: {}, children: [] };
  },
  createComponent<Props extends object>(
    component: (props: Props) => unknown,
    props: Props,
  ): unknown {
    return component(props);
  },
  insert(parent: unknown, child: unknown): void {
    (parent as Node).children.push(child);
  },
  setAttribute(element: unknown, name: string, value: string): void {
    (element as Node).attributes[name] = value;
  },
  listen(): void {
    // The fake only checks the generated tree.
  },
};

Deno.test("Solid adapter uses component/element primitives and safe children", () => {
  const render = createSolidRenderer(fakeSolid);
  const tree = render({
    profile: { name: "safe", version: "0.1" },
    resources: [],
    states: [],
    actions: [],
    pages: [{
      name: "home",
      title: "<not markup>",
      components: [{
        kind: "column",
        attributes: {},
        children: [{
          kind: "text",
          text: "<script>bad()</script>",
          attributes: {},
          children: [],
          events: [],
        }],
        events: [],
      }],
    }],
  }) as Node;
  const section = tree.children[0] as Node;
  const column = section.children[1] as Node;
  const text = column.children[0] as Node;
  assert(text.type === "span");
  assert(text.children[0] === "<script>bad()</script>");
});

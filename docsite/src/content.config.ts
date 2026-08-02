import { defineCollection } from "astro:content";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

function documentationId(entry: string): string {
  const id = entry.replace(/\.mdx?$/, "");
  const segments = id.split("/");
  const locale = segments[0] === "en" ? segments.shift() : undefined;
  const topic = segments.join("/");
  if (locale) return topic === "index" ? locale : `${locale}/docs/${topic}`;
  return topic === "index" || topic === "404" ? topic : `docs/${topic}`;
}

export const collections = {
  docs: defineCollection({
    loader: docsLoader({ generateId: ({ entry }) => documentationId(entry) }),
    schema: docsSchema(),
  }),
};

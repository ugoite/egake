import { defineCollection } from "astro:content";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

function documentationId(entry: string): string {
  const id = entry.replace(/\.mdx?$/, "");
  return id === "index" || id === "404" ? id : `docs/${id}`;
}

export const collections = {
  docs: defineCollection({
    loader: docsLoader({ generateId: ({ entry }) => documentationId(entry) }),
    schema: docsSchema(),
  }),
};

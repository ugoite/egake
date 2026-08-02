import mdx from "@astrojs/mdx";
import { unified } from "@astrojs/markdown-remark";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { docsSidebarDirectory, docsSourceDirectory } from "./src/docs-ssot.mjs";
import rewriteDocLinks from "./src/remark-doc-links.mjs";

const docsiteRoot = fileURLToPath(new URL(".", import.meta.url));
const configuredBase = process.env.DOCSITE_BASE ?? "/";
const base = `/${configuredBase.replace(/^\/+|\/+$/g, "")}/`.replace("//", "/");
const site = process.env.DOCSITE_ORIGIN;

export default defineConfig({
  ...(site ? { site } : {}),
  base,
  markdown: {
    // Starlight uses Astro's configured processor for processedDirs. Keeping
    // the rewrite here makes authored links route-aware without deprecated
    // top-level remarkPlugins configuration.
    processor: unified({ remarkPlugins: [rewriteDocLinks] }),
  },
  vite: {
    // External canonical MDX files resolve imports from the repository root;
    // point Starlight component imports back to this site's locked install.
    resolve: {
      alias: [
        {
          find: /^@astrojs\/starlight\/components$/,
          replacement: path.join(
            docsiteRoot,
            "node_modules/@astrojs/starlight/components.ts",
          ),
        },
      ],
    },
  },
  integrations: [
    starlight({
      title: "ikashita",
      description:
        "初心者向けに学ぶ、ikashitaのKDL UIランタイムとResource Contract。",
      locales: {
        root: { label: "日本語", lang: "ja" },
        en: { label: "English", lang: "en" },
      },
      defaultLocale: "root",
      customCss: ["./src/styles/custom.css"],
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/ikashita/ikashita",
        },
      ],
      markdown: {
        // docs/ is the canonical source shared by GitHub, editors, and this site.
        processedDirs: [docsSourceDirectory],
      },
      sidebar: [
        { slug: "index" },
        {
          label: "はじめに",
          items: [
            { slug: "docs/guide/what-is-ikashita" },
            { slug: "docs/guide/mental-model" },
            { slug: "docs/guide/quickstart" },
          ],
        },
        {
          label: "使い方",
          items: [
            { slug: "docs/guide/usage/index" },
            {
              label: "ジャンル別ガイド",
              collapsed: false,
              items: [
                {
                  autogenerate: {
                    directory: docsSidebarDirectory("guide/usage"),
                  },
                },
              ],
            },
          ],
        },
        {
          label: "リファレンス",
          collapsed: true,
          items: [
            { slug: "docs/usage" },
            { slug: "docs/spec" },
            { slug: "docs/reference/repository-layout" },
          ],
        },
      ],
    }),
    mdx(),
  ],
});

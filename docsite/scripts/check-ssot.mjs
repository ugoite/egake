import { lstat, realpath, readdir, readFile } from "node:fs/promises";
import path from "node:path";

const docsiteRoot = process.cwd();
const repoRoot = path.resolve(docsiteRoot, "..");
const docsRoot = path.join(repoRoot, "docs");
const contentRoot = path.join(docsiteRoot, "src", "content", "docs");

const contentStats = await lstat(contentRoot);
if (!contentStats.isSymbolicLink()) {
  throw new Error(
    "docsite/src/content/docs must be a symlink to the docs/ SSOT",
  );
}
if ((await realpath(contentRoot)) !== (await realpath(docsRoot))) {
  throw new Error(
    "docsite/src/content/docs must point exactly to repository docs/",
  );
}

const config = await readFile(
  path.join(docsiteRoot, "src/content.config.ts"),
  "utf8",
);
if (!config.includes("docsLoader({")) {
  throw new Error("content.config.ts must use Starlight's docsLoader()");
}

const files = await collectMarkdown(docsRoot);
if (files.length === 0)
  throw new Error("docs/ must contain Markdown documentation");
for (const file of files) {
  const source = await readFile(file, "utf8");
  if (!source.startsWith("---\n") || !/^title:\s*.+$/m.test(source)) {
    throw new Error(
      `${path.relative(repoRoot, file)} needs Starlight title frontmatter`,
    );
  }
}

async function collectMarkdown(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectMarkdown(fullPath)));
    else if (/\.mdx?$/.test(entry.name)) files.push(fullPath);
  }
  return files;
}

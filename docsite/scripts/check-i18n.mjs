import { createHash } from "node:crypto";
import { readFile, writeFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const docsRoot = path.join(repoRoot, "docs");
const markerPatterns = [
  /^<!--[ \t]*i18n-sync:[ \t]*id=([^\s]+)[ \t]+digest=([0-9a-f]{64})[ \t]*-->[ \t]*$/gm,
  /^\{\/\*[ \t]*i18n-sync:[ \t]*id=([^\s]+)[ \t]+digest=([0-9a-f]{64})[ \t]*\*\/\}[ \t]*$/gm,
];
const expectedTopics = new Set([
  "index",
  "guide/what-is-ikashita",
  "guide/mental-model",
  "guide/quickstart",
  "guide/usage/index",
  "guide/usage/csv",
  "guide/usage/javascript",
  "guide/usage/python",
  "guide/usage/ugoite",
  "guide/usage/framework-adapters",
  "usage",
  "spec",
  "reference/repository-layout",
]);
const update = process.argv.includes("--update");

const rootFiles = await collectDocs(docsRoot);
const rootTopics = new Map(
  rootFiles
    .filter(
      (file) => !path.relative(docsRoot, file).split(path.sep).includes("en"),
    )
    .map((file) => [topicFor(file), file]),
);
const errors = [];

for (const topic of expectedTopics) {
  if (!rootTopics.has(topic)) errors.push(`missing Japanese topic: ${topic}`);
}
for (const topic of rootTopics.keys()) {
  if (!expectedTopics.has(topic))
    errors.push(`unexpected Japanese topic: ${topic}`);
}

if (errors.length === 0) {
  for (const [topic, japaneseFile] of rootTopics) {
    const englishFile = path.join(
      docsRoot,
      "en",
      path.relative(docsRoot, japaneseFile),
    );
    if (!(await exists(englishFile))) {
      errors.push(
        `${topic}: missing English counterpart ${path.relative(repoRoot, englishFile)}`,
      );
      continue;
    }
    const [japanese, english] = await Promise.all([
      readFile(japaneseFile, "utf8"),
      readFile(englishFile, "utf8"),
    ]);
    const id = topic;
    const digest = pairDigest(id, japanese, english);
    const japaneseMarker = getMarker(japanese, japaneseFile, errors);
    const englishMarker = getMarker(english, englishFile, errors);
    const rootSlug = topic === "index" ? "index" : `docs/${topic}`;
    const englishSlug = topic === "index" ? "en" : `en/docs/${topic}`;
    if (japaneseMarker?.id !== id)
      errors.push(`${topic}: Japanese sync ID must be ${id}`);
    if (englishMarker?.id !== id)
      errors.push(`${topic}: English sync ID must be ${id}`);
    if (japaneseMarker?.digest !== digest || englishMarker?.digest !== digest) {
      errors.push(
        `${topic}: sync digest is stale; run \`mise run docs:i18n:update\``,
      );
    }
    if (headingShape(japanese) !== headingShape(english)) {
      errors.push(`${topic}: Japanese/English heading structure differs`);
    }
    if (!rootSlug || !englishSlug)
      errors.push(`${topic}: locale slug calculation failed`);
    if (update) {
      await writeFile(
        japaneseFile,
        replaceMarker(japanese, japaneseFile, id, digest),
        "utf8",
      );
      await writeFile(
        englishFile,
        replaceMarker(english, englishFile, id, digest),
        "utf8",
      );
    }
  }
}

if (errors.length > 0 && !update) {
  console.error("Documentation locale check failed:");
  console.error(errors.join("\n"));
  process.exit(1);
}

if (update) {
  if (errors.some((error) => error.includes("missing English counterpart"))) {
    console.error(
      "Cannot update locale markers until every English counterpart exists:",
    );
    console.error(errors.join("\n"));
    process.exit(1);
  }
  console.log(
    `Updated sync markers for ${rootTopics.size} documentation topics.`,
  );
} else {
  console.log(
    `Documentation locale check passed (${rootTopics.size} topics, ja/en).`,
  );
}

async function collectDocs(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await collectDocs(fullPath)));
    else if (/\.mdx?$/.test(entry.name)) files.push(fullPath);
  }
  return files;
}

function topicFor(file) {
  return path
    .relative(docsRoot, file)
    .replaceAll(path.sep, "/")
    .replace(/\.mdx?$/, "");
}

function getMarker(source, file, errors) {
  const matches = markerMatches(source);
  if (matches.length !== 1) {
    errors.push(
      `${path.relative(repoRoot, file)}: expected exactly one i18n-sync marker`,
    );
    return null;
  }
  return { id: matches[0][1], digest: matches[0][2] };
}

function pairDigest(id, japanese, english) {
  const canonical = `${id}\n---ja---\n${withoutMarker(japanese)}\n---en---\n${withoutMarker(english)}`;
  return createHash("sha256")
    .update(canonical.replaceAll("\r\n", "\n"))
    .digest("hex");
}

function withoutMarker(source) {
  return markerPatterns
    .reduce((value, pattern) => {
      pattern.lastIndex = 0;
      return value.replace(pattern, "");
    }, source)
    .replace(/^\n+/, "");
}

function replaceMarker(source, file, id, digest) {
  const marker = file.endsWith(".mdx")
    ? `{/* i18n-sync: id=${id} digest=${digest} */}`
    : `<!-- i18n-sync: id=${id} digest=${digest} -->`;
  for (const pattern of markerPatterns) {
    pattern.lastIndex = 0;
    if (pattern.test(source)) {
      pattern.lastIndex = 0;
      return source.replace(pattern, marker);
    }
  }
  const frontmatterEnd = source.indexOf("---\n", 4);
  if (frontmatterEnd < 0)
    throw new Error(
      "All documentation must have frontmatter before marker update",
    );
  const insertAt = frontmatterEnd + 4;
  return `${source.slice(0, insertAt)}\n${marker}\n${source.slice(insertAt).replace(/^\n*/, "\n")}`;
}

function markerMatches(source) {
  return markerPatterns.flatMap((pattern) => {
    pattern.lastIndex = 0;
    return [...source.matchAll(pattern)];
  });
}

function headingShape(source) {
  const body = source.replace(/^---\n[\s\S]*?\n---\n/, "");
  const levels = [];
  let fenced = false;
  for (const line of body.split("\n")) {
    if (/^\s*```/.test(line)) fenced = !fenced;
    if (!fenced) {
      const match = /^(#{1,6})\s+/.exec(line);
      if (match) levels.push(match[1].length);
    }
  }
  return levels.join(",");
}

async function exists(file) {
  try {
    await readFile(file);
    return true;
  } catch {
    return false;
  }
}

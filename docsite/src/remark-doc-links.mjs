import path from "node:path";
import { fileURLToPath } from "node:url";

const docsiteRoot = fileURLToPath(new URL("..", import.meta.url));
const docsRoot = path.resolve(docsiteRoot, "../docs");
const externalUrl = /^(?:[a-z][a-z\d+.-]*:|\/\/)/i;

/**
 * Convert links to another authored doc into a route-relative URL.
 *
 * The source tree is deliberately outside src/content/docs, so Markdown's
 * source-relative URL is not the same as the generated Starlight page URL.
 * Relative output URLs are base-safe and keep the current locale prefix.
 */
export default function remarkDocLinks() {
  return (tree, file) => {
    if (!file.path) return;
    const source = path.resolve(file.path);
    if (!source.startsWith(`${docsRoot}${path.sep}`)) return;

    visitLinks(tree, (node) => {
      const [destination, fragment] = node.url.split("#", 2);
      if (
        !destination ||
        destination.startsWith("?") ||
        externalUrl.test(destination)
      ) {
        return;
      }

      const target = resolveDocTarget(source, destination);
      if (!target) return;

      const currentRoute = routeForDoc(source);
      const targetRoute = routeForDoc(target);
      const currentDirectory = path.posix.dirname(currentRoute);
      let relative = path.posix.relative(currentDirectory, targetRoute);
      if (!relative) relative = ".";
      if (!relative.endsWith("/")) relative += "/";
      node.url = relative + (fragment ? `#${fragment}` : "");
    });
  };
}

function resolveDocTarget(source, destination) {
  let candidate;
  if (destination.startsWith("/docs/")) {
    candidate = path.join(docsRoot, destination.slice("/docs/".length));
  } else if (destination === "/docs" || destination === "/docs/") {
    candidate = path.join(docsRoot, "index.mdx");
  } else {
    candidate = path.resolve(path.dirname(source), destination);
  }

  if (!candidate.startsWith(`${docsRoot}${path.sep}`) && candidate !== docsRoot)
    return null;
  if (path.extname(candidate)) {
    return isDocFile(candidate) ? candidate : null;
  }
  for (const extension of [".mdx", ".md"]) {
    const withExtension = `${candidate}${extension}`;
    if (isDocFile(withExtension)) return withExtension;
  }
  const index = path.join(candidate, "index.mdx");
  if (isDocFile(index)) return index;
  const markdownIndex = path.join(candidate, "index.md");
  return isDocFile(markdownIndex) ? markdownIndex : null;
}

function isDocFile(file) {
  return file.endsWith(".md") || file.endsWith(".mdx");
}

function routeForDoc(file) {
  const relative = path.relative(docsRoot, file).replaceAll(path.sep, "/");
  const segments = relative.split("/");
  const locale = segments[0] === "en" ? segments.shift() : undefined;
  const withoutExtension = segments.join("/").replace(/\.mdx?$/, "");
  const topic = withoutExtension === "index" ? "" : withoutExtension;
  const route = topic ? `docs/${topic}` : "";
  return `/${locale ? `${locale}/` : ""}${route ? `${route}/` : ""}`;
}

function visitLinks(node, callback) {
  if (!node || typeof node !== "object") return;
  if (node.type === "link" && typeof node.url === "string") callback(node);
  for (const value of Object.values(node)) {
    if (Array.isArray(value))
      value.forEach((child) => visitLinks(child, callback));
    else if (value && typeof value === "object") visitLinks(value, callback);
  }
}

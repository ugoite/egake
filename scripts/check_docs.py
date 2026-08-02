"""Validate local Markdown links used by the repository documentation."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit

MARKDOWN_LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
SKIPPED_SCHEMES = ("http://", "https://", "mailto:")


def markdown_files(root: Path) -> list[Path]:
    """Return tracked documentation files without entering generated output."""

    return sorted(
        path
        for path in root.rglob("*")
        if path.suffix in {".md", ".mdx"}
        if ".git" not in path.parts and "target" not in path.parts and "node_modules" not in path.parts
    )


def local_target(root: Path, source: Path, destination: str) -> Path | None:
    """Resolve a Markdown destination when it refers to a local file."""

    destination = destination.strip()
    if not destination or destination.startswith("#") or destination.startswith(SKIPPED_SCHEMES):
        return None
    path_part = unquote(destination.split("#", 1)[0]).strip("<>")
    if not path_part:
        return None
    generated_target = generated_doc_target(root, source, path_part)
    if generated_target is not None:
        return generated_target
    return (source.parent / path_part).resolve()


def generated_doc_target(root: Path, source: Path, destination: str) -> Path | None:
    """Resolve route-relative links against the generated Starlight URL tree."""

    if not destination.endswith("/"):
        return None
    docs_root = root / "docs"
    try:
        relative = source.relative_to(docs_root).as_posix()
    except ValueError:
        return None
    segments = relative.split("/")
    locale = "en" if segments[0] == "en" else None
    if locale:
        segments.pop(0)
    topic = "/".join(segments)
    topic = re.sub(r"\.mdx?$", "", topic)
    if topic == "index":
        topic = ""
    elif topic.endswith("/index"):
        topic = topic.removesuffix("/index")
    current_route = f"/{locale + '/' if locale else ''}{'' if not topic else 'docs/' + topic}/"
    resolved = urlsplit(urljoin(current_route, destination)).path.strip("/")
    resolved_parts = resolved.split("/") if resolved else []
    if resolved_parts and resolved_parts[0] == "en":
        if locale != "en":
            return None
        resolved_parts.pop(0)
    elif locale == "en":
        return None
    if not resolved_parts:
        return docs_root / ("en/index.mdx" if locale == "en" else "index.mdx")
    if resolved_parts[0] != "docs":
        return None
    target = docs_root / ("en" if locale == "en" else "") / "/".join(resolved_parts[1:])
    for extension in (".mdx", ".md"):
        candidate = target.with_suffix(extension)
        if candidate.is_file():
            return candidate
    for extension in (".mdx", ".md"):
        candidate = target / f"index{extension}"
        if candidate.is_file():
            return candidate
    return target


def broken_links(root: Path) -> list[str]:
    """Return human-readable errors for missing or escaping local links."""

    resolved_root = root.resolve()
    errors: list[str] = []
    for source in markdown_files(root):
        content = source.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK.finditer(content):
            target = local_target(root, source, match.group(1))
            if target is None:
                continue
            try:
                target.relative_to(resolved_root)
            except ValueError:
                errors.append(f"{source}: link escapes repository: {match.group(1)}")
            else:
                if not target.exists():
                    errors.append(f"{source}: missing link target: {match.group(1)}")
    return errors


def main() -> int:
    """Run the documentation link check."""

    root = Path(__file__).resolve().parents[1]
    errors = broken_links(root)
    if errors:
        print("Documentation link check failed:", file=sys.stderr)
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Documentation link check passed ({len(markdown_files(root))} Markdown files).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

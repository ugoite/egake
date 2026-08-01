"""Validate local Markdown links used by the repository documentation."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

MARKDOWN_LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
SKIPPED_SCHEMES = ("http://", "https://", "mailto:")


def markdown_files(root: Path) -> list[Path]:
    """Return tracked documentation files without entering generated output."""

    return sorted(
        path
        for path in root.rglob("*.md")
        if ".git" not in path.parts and "target" not in path.parts and "node_modules" not in path.parts
    )


def local_target(source: Path, destination: str) -> Path | None:
    """Resolve a Markdown destination when it refers to a local file."""

    destination = destination.strip()
    if not destination or destination.startswith("#") or destination.startswith(SKIPPED_SCHEMES):
        return None
    path_part = unquote(destination.split("#", 1)[0]).strip("<>")
    if not path_part:
        return None
    return (source.parent / path_part).resolve()


def broken_links(root: Path) -> list[str]:
    """Return human-readable errors for missing or escaping local links."""

    resolved_root = root.resolve()
    errors: list[str] = []
    for source in markdown_files(root):
        content = source.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK.finditer(content):
            target = local_target(source, match.group(1))
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

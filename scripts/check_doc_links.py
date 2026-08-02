"""Check internal links in a built Starlight site for files and fragments."""

from __future__ import annotations

import argparse
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit

SKIPPED_SCHEMES = {"data", "javascript", "mailto", "http", "https"}
HTML_SUFFIXES = {".html", ".htm"}


class DocumentParser(HTMLParser):
    """Collect URL attributes and fragment targets from one HTML document."""

    def __init__(self) -> None:
        super().__init__()
        self.references: list[tuple[str, str]] = []
        self.fragments: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        for attribute in ("href", "src"):
            value = values.get(attribute)
            if value is not None:
                self.references.append((attribute, value))
        for attribute in ("id", "name"):
            value = values.get(attribute)
            if value:
                self.fragments.add(value)


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--base", default="/")
    return parser.parse_args()


def normalize_base(value: str) -> str:
    value = value.strip()
    if not value or value == "/":
        return "/"
    return f"/{value.strip('/')}"


def page_url(relative: Path, base: str) -> str:
    relative_url = relative.as_posix()
    if relative_url == "index.html":
        path = "/"
    elif relative.name == "index.html":
        path = f"/{relative.parent.as_posix().strip('/')}/"
    else:
        path = f"/{relative_url}"
    return f"{base}{path}" if base != "/" else path


def html_files(dist: Path) -> list[Path]:
    return sorted(path for path in dist.rglob("*.html") if path.is_file())


def resolve_target(dist: Path, url_path: str, base: str) -> Path | None:
    if not url_path.startswith("/"):
        raise ValueError(f"resolved URL is not root-relative: {url_path}")
    if base != "/":
        if url_path != base and not url_path.startswith(f"{base}/"):
            raise ValueError(f"URL escapes configured base {base}/: {url_path}")
        relative_url = url_path[len(base) :].lstrip("/")
    else:
        relative_url = url_path.lstrip("/")
    relative_url = unquote(relative_url)
    if "\x00" in relative_url:
        raise ValueError(f"URL contains NUL: {url_path}")

    candidate = (dist / relative_url).resolve()
    try:
        candidate.relative_to(dist.resolve())
    except ValueError as error:
        raise ValueError(f"URL escapes dist: {url_path}") from error

    if candidate.is_dir():
        candidate /= "index.html"
    elif not candidate.exists() and not candidate.suffix:
        candidate /= "index.html"
    return candidate if candidate.is_file() else None


def fragment_exists(target: Path, fragment: str, parsed: dict[Path, DocumentParser]) -> bool:
    if target.suffix.lower() not in HTML_SUFFIXES:
        return True
    if target not in parsed:
        parser = DocumentParser()
        parser.feed(target.read_text(encoding="utf-8"))
        parsed[target] = parser
    return unquote(fragment) in parsed[target].fragments


def check(dist: Path, base: str) -> list[str]:
    errors: list[str] = []
    parsed: dict[Path, DocumentParser] = {}
    pages = html_files(dist)
    if not pages:
        return [f"no HTML files found under {dist}"]

    for page in pages:
        parser = DocumentParser()
        parser.feed(page.read_text(encoding="utf-8"))
        parsed[page] = parser
        current_url = page_url(page.relative_to(dist), base)
        for attribute, reference in parser.references:
            split = urlsplit(reference)
            if split.scheme.lower() in SKIPPED_SCHEMES or split.netloc:
                continue
            if split.path == "" and split.fragment:
                if not fragment_exists(page, split.fragment, parsed):
                    errors.append(f"{page}: missing fragment #{split.fragment}")
                continue

            resolved = urlsplit(urljoin(current_url, reference))
            if resolved.scheme or resolved.netloc:
                continue
            try:
                target = resolve_target(dist, resolved.path, base)
            except ValueError as error:
                errors.append(f"{page}: {attribute}={reference!r}: {error}")
                continue
            if target is None:
                errors.append(f"{page}: {attribute}={reference!r}: target does not exist")
                continue
            if resolved.fragment and not fragment_exists(target, resolved.fragment, parsed):
                errors.append(f"{page}: {attribute}={reference!r}: fragment does not exist")
    return errors


def main() -> int:
    args = arguments()
    dist = args.dist.resolve()
    base = normalize_base(args.base)
    errors = check(dist, base)
    if errors:
        print(f"Built-site link check failed for base {base}/:", file=sys.stderr)
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Built-site link check passed ({len(html_files(dist))} HTML files, base {base}/).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

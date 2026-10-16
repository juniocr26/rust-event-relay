#!/usr/bin/env python3
"""Check repository-local Markdown file links; never fetch remote URLs.

Includes historical documents without rewriting their dated claims. Anchors are
not checked: Markdown renderers differ in their heading-slug algorithms.
"""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def broken_links(document: Path, root: Path) -> list[str]:
    failures = []
    # Ignore fenced examples: their illustrative paths need not be real links.
    text = re.sub(r"```.*?```", "", document.read_text(), flags=re.S)
    for raw in LINK.findall(text):
        target = raw.strip().split(' "', 1)[0].strip("<>")
        url = urlsplit(target)
        if url.scheme or url.netloc or not url.path:
            continue
        path = (document.parent / unquote(url.path)).resolve()
        if not path.is_relative_to(root.resolve()) or not path.exists():
            failures.append(raw)
    return failures


def main() -> int:
    documents = sorted((ROOT / "docs").rglob("*.md"))
    documents += [ROOT / "README.md", ROOT / "README.pt-BR.md", ROOT / "frontend/README.md"]
    failures = [(p.relative_to(ROOT), link) for p in documents for link in broken_links(p, ROOT)]
    for path, link in failures:
        print(f"{path}: unresolved local link: {link}")
    print(f"Checked {len(documents)} Markdown files; {len(failures)} unresolved local file links. Heading anchors and remote links are outside this check.")
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())

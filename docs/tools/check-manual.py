#!/usr/bin/env python3
"""Deterministic structural checks for the Josh mdBook manual."""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote

DOCS = Path(__file__).resolve().parents[1]
SRC = DOCS / "src"
SUMMARY = SRC / "SUMMARY.md"
LINK_RE = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
ANCHOR_RE = re.compile(r'<a id="([A-Za-z0-9_-]+)"></a>')

errors: list[str] = []
runnable_examples: list[tuple[Path, int, str, str]] = []


class ArtifactParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.ids: list[str] = []
        self.pre_depth = 0
        self.code_blocks = 0
        self.literal_fences = 0
        self.current_row: list[str] | None = None
        self.current_cell: list[str] | None = None
        self.rows: list[list[str]] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        for name, value in attrs:
            if name == "id" and value is not None:
                self.ids.append(value)
        if tag == "pre":
            self.pre_depth += 1
        elif tag == "code" and self.pre_depth:
            self.code_blocks += 1
        elif tag == "tr":
            self.current_row = []
        elif tag in {"td", "th"} and self.current_row is not None:
            self.current_cell = []

    def handle_endtag(self, tag: str) -> None:
        if tag == "pre":
            self.pre_depth -= 1
        elif tag in {"td", "th"} and self.current_cell is not None:
            assert self.current_row is not None
            self.current_row.append("".join(self.current_cell).strip())
            self.current_cell = None
        elif tag == "tr" and self.current_row is not None:
            self.rows.append(self.current_row)
            self.current_row = None

    def handle_data(self, data: str) -> None:
        self.literal_fences += data.count("```")
        if self.current_cell is not None:
            self.current_cell.append(data)


def fail(message: str) -> None:
    errors.append(message)


def slugify(heading: str) -> str:
    heading = re.sub(r"<[^>]+>", "", heading).strip().lower()
    heading = re.sub(r"[^\w\- ]", "", heading, flags=re.UNICODE)
    return re.sub(r"[ _]+", "-", heading)


def split_target(raw: str) -> tuple[str, str]:
    raw = unquote(raw.split(maxsplit=1)[0].strip("<>"))
    path, marker, anchor = raw.partition("#")
    return path, anchor if marker else ""


def page_anchors(path: Path, text: str) -> set[str]:
    anchors = set(ANCHOR_RE.findall(text))
    for line in text.splitlines():
        if line.startswith("#"):
            title = line.lstrip("#").strip()
            anchors.add(slugify(title))
    return anchors


pages = sorted(SRC.rglob("*.md"))
if not SUMMARY.exists():
    fail("missing src/SUMMARY.md")
    pages = []

summary_text = SUMMARY.read_text(encoding="utf-8") if SUMMARY.exists() else ""
summary_paths: list[Path] = []
for raw in LINK_RE.findall(summary_text):
    path_text, _ = split_target(raw)
    if not path_text or "://" in path_text or path_text.startswith("mailto:"):
        continue
    target = (SRC / path_text).resolve()
    summary_paths.append(target)
    if not target.is_file():
        fail(f"SUMMARY target does not exist: {path_text}")

listed = set(summary_paths)
for page in pages:
    if page == SUMMARY:
        continue
    if page.resolve() not in listed:
        fail(f"page is not reachable from SUMMARY: {page.relative_to(SRC)}")
    text = page.read_text(encoding="utf-8")
    if "</p>\n```" in text:
        fail(f"raw HTML label absorbs following code fence: {page.relative_to(SRC)}")
    lines = text.splitlines()
    in_fence = False
    fence_label = ""
    fence_language = ""
    fence_start = 0
    fence_body: list[str] = []
    for index, line in enumerate(lines):
        if line.startswith("```"):
            if not in_fence:
                previous = index - 1
                while previous >= 0 and not lines[previous].strip():
                    previous -= 1
                fence_label = lines[previous] if previous >= 0 else ""
                fence_language = line.removeprefix("```").strip()
                fence_start = index + 1
                fence_body = []
                if "example-label" not in fence_label and "**Host command**" not in fence_label:
                    fail(f"unlabeled code fence at {page.relative_to(SRC)}:{index + 1}")
            elif "Runnable" in fence_label:
                body = "\n".join(fence_body).strip()
                if not body:
                    fail(f"empty runnable fence at {page.relative_to(SRC)}:{fence_start}")
                if not fence_language:
                    fail(f"runnable fence lacks a language at {page.relative_to(SRC)}:{fence_start}")
                runnable_examples.append((page, fence_start, fence_language, body))
            in_fence = not in_fence
        elif in_fence:
            fence_body.append(line)
    if in_fence:
        fail(f"unclosed code fence: {page.relative_to(SRC)}")

texts = {page.resolve(): page.read_text(encoding="utf-8") for page in pages}
anchors = {path: page_anchors(path, text) for path, text in texts.items()}
for page, text in texts.items():
    for raw in LINK_RE.findall(text):
        path_text, anchor = split_target(raw)
        if "://" in path_text or path_text.startswith(("mailto:", "javascript:")):
            continue
        target = page if not path_text else (page.parent / path_text).resolve()
        if target.suffix and target.suffix != ".md":
            if not target.exists():
                fail(f"broken asset link in {page.relative_to(SRC)}: {raw}")
            continue
        if target not in texts:
            fail(f"broken page link in {page.relative_to(SRC)}: {raw}")
            continue
        if anchor and anchor not in anchors[target]:
            fail(f"broken anchor in {page.relative_to(SRC)}: {raw}")

mdbook = shutil.which("mdbook")
if mdbook is None:
    print("mdbook not on PATH; skipping generated-HTML checks", file=sys.stderr)
else:
    with tempfile.TemporaryDirectory(prefix="josh-manual-check-") as directory:
        built = Path(directory)
        result = subprocess.run(
            [mdbook, "build", str(DOCS), "-d", str(built)],
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            fail(f"mdbook build failed: {result.stderr.strip()}")
        else:
            parsed_artifacts: dict[Path, ArtifactParser] = {}
            for artifact in built.rglob("*.html"):
                parser = ArtifactParser()
                parser.feed(artifact.read_text(encoding="utf-8"))
                parsed_artifacts[artifact] = parser
                duplicates = [value for value, count in Counter(parser.ids).items() if count > 1]
                if duplicates:
                    fail(f"duplicate generated IDs in {artifact.relative_to(built)}: {', '.join(duplicates)}")
                if parser.literal_fences:
                    fail(f"literal Markdown fences in generated {artifact.relative_to(built)}")
            for page, text in texts.items():
                if page == SUMMARY.resolve():
                    continue
                relative = page.relative_to(SRC).with_suffix(".html")
                artifact = built / relative
                parser = parsed_artifacts.get(artifact)
                if parser is None:
                    fail(f"missing generated page: {relative}")
                    continue
                source_fences = sum(1 for line in text.splitlines() if line.startswith("```")) // 2
                if parser.code_blocks < source_fences:
                    fail(f"{relative} rendered {parser.code_blocks} code blocks for {source_fences} source fences")
            cli_parser = parsed_artifacts.get(built / "agent-terminal/cli-reference.html")
            wait_rows = [] if cli_parser is None else [row for row in cli_parser.rows if row and row[0] == "wait"]
            if len(wait_rows) != 1 or len(wait_rows[0]) != 3 or "--stable DURATION" not in wait_rows[0][1] or not wait_rows[0][2].startswith("Session/revision/process"):
                fail("generated agent-terminal wait table row is malformed")

if errors:
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    print(f"manual checks failed: {len(errors)} error(s)", file=sys.stderr)
    raise SystemExit(1)
print(f"manual checks passed: {len(pages) - 1} pages")
print(f"runnable examples extracted: {len(runnable_examples)}")

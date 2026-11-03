#!/usr/bin/env python3
"""Run-aware find-and-replace for .docx, preserving formatting.

Why a script: in a .docx a single visible word is often split across several
runs (<w:r>), so a naive ``run.text.replace(...)`` silently misses matches that
span run boundaries. This walks every paragraph (body, tables — recursively —
headers and footers), reconstructs the full paragraph text, and rewrites only
the runs the match touches, keeping the formatting of the run where the match
starts.

Usage:
    python replace.py in.docx out.docx --replace "{{year}}" "2026"
    python replace.py in.docx out.docx -r "OLD" "NEW" -r "FOO" "BAR"
    python replace.py in.docx out.docx --map replacements.json   # {"old": "new"}

Requires: python-docx  (pip install python-docx)
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterator

from docx import Document
from docx.document import Document as _Doc
from docx.table import Table
from docx.text.paragraph import Paragraph


def _iter_block_paragraphs(parent: object) -> Iterator[Paragraph]:
    """Yield every paragraph under *parent*, descending into tables."""
    if isinstance(parent, _Doc):
        container = parent.element.body
    else:  # a table cell
        container = parent._tc  # type: ignore[attr-defined]

    for child in container.iterchildren():
        if child.tag.endswith("}p"):
            yield Paragraph(child, parent)  # type: ignore[arg-type]
        elif child.tag.endswith("}tbl"):
            table = Table(child, parent)  # type: ignore[arg-type]
            for row in table.rows:
                for cell in row.cells:
                    yield from _iter_block_paragraphs(cell)


def _all_paragraphs(doc: _Doc) -> Iterator[Paragraph]:
    yield from _iter_block_paragraphs(doc)
    for section in doc.sections:
        for hf in (section.header, section.footer,
                   section.first_page_header, section.first_page_footer,
                   section.even_page_header, section.even_page_footer):
            for para in hf.paragraphs:
                yield para
            for table in hf.tables:
                for row in table.rows:
                    for cell in row.cells:
                        yield from _iter_block_paragraphs(cell)


def replace_in_paragraph(paragraph: Paragraph, old: str, new: str) -> int:
    """Replace every ``old`` with ``new`` in one paragraph. Returns count.

    The replacement text lands in the run where the match starts (inheriting its
    formatting); the rest of the match is cut from the runs it spanned.
    """
    runs = paragraph.runs
    if not runs or not old:
        return 0

    count = 0
    search_from = 0
    text = "".join(r.text for r in runs)
    while True:
        start = text.find(old, search_from)
        if start == -1:
            break
        end = start + len(old)

        pos = 0
        first = True
        for run in runs:
            r_start = pos
            r_end = pos + len(run.text)
            pos = r_end
            if r_end <= start or r_start >= end:
                continue  # this run is outside the match
            local_start = max(start, r_start) - r_start
            local_end = min(end, r_end) - r_start
            if first:
                tail = run.text[local_end:] if r_end >= end else ""
                run.text = run.text[:local_start] + new + tail
                first = False
            else:
                run.text = run.text[:local_start] + run.text[local_end:]

        count += 1
        search_from = start + len(new)  # skip past the inserted text (handles new ⊇ old)
        text = "".join(r.text for r in runs)

    return count


def main() -> int:
    ap = argparse.ArgumentParser(description="Run-aware find-and-replace for .docx")
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("-r", "--replace", nargs=2, action="append", metavar=("OLD", "NEW"),
                    default=[], help="a replacement pair; repeatable")
    ap.add_argument("--map", help="JSON file of {\"old\": \"new\", ...}")
    args = ap.parse_args()

    pairs: list[tuple[str, str]] = [(o, n) for o, n in args.replace]
    if args.map:
        with open(args.map, encoding="utf-8") as fh:
            pairs.extend((str(k), str(v)) for k, v in json.load(fh).items())
    if not pairs:
        ap.error("provide at least one --replace OLD NEW or --map file.json")

    doc = Document(args.input)
    paragraphs = list(_all_paragraphs(doc))
    total = 0
    per_pair: dict[str, int] = {}
    for old, new in pairs:
        n = sum(replace_in_paragraph(p, old, new) for p in paragraphs)
        per_pair[old] = n
        total += n

    doc.save(args.output)
    for old, n in per_pair.items():
        print(f"  {old!r}: {n} replacement(s)")
    print(f"Replaced {total} occurrence(s) → {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

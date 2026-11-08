---
name: docx
description: "Use for creating, reading, editing, or converting Word documents and templates (.docx, .dotx), including styled reports, tables, page numbers, find and replace, comments, and tracked changes."
metadata:
  version: "1.2.0"
  author: Yaca
---

# Word documents (.docx)

Adapted for Yaca from [Anthropic's Word skill](https://github.com/anthropics/skills/tree/main/skills/docx)
and the existing Yaca/AgentChat workflow. See [Apache-2.0](../ANTHROPIC-LICENSE.txt)
for the Anthropic material.

Use **python-docx** for new documents and ordinary edits, and pandoc or
LibreOffice for conversions. For features the library cannot represent, modify
only the affected OOXML parts and preserve the rest of the package.

If the project already uses the npm `docx` package, it can generate new
documents but cannot load a Word file for editing. For fixed-width tables in
that library, set both the table's column widths and each cell's width in DXA.
Use the library's numbering configuration for lists so Word can renumber them.

## Hard rule: generate with code, never write the file as text

A `.docx` is a ZIP container, not text. **Never** pass a `.docx` path to
`Write` or `Edit` — a text write would leave the user with a file Word cannot
open. Every document below is produced by *running Python* through `Bash`.

The generator script is ordinary text, so write it the normal way — `Write` a
`.py` (the user watches it stream), then run it:

```python
# build_report.py  — written with Write
from docx import Document

doc = Document()
doc.add_heading("Quarterly Report", level=0)
doc.add_paragraph("Revenue grew 15% year over year.")
doc.save("report.docx")
```

```powershell
python build_report.py
```

The shell is whatever `Bash` says in the environment section: on Windows that
is PowerShell, so a heredoc (`python - <<'PY' …`) is not available there —
write the script file instead, which is the better habit anyway.

Prefer the script for anything non-trivial: it is visible, re-runnable, and
easy to fix. The same rule holds for editing — read the file with python-docx,
change it in memory, save it again; never do string surgery on the bytes.

## Reading a document

`Read` opens a `.docx` itself: it returns the paragraphs, headings as Markdown
ones, tables as Markdown tables, and the pictures inside as images you are
shown. That is the way to read a document — no Python needed, and it works
before any dependency is installed.

Use Python when you need something `Read` does not give: paragraph styles by
name, the section setup, headers and footers, or a programmatic walk of the
document.

```python
from docx import Document

doc = Document("input.docx")
for p in doc.paragraphs:
    print(p.style.name, "|", p.text)
for table in doc.tables:
    for row in table.rows:
        print([cell.text for cell in row.cells])
```

`ExtractImages` writes the pictures of a document out as files when they are
to be reused rather than looked at.

Legacy `.doc` must be converted first (needs LibreOffice):

```powershell
soffice --headless --convert-to docx input.doc --outdir .
```

## Creating a document

```python
from docx import Document
from docx.shared import Pt, Inches, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH

doc = Document()

# Default font for the whole document
style = doc.styles["Normal"]
style.font.name = "Calibri"
style.font.size = Pt(11)

doc.add_heading("Quarterly Report", level=0)        # title
doc.add_heading("Summary", level=1)
p = doc.add_paragraph("Revenue grew ")
p.add_run("15%").bold = True
p.add_run(" year over year.")

# Bulleted / numbered lists use built-in styles (never type "•" yourself)
doc.add_paragraph("First point", style="List Bullet")
doc.add_paragraph("Step one", style="List Number")

doc.save("report.docx")
```

### Tables

```python
table = doc.add_table(rows=1, cols=3)
table.style = "Light Grid Accent 1"          # a built-in style → clean borders
hdr = table.rows[0].cells
hdr[0].text, hdr[1].text, hdr[2].text = "Item", "Qty", "Price"
for item, qty, price in rows_data:
    cells = table.add_row().cells
    cells[0].text, cells[1].text, cells[2].text = item, str(qty), f"${price}"
```

### Images

```python
from docx.shared import Inches
doc.add_picture("chart.png", width=Inches(6))   # width keeps aspect ratio
```

The picture has to exist as a file first. A picture taken out of another
document comes from `ExtractImages`; one the user described is made with
matplotlib or another generator, not pasted from nothing.

### Page setup, headers/footers, page breaks

```python
from docx.shared import Inches
from docx.enum.section import WD_ORIENT

section = doc.sections[0]
section.page_width, section.page_height = Inches(8.5), Inches(11)   # US Letter
section.left_margin = section.right_margin = Inches(1)

section.header.paragraphs[0].text = "Company Name"
section.footer.paragraphs[0].text = "Confidential"

doc.add_page_break()
```

Use a `PAGE` field for automatic page numbers. A table of contents needs real
heading styles or styles with outline levels; verify the fields after updating
them in a compatible editor.

## Editing an existing document

### Find-and-replace — use the bundled script

A visible word in Word is often split across several runs, so naive
`run.text.replace(...)` silently misses matches that cross run boundaries. Use
**`scripts/replace.py`**, which is run-aware and preserves formatting (it walks
the body, tables, headers, and footers):

```powershell
python scripts/replace.py input.docx output.docx --replace "{{year}}" "2026"
python scripts/replace.py input.docx output.docx -r "{{name}}" "Acme" -r "{{date}}" "Jun 2026"
python scripts/replace.py input.docx output.docx --map tokens.json   # {"{{a}}": "b"}
```

Run it by its absolute path: the skill carries the directory of this file in
the header of the `Skill` result, and a relative path in the person's folder
does not point at it.

### Other edits in Python

Edit individual runs when a paragraph's inline styling or links matter.
Assigning `paragraph.text` rebuilds the runs, so use it only when replacing the
whole paragraph is appropriate.

```python
from docx import Document
doc = Document("input.docx")
doc.paragraphs[0].text = "New title"
doc.add_paragraph("Appended note.")
doc.save("output.docx")
```

For redlines, use Word-compatible tracked changes. If writing OOXML directly,
wrap inserted/deleted runs in `w:ins`/`w:del` with distinct IDs and the requested
author. Deleted text uses `w:delText`. Preserve existing revisions and check
that rejecting your changes restores the original text. For comments, use
`Document.add_comment` when available and anchor the comment to the relevant
runs; check that the comment appears on the intended passage in the editor.

## Converting Markdown → .docx

When the user gives Markdown/prose, pandoc is the fastest path:

```powershell
pandoc notes.md -o notes.docx
# Use a reference doc to control styles/fonts:
pandoc notes.md --reference-doc=template.docx -o notes.docx
```

## Check the result

Use `Read` to check structure and text. For layout-sensitive work, render a
temporary PDF with `soffice --headless --convert-to pdf report.docx --outdir .`,
then rasterize it with Poppler or PyMuPDF and inspect pages with `ReadImage`.
Look for split tables, clipped text, unexpected page breaks, and missing
glyphs. If rendering tools are unavailable, say what you could verify.

## Finish

- Save the final file at the requested path and deliver it with `Send`.
- **Dependencies:** Python with **`python-docx`**. Optional: **pandoc** (rich
  read / Markdown→docx), **LibreOffice** (`.doc`→`.docx` and PDF export).
- **Fix the script, not the file.** When the document is wrong, change the
  generator and run it again; that is the whole point of writing one.

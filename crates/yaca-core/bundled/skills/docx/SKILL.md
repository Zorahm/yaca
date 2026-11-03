---
name: docx
description: "Use when the user wants to create, read, edit, or convert Word documents (.docx). Triggers: 'Word doc', '.docx', a report/memo/letter/template deliverable, extracting or reorganizing content from a Word file, adding headings/tables/images/page numbers/table of contents, find-and-replace in a Word file, or converting Markdown/text to .docx (or .doc to .docx). Do NOT use for PDFs, spreadsheets, or presentations."
version: "1.1.0"
author: Yaca
---

# Word documents (.docx)

Create, read, and edit `.docx` files with the open-source **python-docx**
library, plus `pandoc`/LibreOffice for conversions. Adapted for Yaca from the
AgentChat office skills.

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

For automatic page numbers in the footer, python-docx needs a raw field — add a
`PAGE` field run; for most reports a static footer is enough.

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

```python
from docx import Document
doc = Document("input.docx")
doc.paragraphs[0].text = "New title"
doc.add_paragraph("Appended note.")
doc.save("output.docx")
```

## Converting Markdown → .docx

When the user gives Markdown/prose, pandoc is the fastest path:

```powershell
pandoc notes.md -o notes.docx
# Use a reference doc to control styles/fonts:
pandoc notes.md --reference-doc=template.docx -o notes.docx
```

## Export to PDF (optional)

```powershell
soffice --headless --convert-to pdf report.docx --outdir .
```

## Finish

- **Save in the person's folder.** Write the final document to the working
  folder with a relative path (e.g. `report.docx`); the feed shows it as a
  changed file, which is how they open it.
- **Read the result back** with `Read` before saying it is done: a heading
  level, a table or an image that did not land is cheap to notice now and
  expensive after the user opens the file.
- **Dependencies:** Python with **`python-docx`**. Optional: **pandoc** (rich
  read / Markdown→docx), **LibreOffice** (`.doc`→`.docx` and PDF export). If a
  tool is missing, say so and let the person install it — do not install it
  yourself.
- **Fix the script, not the file.** When the document is wrong, change the
  generator and run it again; that is the whole point of writing one.

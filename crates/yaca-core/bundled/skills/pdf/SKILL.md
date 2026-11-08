---
name: pdf
description: "Use when a PDF is a primary input or deliverable, including text and table extraction, OCR, page operations, forms, and creating or verifying a PDF."
metadata:
  version: "1.2.0"
  author: Yaca
---

# PDF files

Adapted for Yaca from [Anthropic's PDF skill](https://github.com/anthropics/skills/tree/main/skills/pdf).
See [Apache-2.0](../ANTHROPIC-LICENSE.txt).

## Read and extract

- Start with Yaca's `Read` for text. Use the `pages` argument, for example
  `"3-7"`, to read a range in a longer file. Page numbers are one-based, and
  each call can read up to 20 pages.
- Use pdfplumber for text whose position matters or to extract tables. Check
  sample rows against the rendered page, especially for merged cells and
  multi-line values.
- For scans, render the pages and OCR them with Tesseract using the document's
  language. Check the output against the image. Use OCRmyPDF if the requested
  deliverable is a searchable PDF rather than just extracted text.
- Use a renderer such as Poppler or PyMuPDF to inspect layout, images, and
  pages without a text layer. View the rendered files with `ReadImage`.

## Create or change a file

Write a script and run it through `Bash` with the session's shell syntax. The
libraries below are options; check what is installed in the environment.

| Task | Approach |
|---|---|
| Export an existing Word, Excel, or PowerPoint file | Use that format's skill and export from a compatible editor. This retains the source layout better than rebuilding it. |
| Build a new PDF | Use ReportLab. Prefer Platypus for flowing text, tables, and pagination. Embed a font with the required glyphs, especially for non-Latin text. |
| Merge, split, rotate, or watermark pages | Use pypdf or qpdf. Preserve page order and verify page boxes, links, or bookmarks when they matter to the result. |
| Fill an interactive form | Inspect the field names and types with pypdf, clone the original document, and update the relevant fields and appearances. Retain editability unless flattening was requested. |
| Fill a flat form | Render and locate the boxes, draw a text overlay in PDF coordinates, accounting for the page crop and rotation, and merge it onto a copy of the original. |

For example, to merge complete documents with pypdf:

```python
from pypdf import PdfWriter

writer = PdfWriter()
for source in ("cover.pdf", "body.pdf"):
    writer.append(source)
with open("combined.pdf", "wb") as output:
    writer.write(output)
```

## Verify and deliver

Reopen the result. Check the number and order of pages and the text or form
values that were meant to change. Render changed pages for visual work and
check for clipping, missing glyphs, or invisible field values. For a populated
form, also confirm the values persist after reopening. If a rendering or OCR
tool is unavailable, state the verification limit.

Save the final file at the requested path and deliver it with `Send`.

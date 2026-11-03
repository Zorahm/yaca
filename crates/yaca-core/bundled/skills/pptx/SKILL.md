---
name: pptx
description: "Use whenever a .pptx is involved — input, output, or both: creating slide decks / pitch decks / presentations, reading or extracting text from a .pptx, editing or restyling an existing deck, adding slides, images, tables, charts, or speaker notes, and exporting slides to PDF or images. Trigger when the user mentions 'deck', 'slides', 'presentation', or a .pptx filename. Do NOT use for Word docs, spreadsheets, or PDFs as the primary deliverable."
version: "1.1.0"
author: Yaca
---

# Presentations (.pptx)

Create, read, and edit PowerPoint decks with the open-source **python-pptx**
library, plus LibreOffice for PDF/image export. Adapted for Yaca from the
AgentChat office skills.

## Hard rule: generate with code, never write the file as text

A `.pptx` is a ZIP container, not text. **Never** pass a `.pptx` path to
`Write` or `Edit` — a text write would leave the user with a file PowerPoint
cannot open. Every deck below is produced by *running Python* through `Bash`.

The generator script is ordinary text, so write it the normal way — `Write` a
`.py` (the user watches it stream), then run it:

```python
# build_deck.py  — written with Write
from pptx import Presentation

prs = Presentation()
slide = prs.slides.add_slide(prs.slide_layouts[0])
slide.shapes.title.text = "Quarterly Review"
slide.placeholders[1].text = "Revenue grew 15%"
prs.save("deck.pptx")
```

```powershell
python build_deck.py
```

Prefer the script for a real deck — slide loops get long, and a file is easy to
re-run after a fix.

## Reading a deck

`Read` opens a `.pptx` itself: slide by slide, with the text of every shape and
the speaker notes, and the pictures shown as images. That is the way to read a
deck — no Python needed.

Use Python when extracting runs with their fonts, the notes of one slide in
particular, or a programmatic walk of the shapes.

```python
from pptx import Presentation

prs = Presentation("input.pptx")
for i, slide in enumerate(prs.slides, start=1):
    print(f"--- Slide {i} ---")
    for shape in slide.shapes:
        if shape.has_text_frame:
            for para in shape.text_frame.paragraphs:
                print("".join(run.text for run in para.runs))
    if slide.has_notes_slide:
        print("NOTES:", slide.notes_slide.notes_text_frame.text)
```

`ExtractImages` writes the pictures of a deck out as files when they are to be
reused rather than looked at.

## Creating a deck

Work from the built-in layouts of the default template (indexes are stable for
the default template: 0 = Title, 1 = Title+Content, 5 = Title Only, 6 = Blank).

```python
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN

prs = Presentation()
prs.slide_width, prs.slide_height = Inches(13.333), Inches(7.5)   # 16:9

# Title slide
slide = prs.slides.add_slide(prs.slide_layouts[0])
slide.shapes.title.text = "Quarterly Review"
slide.placeholders[1].text = "FY2026 · Q1"

# Bulleted content slide
slide = prs.slides.add_slide(prs.slide_layouts[1])
slide.shapes.title.text = "Highlights"
body = slide.placeholders[1].text_frame
body.text = "Revenue up 15%"
for line in ("Churn down to 2%", "Two new markets"):
    p = body.add_paragraph()
    p.text = line
    p.level = 0

prs.save("deck.pptx")
```

### Free-form text box, colors, fonts

```python
slide = prs.slides.add_slide(prs.slide_layouts[6])      # blank
box = slide.shapes.add_textbox(Inches(1), Inches(1), Inches(11), Inches(1.5))
tf = box.text_frame
tf.text = "Big Statement"
run = tf.paragraphs[0].runs[0]
run.font.size = Pt(40)
run.font.bold = True
run.font.color.rgb = RGBColor(0x1F, 0x4E, 0x78)
tf.paragraphs[0].alignment = PP_ALIGN.CENTER
```

### Images

```python
slide.shapes.add_picture("chart.png", Inches(1), Inches(2), width=Inches(8))
```

The picture has to exist as a file first — made with matplotlib, or taken out
of another document with `ExtractImages`.

### Tables

```python
rows, cols = 3, 3
tbl = slide.shapes.add_table(rows, cols, Inches(1), Inches(2),
                             Inches(11), Inches(3)).table
for c, head in enumerate(("Item", "Q1", "Q2")):
    tbl.cell(0, c).text = head
tbl.cell(1, 0).text = "Revenue"
tbl.cell(1, 1).text = "$1.2M"
```

### Charts

```python
from pptx.chart.data import CategoryChartData
from pptx.enum.chart import XL_CHART_TYPE

data = CategoryChartData()
data.categories = ["Jan", "Feb", "Mar"]
data.add_series("Revenue", (1.0, 1.2, 1.4))
slide.shapes.add_chart(XL_CHART_TYPE.COLUMN_CLUSTERED,
                       Inches(1), Inches(2), Inches(8), Inches(4.5), data)
```

### Speaker notes

```python
slide.notes_slide.notes_text_frame.text = "Emphasize the 15% number."
```

## Editing an existing deck

```python
from pptx import Presentation
prs = Presentation("input.pptx")

# Replace a placeholder string everywhere
for slide in prs.slides:
    for shape in slide.shapes:
        if shape.has_text_frame and "{{client}}" in shape.text_frame.text:
            for p in shape.text_frame.paragraphs:
                for run in p.runs:
                    run.text = run.text.replace("{{client}}", "Acme Inc.")

prs.save("output.pptx")
```

To start from the user's branded template, open *their* `.pptx` and add slides to
it — new slides inherit its theme, fonts, and colors.

## Export to PDF / images (optional)

```powershell
soffice --headless --convert-to pdf deck.pptx --outdir .
```

Slide by slide, so the look can be checked:

```python
# slide_images.py — needs PyMuPDF: pip install pymupdf
import fitz
doc = fitz.open("deck.pdf")
for i, page in enumerate(doc, start=1):
    page.get_pixmap(dpi=110).save(f"slide-{i}.png")
```

Use the images to **visually verify** the deck — look at them with `ReadImage`
— then fix any overflowing text or overlaps before presenting the `.pptx`.

## Finish

- **Save in the person's folder** with a relative path (e.g. `deck.pptx`); the
  feed shows it as a changed file, which is how they open it.
- **Read the result back** with `Read`, and look at the exported images before
  saying it is done.
- **Dependencies:** Python with **`python-pptx`**. Optional: **LibreOffice** for
  PDF export, **PyMuPDF** for slide images. If a tool is missing, say so and let
  the person install it — do not install it yourself.
- Keep one idea per slide; large readable type (titles ≥ 28pt, body ≥ 18pt).
- Reuse layout placeholders instead of free text boxes where possible — they keep
  the deck's theme and alignment consistent.
- Render data as a native chart or a pre-made image, not a wall of numbers.

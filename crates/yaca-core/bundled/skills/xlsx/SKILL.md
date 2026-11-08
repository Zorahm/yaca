---
name: xlsx
description: "Use when a spreadsheet (.xlsx, .xlsm, .xltx, .csv, or .tsv) is a primary input or deliverable, including workbook editing, formulas, formatting, charts, data cleaning, and tabular conversion."
metadata:
  version: "1.2.0"
  author: Yaca
---

# Spreadsheets (.xlsx / .csv)

Adapted for Yaca from [Anthropic's spreadsheet skill](https://github.com/anthropics/skills/tree/main/skills/xlsx)
and the existing Yaca/AgentChat workflow. See [Apache-2.0](../ANTHROPIC-LICENSE.txt)
for the Anthropic material.

Use **openpyxl** for workbook structure, formulas, and formatting and
**pandas** for bulk data.

## Hard rule: generate with code, never write the file as text

An `.xlsx` is a ZIP container, not text. **Never** pass an `.xlsx` path to
`Write` or `Edit` — a text write would leave the user with a file Excel cannot
open. Every workbook below is produced by *running Python* through `Bash`.

The generator script is ordinary text, so write it the normal way — `Write` a
`.py` (the user watches it stream), then run it:

```python
# build_report.py  — written with Write
from openpyxl import Workbook

wb = Workbook()
ws = wb.active
ws.title = "Q1"
ws.append(["Item", "Qty", "Price"])
ws.append(["Widget", 12, 9.99])
wb.save("report.xlsx")
```

```powershell
python build_report.py
```

`.csv` is the exception in this skill — it *is* text, so `Write` is fine for
it (though pandas `.to_csv()` is less error-prone once there's real data).

## Reading a workbook

`Read` opens an `.xlsx` itself: it returns one table per sheet, with shared
strings and dates already resolved, up to 200 rows and 40 columns per sheet,
and says where it stopped. That is the way to look at a workbook — no Python
needed.

Go to Python when the sheet is too big for that, when you need cell
coordinates rather than a table, or when you are about to change the file.

## Choosing the tool

- **pandas** — reading/cleaning/transforming lots of rows, CSV↔XLSX, quick stats.
- **openpyxl** — formulas, cell styling, number formats, multiple sheets, charts,
  column widths. Use it when the output must *look* like a real spreadsheet.

## Reading data

```python
import pandas as pd
df = pd.read_excel("input.xlsx", sheet_name=0)   # or read_csv("input.csv")
print(df.head())
print(df.dtypes)
```

Read formulas vs. computed values with openpyxl:

```python
from openpyxl import load_workbook
wb = load_workbook("input.xlsx")            # formulas as strings
wb_vals = load_workbook("input.xlsx", data_only=True)   # last-cached values
ws = wb.active
print(ws["B2"].value)                       # e.g. "=A1*1.2"
```

Use `data_only=True` only for inspection. It exposes cached values and drops
formulas if you save that loaded workbook. Saving with openpyxl clears formula
caches; a compatible spreadsheet app must recalculate to restore them.

## Creating a spreadsheet

### From a DataFrame (fastest for data)

```python
import pandas as pd
with pd.ExcelWriter("report.xlsx", engine="openpyxl") as xl:
    df.to_excel(xl, sheet_name="Data", index=False)
    summary.to_excel(xl, sheet_name="Summary", index=False)
```

### With openpyxl (formatting + formulas)

```python
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
from openpyxl.utils import get_column_letter

wb = Workbook()
ws = wb.active
ws.title = "Sales"

headers = ["Month", "Units", "Price", "Revenue"]
ws.append(headers)

# Header styling
fill = PatternFill("solid", fgColor="1F4E78")
for col, _ in enumerate(headers, start=1):
    c = ws.cell(row=1, column=col)
    c.font = Font(bold=True, color="FFFFFF")
    c.fill = fill
    c.alignment = Alignment(horizontal="center")

data = [("Jan", 120, 9.99), ("Feb", 150, 9.99), ("Mar", 175, 8.99)]
for i, (m, units, price) in enumerate(data, start=2):
    ws.cell(row=i, column=1, value=m)
    ws.cell(row=i, column=2, value=units)
    ws.cell(row=i, column=3, value=price)
    ws.cell(row=i, column=4, value=f"=B{i}*C{i}")          # formula
    ws.cell(row=i, column=3).number_format = '"$"#,##0.00'
    ws.cell(row=i, column=4).number_format = '"$"#,##0.00'

last = len(data) + 1
ws.cell(row=last + 1, column=1, value="Total").font = Font(bold=True)
ws.cell(row=last + 1, column=4, value=f"=SUM(D2:D{last})")

# Auto-ish column widths
for col in range(1, len(headers) + 1):
    width = max(len(str(ws.cell(row=r, column=col).value or "")) for r in range(1, last + 2))
    ws.column_dimensions[get_column_letter(col)].width = width + 2

ws.freeze_panes = "A2"        # keep header visible
wb.save("report.xlsx")
```

### Charts

```python
from openpyxl.chart import BarChart, Reference
chart = BarChart()
chart.title = "Revenue by Month"
data = Reference(ws, min_col=4, min_row=1, max_row=last)
cats = Reference(ws, min_col=1, min_row=2, max_row=last)
chart.add_data(data, titles_from_data=True)
chart.set_categories(cats)
ws.add_chart(chart, "F2")
wb.save("report.xlsx")
```

## Editing an existing file

Load the workbook with formulas, edit only the intended cells, and preserve
the existing style and other sheets. Write merged cells through their top-left
anchor. For `.xlsm`, use `keep_vba=True` and retain the extension. Check
advanced features such as embedded controls before choosing a round trip.

```python
from openpyxl import load_workbook
wb = load_workbook("input.xlsx")
ws = wb["Sheet1"]
ws["C2"] = "=A2*B2"
wb.save("output.xlsx")
```

Structural edits need special care: inserting rows with openpyxl does not
update all dependent formulas, tables, or chart ranges. Verify affected
references explicitly. For cross-sheet formulas, quote sheet names containing
spaces, as in `='Sales Inputs'!$B$5`.

## Recalculating and checking formulas

Put derived values in formulas, with editable assumptions in labeled cells.
Check representative rows and boundary cases against independent expected
results. To verify cached results, recalculate a copy with Excel or LibreOffice:

```powershell
soffice --headless --convert-to xlsx --outdir recalced output.xlsx
```

Reopen the recalculated copy in both formula and `data_only` modes and check for
errors such as `#REF!`, `#DIV/0!`, and `#NAME?`. Avoid recalculating workbooks
with unavailable external links or unsupported formulas in LibreOffice; it
can replace valid cached values with errors or change the workbook. If a
compatible engine is unavailable, report that cached results were not verified.

## Cleaning messy data (pandas)

```python
import pandas as pd
df = pd.read_csv("messy.csv", skiprows=2)        # skip junk header rows
df.columns = [c.strip().lower() for c in df.columns]
df = df.dropna(how="all").drop_duplicates()
df["amount"] = pd.to_numeric(df["amount"], errors="coerce")
df.to_excel("clean.xlsx", index=False)
```

## Finish

- Save the final file at the requested path and deliver it with `Send`.
- **Read the result back** with `Read` before saying it is done: a sheet name,
  a header row or a formula that did not land is cheap to notice now.
- **Dependencies:** Python with **`openpyxl`**, and **`pandas`** for bulk data.
  Optional: **LibreOffice** for recalculating compatible workbooks.
- Use real **number formats** (`'"$"#,##0.00'`, `'0.0%'`, `'yyyy-mm-dd'`) instead
  of pre-formatting numbers into strings — keeps cells numeric.
- Use a consistent professional font; bold the header row and `freeze_panes`.
- Verify every cell a formula references exists before saving.

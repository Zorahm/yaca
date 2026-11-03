---
name: xlsx
description: "Use when a spreadsheet is the input or output: open/read/edit/fix an existing .xlsx, .xlsm, .csv, or .tsv (add columns, compute formulas, format, chart, clean messy data), create a spreadsheet from scratch or from data, or convert between tabular formats. Trigger when the user references a spreadsheet by name or wants a spreadsheet deliverable. Do NOT use when the deliverable is a Word doc, presentation, or plain script."
version: "1.1.0"
author: Yaca
---

# Spreadsheets (.xlsx / .csv)

Create, read, and edit spreadsheets with the open-source **openpyxl** (formatting
and formulas) and **pandas** (bulk data). Adapted for Yaca from the AgentChat
office skills.

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

> `data_only=True` returns the value **cached by the app that last saved the
> file**. A file written by openpyxl has no cached values until recalculated
> (see below).

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

`load_workbook` → change cells → `save`. openpyxl preserves existing formatting,
formulas, and other sheets:

```python
from openpyxl import load_workbook
wb = load_workbook("input.xlsx")
ws = wb["Sheet1"]
ws["C2"] = "=A2*B2"
ws.insert_rows(2)                      # insert a blank row
wb.save("output.xlsx")
```

## Recalculating formula values (optional)

openpyxl writes formula *strings* but does not compute them. If the user needs
the calculated values baked in (e.g. for `data_only` reads or downstream tools),
recalculate with LibreOffice headless:

```powershell
soffice --headless --convert-to xlsx --calc input.xlsx --outdir recalced/
```

(LibreOffice opens, recalculates, and re-saves. Requires LibreOffice installed.)

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

- **Save in the person's folder** with a relative path (e.g. `report.xlsx`);
  the feed shows it as a changed file, which is how they open it.
- **Read the result back** with `Read` before saying it is done: a sheet name,
  a header row or a formula that did not land is cheap to notice now.
- **Dependencies:** Python with **`openpyxl`**, and **`pandas`** for bulk data.
  Optional: **LibreOffice** for recalculating formula values. If a tool is
  missing, say so and let the person install it — do not install it yourself.
- Use real **number formats** (`'"$"#,##0.00'`, `'0.0%'`, `'yyyy-mm-dd'`) instead
  of pre-formatting numbers into strings — keeps cells numeric.
- Use a consistent professional font; bold the header row and `freeze_panes`.
- Verify every cell a formula references exists before saving.

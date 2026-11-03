//! The contents of an Excel workbook: one table per sheet.
//!
//! A sheet is a sparse grid: the file keeps only the cells that hold
//! something, each with its address (`B2`). They are put back into rows so the
//! model reads a table rather than a list of addresses, and the first row is
//! drawn as the header вЂ” in a spreadsheet that is what the first row is for.
//!
//! Two things are read beyond the text. Shared strings, because a cell holding
//! text keeps only an index into `sharedStrings.xml` and a reader that missed
//! them would show numbers and nothing else. And number formats, for one
//! decision: a date is a number in the file, and `45306` is not what the person
//! means by it. Everything else is shown as Excel stored it.
//!
//! A workbook may be enormous. A sheet is shown to [`MAX_ROWS`] rows and
//! [`MAX_COLS`] columns, and what was left out is said, so the model knows to
//! ask for a part rather than take the first two hundred rows for the whole
//! sheet.

use std::collections::HashMap;

use quick_xml::events::Event;

use super::package::{attr, local, name, next_element, reader, text, Package};
use crate::error::ToolError;

/// Rows of one sheet shown to the model.
const MAX_ROWS: usize = 200;
/// Columns of one sheet shown.
const MAX_COLS: usize = 40;
/// Sheets shown. A workbook with more is a database, and the model is told
/// which sheets there are.
const MAX_SHEETS: usize = 8;

/// One sheet of the workbook and where its XML lives.
struct Sheet {
    name: String,
    part: String,
}

/// How a cell's number should be read, from its style.
#[derive(Debug, Clone, Copy, Default)]
struct Format {
    date: bool,
}

pub fn read(pkg: &mut Package) -> Result<String, ToolError> {
    let shared = shared_strings(pkg)?;
    let formats = formats(pkg)?;
    let sheets = sheets(pkg)?;
    if sheets.is_empty() {
        return Err(ToolError::failed(
            "this workbook has no sheets: xl/workbook.xml names none",
        ));
    }

    let mut out = String::new();
    for (index, sheet) in sheets.iter().take(MAX_SHEETS).enumerate() {
        let Some(xml) = pkg.text(&sheet.part)? else {
            out.push_str(&format!(
                "--- {} (sheet {} of {}) ---\n[the sheet's XML is missing from the file]\n\n",
                sheet.name,
                index + 1,
                sheets.len()
            ));
            continue;
        };
        out.push_str(&format!(
            "--- {} (sheet {} of {}) ---\n",
            sheet.name,
            index + 1,
            sheets.len()
        ));
        out.push_str(&sheet_text(&xml, &shared, &formats)?);
        out.push('\n');
    }
    if sheets.len() > MAX_SHEETS {
        let rest: Vec<&str> = sheets
            .iter()
            .skip(MAX_SHEETS)
            .map(|s| s.name.as_str())
            .collect();
        out.push_str(&format!(
            "[{} more sheets are not shown: {}]\n",
            rest.len(),
            rest.join(", ")
        ));
    }
    Ok(out.trim_end().to_string())
}

/// `xl/sharedStrings.xml`: `<si>` is one string, and rich text splits it into
/// runs (`<r><t>`), which are joined. Phonetic guides (`<rPh>`) are not the
/// text of the cell.
fn shared_strings(pkg: &mut Package) -> Result<Vec<String>, ToolError> {
    let Some(xml) = pkg.text("xl/sharedStrings.xml")? else {
        return Ok(Vec::new());
    };
    let mut reader = reader(&xml);
    let mut strings = Vec::new();
    let mut current: Option<String> = None;
    let mut in_text = false;
    let mut phonetic = 0usize;

    loop {
        match next_element(&mut reader)? {
            Event::Eof => break,
            Event::Start(e) => match name(&e) {
                b"si" => current = Some(String::new()),
                b"rPh" => phonetic += 1,
                b"t" if current.is_some() && phonetic == 0 => in_text = true,
                _ => {}
            },
            Event::Empty(e) if name(&e) == b"t" && phonetic > 0 => {
                let _ = e;
            }
            Event::End(e) => match local(e.name().as_ref()) {
                b"t" => in_text = false,
                b"rPh" => phonetic = phonetic.saturating_sub(1),
                b"si" => strings.push(current.take().unwrap_or_default()),
                _ => {}
            },
            Event::Text(e) if in_text => {
                if let Some(current) = current.as_mut() {
                    current.push_str(&text(&e));
                }
            }
            Event::GeneralRef(e) if in_text => {
                if let (Some(current), Some(ch)) = (current.as_mut(), super::package::reference(&e))
                {
                    current.push_str(&ch);
                }
            }
            _ => {}
        }
    }
    Ok(strings)
}

/// `xl/workbook.xml` and its relationships: the sheets in the order the tabs
/// show them, each with the part that holds its cells.
fn sheets(pkg: &mut Package) -> Result<Vec<Sheet>, ToolError> {
    let Some(xml) = pkg.text("xl/workbook.xml")? else {
        return Err(ToolError::failed(
            "this .xlsx has no xl/workbook.xml: it is not an Excel workbook",
        ));
    };
    let rels = relationships(pkg, "xl/workbook.xml")?;
    let mut reader = reader(&xml);
    let mut sheets = Vec::new();
    loop {
        let event = next_element(&mut reader)?;
        match &event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if name(e) == b"sheet" => {
                let name = attr(e, "name").unwrap_or_else(|| "Sheet".into());
                let id = super::package::rel_id(e).unwrap_or_default();
                if let Some(target) = rels.get(&id) {
                    sheets.push(Sheet {
                        name,
                        part: target.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(sheets)
}

/// The relationships of one part, by id, with targets made absolute inside
/// the package: `worksheets/sheet1.xml` under `xl/` is `xl/worksheets/вЂ¦`.
fn relationships(pkg: &mut Package, part: &str) -> Result<HashMap<String, String>, ToolError> {
    let (dir, file) = match part.rfind('/') {
        Some(at) => (&part[..at + 1], &part[at + 1..]),
        None => ("", part),
    };
    let rels_part = format!("{dir}_rels/{file}.rels");
    let Some(xml) = pkg.text(&rels_part)? else {
        return Ok(HashMap::new());
    };
    let mut reader = reader(&xml);
    let mut out = HashMap::new();
    loop {
        let event = next_element(&mut reader)?;
        match &event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if name(e) == b"Relationship" => {
                let (Some(id), Some(target)) = (attr(e, "Id"), attr(e, "Target")) else {
                    continue;
                };
                let part = if let Some(absolute) = target.strip_prefix('/') {
                    absolute.to_string()
                } else {
                    format!("{dir}{target}")
                };
                out.insert(id, part);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `xl/styles.xml`: for each cell format, whether its number is a date.
fn formats(pkg: &mut Package) -> Result<Vec<Format>, ToolError> {
    let Some(xml) = pkg.text("xl/styles.xml")? else {
        return Ok(Vec::new());
    };
    let mut reader = reader(&xml);
    let mut custom: HashMap<u32, bool> = HashMap::new();
    let mut formats = Vec::new();
    let mut in_cell_xfs = false;

    loop {
        let event = next_element(&mut reader)?;
        match &event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => match (name(e), in_cell_xfs) {
                (b"numFmt", _) => {
                    let (Some(id), Some(code)) = (attr(e, "numFmtId"), attr(e, "formatCode"))
                    else {
                        continue;
                    };
                    if let Ok(id) = id.parse::<u32>() {
                        custom.insert(id, is_date_format(&code));
                    }
                }
                (b"cellXfs", _) => in_cell_xfs = true,
                (b"xf", true) => {
                    let id = attr(e, "numFmtId")
                        .and_then(|id| id.parse::<u32>().ok())
                        .unwrap_or(0);
                    let date = custom.get(&id).copied().unwrap_or_else(|| builtin_date(id));
                    formats.push(Format { date });
                }
                _ => {}
            },
            Event::End(e) if local(e.name().as_ref()) == b"cellXfs" => in_cell_xfs = false,
            _ => {}
        }
    }
    Ok(formats)
}

/// The built-in number formats Excel numbers its dates and times by.
fn builtin_date(id: u32) -> bool {
    matches!(id, 14..=22 | 27..=36 | 45..=47 | 50..=58)
}

/// Whether a custom format code draws a date or a time. Quoted text and
/// bracketed sections (colour, locale) are removed first: `"Q"0` is a number
/// and `[$-409]mmmm d` is a month.
fn is_date_format(code: &str) -> bool {
    let mut plain = String::with_capacity(code.len());
    let mut quoted = false;
    let mut bracketed = 0usize;
    for ch in code.chars() {
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => bracketed += 1,
            ']' if !quoted && bracketed > 0 => bracketed -= 1,
            _ if quoted || bracketed > 0 => {}
            _ => plain.push(ch.to_ascii_lowercase()),
        }
    }
    plain
        .chars()
        .any(|ch| matches!(ch, 'y' | 'm' | 'd' | 'h' | 's'))
}

/// The cells of one sheet as rows of `| a | b |`.
fn sheet_text(xml: &str, shared: &[String], formats: &[Format]) -> Result<String, ToolError> {
    let mut reader = reader(xml);
    let mut out = String::new();
    let mut cells: Vec<String> = Vec::new();
    let mut rows = 0usize;
    let mut columns_cut = false;
    let mut shown_rows = 0usize;

    // The cell being read.
    let mut kind = String::new();
    let mut style = 0u32;
    let mut value: Option<String> = None;
    let mut inline: Option<String> = None;
    let mut column: Option<usize> = None;
    let mut in_value = false;
    let mut in_inline = false;
    let mut cell_phonetic = 0usize;

    loop {
        match next_element(&mut reader)? {
            Event::Eof => break,
            Event::Start(e) => match name(&e) {
                b"row" => {
                    cells.clear();
                    column = None;
                }
                b"c" => {
                    kind = attr(&e, "t").unwrap_or_default();
                    style = attr(&e, "s").and_then(|s| s.parse().ok()).unwrap_or(0);
                    value = None;
                    inline = None;
                    column = attr(&e, "r").and_then(|r| column_of(&r));
                }
                b"v" => in_value = true,
                b"t" if kind == "inlineStr" => in_inline = true,
                b"rPh" => cell_phonetic += 1,
                _ => {}
            },
            Event::End(e) => match local(e.name().as_ref()) {
                b"v" => in_value = false,
                b"t" => in_inline = false,
                b"rPh" => cell_phonetic = cell_phonetic.saturating_sub(1),
                b"c" => {
                    if let Some(at) = column {
                        let value = cell_value(
                            &kind,
                            style,
                            value.as_deref(),
                            inline.as_deref(),
                            shared,
                            formats,
                        );
                        if !value.is_empty() {
                            if at < MAX_COLS {
                                if cells.len() <= at {
                                    cells.resize(at + 1, String::new());
                                }
                                cells[at] = value.replace('|', "\\|").replace('\n', " ");
                            } else {
                                columns_cut = true;
                            }
                        }
                    }
                    column = None;
                }
                b"row" => {
                    rows += 1;
                    if rows > MAX_ROWS {
                        continue;
                    }
                    if cells.iter().all(|c| c.trim().is_empty()) {
                        continue;
                    }
                    out.push_str("| ");
                    out.push_str(&cells.join(" | "));
                    out.push_str(" |\n");
                    shown_rows += 1;
                    if shown_rows == 1 {
                        out.push('|');
                        for _ in 0..cells.len() {
                            out.push_str(" --- |");
                        }
                        out.push('\n');
                    }
                }
                _ => {}
            },
            Event::Text(e) if in_value => {
                value.get_or_insert_with(String::new).push_str(&text(&e));
            }
            Event::Text(e) if in_inline && cell_phonetic == 0 => {
                inline.get_or_insert_with(String::new).push_str(&text(&e));
            }
            Event::GeneralRef(e) if in_value || (in_inline && cell_phonetic == 0) => {
                if let Some(ch) = super::package::reference(&e) {
                    if in_value {
                        value.get_or_insert_with(String::new).push_str(&ch);
                    } else {
                        inline.get_or_insert_with(String::new).push_str(&ch);
                    }
                }
            }
            _ => {}
        }
    }

    if rows == 0 {
        return Ok("[an empty sheet]\n".into());
    }
    if rows > MAX_ROWS {
        out.push_str(&format!(
            "[rows {}-{rows} are not shown; ask for a range of cells to see them]\n",
            MAX_ROWS + 1
        ));
    }
    if columns_cut {
        out.push_str(&format!("[columns past {} are not shown]\n", MAX_COLS));
    }
    Ok(out)
}

/// What the cell shows: text as it is, a number as Excel keeps it, and a date
/// as a date rather than the serial number behind it.
fn cell_value(
    kind: &str,
    style: u32,
    value: Option<&str>,
    inline: Option<&str>,
    shared: &[String],
    formats: &[Format],
) -> String {
    match kind {
        "s" => {
            let index = value.and_then(|v| v.trim().parse::<usize>().ok());
            index
                .and_then(|i| shared.get(i).cloned())
                .unwrap_or_default()
        }
        "inlineStr" => inline.unwrap_or_default().to_string(),
        "str" => value.unwrap_or_default().to_string(),
        "b" => match value.map(str::trim) {
            Some("1") => "TRUE".into(),
            Some("0") => "FALSE".into(),
            other => other.unwrap_or_default().to_string(),
        },
        "e" => value.map(|v| format!("#{v}")).unwrap_or_default(),
        _ => {
            let raw = value.unwrap_or_default().trim();
            let is_date = formats
                .get(style as usize)
                .is_some_and(|format| format.date);
            match (is_date, raw.parse::<f64>()) {
                (true, Ok(serial)) => excel_date(serial),
                _ => raw.to_string(),
            }
        }
    }
}

/// The column of a cell address: `A1` в†’ 0, `AB12` в†’ 27.
fn column_of(address: &str) -> Option<usize> {
    let letters: Vec<u32> = address
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1)
        .collect();
    if letters.is_empty() {
        return None;
    }
    let mut column = 0usize;
    for letter in letters {
        column = column * 26 + letter as usize;
    }
    Some(column - 1)
}

/// A date from Excel's day count, which starts at 1899-12-30 (the leap year
/// Excel believes 1900 had is why the epoch is a day before 1900-01-01).
fn excel_date(serial: f64) -> String {
    let days = serial.trunc() as i64 - 25_569;
    let (year, month, day) = civil_from_days(days);
    let seconds = (serial.fract().abs() * 86_400.0).round() as i64;
    let (hours, minutes) = (seconds / 3_600, seconds % 3_600 / 60);
    if seconds == 0 {
        format!("{year:04}-{month:02}-{day:02}")
    } else {
        format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
    }
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::tests::package;

    fn workbook() -> Vec<u8> {
        let workbook = r#"<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sales" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;
        let shared =
            r#"<sst><si><t>Month</t></si><si><r><t>Rev</t></r><r><t>enue</t></r></si></sst>"#;
        let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Jan</t></is></c><c r="B2"><v>120.5</v></c></row></sheetData></worksheet>"#;
        package(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/sharedStrings.xml", shared),
            ("xl/worksheets/sheet1.xml", sheet),
        ])
    }

    #[test]
    fn shared_strings_and_numbers_are_put_back_into_a_table() {
        let bytes = workbook();
        let mut pkg = Package::open(&bytes, "a.xlsx").unwrap();
        let text = read(&mut pkg).unwrap();
        assert!(text.contains("--- Sales (sheet 1 of 1) ---"), "{text}");
        assert!(text.contains("| Month | Revenue |"), "{text}");
        assert!(text.contains("| --- | --- |"), "{text}");
        assert!(text.contains("| Jan | 120.5 |"), "{text}");
    }

    #[test]
    fn a_date_is_shown_as_a_date() {
        let styles = r#"<styleSheet><numFmts><numFmt numFmtId="164" formatCode="yyyy-mm-dd"/></numFmts><cellXfs><xf numFmtId="0"/><xf numFmtId="164"/></cellXfs></styleSheet>"#;
        let workbook =
            r#"<workbook xmlns:r="r"><sheets><sheet name="S" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;
        let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1" s="1"><v>45306</v></c></row></sheetData></worksheet>"#;
        let bytes = package(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/styles.xml", styles),
            ("xl/worksheets/sheet1.xml", sheet),
        ]);
        let mut pkg = Package::open(&bytes, "a.xlsx").unwrap();
        let text = read(&mut pkg).unwrap();
        // 45306 days after Excel's 1899-12-30 epoch.
        assert!(text.contains("2024-01-15"), "{text}");
    }

    #[test]
    fn a_cell_address_names_its_column() {
        assert_eq!(column_of("A1"), Some(0));
        assert_eq!(column_of("B2"), Some(1));
        assert_eq!(column_of("AA3"), Some(26));
        assert_eq!(column_of("12"), None);
    }

    #[test]
    fn an_empty_book_says_so_rather_than_looking_blank() {
        let bytes = package(&[
            (
                "xl/workbook.xml",
                r#"<workbook><sheets><sheet name="S" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                "<worksheet><sheetData/></worksheet>",
            ),
        ]);
        let mut pkg = Package::open(&bytes, "a.xlsx").unwrap();
        assert!(read(&mut pkg).unwrap().contains("empty sheet"));
    }
}

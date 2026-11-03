//! The text of a Word document: paragraphs, headings and tables.
//!
//! What a person would read on the page, in the order it appears. Word keeps
//! the text of a paragraph split across runs (`<w:r>`), and a run may be
//! formatting alone, so the paragraphs are put back together here rather than
//! shown tag by tag. Styles are read for one thing only: a paragraph styled as
//! a heading becomes a Markdown one, which is what gives the model the outline
//! of a report it was asked to summarize.
//!
//! Deleted text (`<w:del>`) and text moved away from is left out: it is in the
//! file, but it is not what the document says.
//!
//! Pictures are not in `document.xml`; the caller lists them from the package's
//! media folder, so an entirely scanned document still reaches the model as
//! pictures rather than as an empty page.

use quick_xml::events::Event;

use super::package::Package;
use super::package::{attr, name, next_element, reader, text};
use crate::error::ToolError;

pub fn read(pkg: &mut Package) -> Result<String, ToolError> {
    let xml = pkg.text("word/document.xml")?.ok_or_else(|| {
        ToolError::failed("this .docx has no word/document.xml: it is not a Word document")
    })?;
    let mut reader = reader(&xml);

    let mut out = String::new();
    let mut para = String::new();
    let mut style = String::new();
    let mut numbered = false;
    let mut in_text = false;
    let mut skipped = 0usize;
    let mut cell: Option<String> = None;
    let mut row: Vec<String> = Vec::new();
    let mut table_depth = 0usize;
    let mut rows_in_table = 0usize;

    loop {
        match next_element(&mut reader)? {
            Event::Eof => break,
            Event::Start(e) => match name(&e) {
                b"p" => {
                    para.clear();
                    style.clear();
                    numbered = false;
                }
                b"pStyle" => style = attr(&e, "val").unwrap_or_default(),
                b"numPr" if cell.is_none() => numbered = true,
                b"tbl" => {
                    flush_para(&mut out, &mut para, &mut style, &mut numbered, &mut cell);
                    table_depth += 1;
                    rows_in_table = 0;
                }
                b"tr" if table_depth > 0 => {
                    row.clear();
                }
                b"tc" if table_depth > 0 => cell = Some(String::new()),
                b"t" => in_text = true,
                b"tab" => push_str(&mut cell, &mut para, "\t"),
                b"br" | b"cr" => push_str(&mut cell, &mut para, "\n"),
                b"del" | b"moveFrom" => skipped += 1,
                _ => {}
            },
            Event::Empty(e) => match name(&e) {
                b"pStyle" => style = attr(&e, "val").unwrap_or_default(),
                b"numPr" if cell.is_none() => numbered = true,
                b"tab" => push_str(&mut cell, &mut para, "\t"),
                b"br" | b"cr" => push_str(&mut cell, &mut para, "\n"),
                _ => {}
            },
            Event::End(e) => match super::package::local(e.name().as_ref()) {
                b"t" => in_text = false,
                b"p" => flush_para(&mut out, &mut para, &mut style, &mut numbered, &mut cell),
                b"tc" if table_depth > 0 => {
                    if let Some(cell) = cell.take() {
                        row.push(cell.trim().to_string());
                    }
                }
                b"tr" if table_depth > 0 => {
                    if !row.is_empty() {
                        out.push_str("| ");
                        out.push_str(&row.join(" | "));
                        out.push_str(" |\n");
                        rows_in_table += 1;
                        if rows_in_table == 1 {
                            // The first row of a Word table is the header in
                            // nearly every report, and Markdown needs the rule
                            // for the table to render as one.
                            out.push('|');
                            for _ in 0..row.len() {
                                out.push_str(" --- |");
                            }
                            out.push('\n');
                        }
                    }
                }
                b"tbl" if table_depth > 0 => {
                    table_depth -= 1;
                    if table_depth == 0 {
                        out.push('\n');
                    }
                }
                b"del" | b"moveFrom" => skipped = skipped.saturating_sub(1),
                _ => {}
            },
            Event::Text(e) if in_text && skipped == 0 => {
                let text = text(&e);
                push_str(&mut cell, &mut para, &text);
            }
            Event::GeneralRef(e) if in_text && skipped == 0 => {
                if let Some(text) = super::package::reference(&e) {
                    push_str(&mut cell, &mut para, &text);
                }
            }
            _ => {}
        }
    }

    Ok(tidy(out))
}

/// Ends the paragraph being read: into the table cell it is in, or as a line
/// of the document.
fn flush_para(
    out: &mut String,
    para: &mut String,
    style: &mut String,
    numbered: &mut bool,
    cell: &mut Option<String>,
) {
    let text = para.trim();
    if let Some(cell) = cell.as_mut() {
        if !text.is_empty() {
            if !cell.is_empty() {
                cell.push(' ');
            }
            cell.push_str(text);
        }
    } else if !text.is_empty() {
        if let Some(level) = heading_level(style) {
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
        } else if *numbered {
            out.push_str("- ");
        }
        out.push_str(text);
        out.push_str("\n\n");
    }
    para.clear();
    style.clear();
    *numbered = false;
}

/// `Title` and `Heading 1`…`Heading 9` as Word keeps their style ids.
fn heading_level(style: &str) -> Option<usize> {
    let style = style.to_ascii_lowercase();
    if style == "title" {
        return Some(1);
    }
    if style == "subtitle" {
        return Some(2);
    }
    let rest = style.strip_prefix("heading")?.trim();
    rest.parse::<usize>().ok().map(|n| n.clamp(1, 6))
}

fn push_str(cell: &mut Option<String>, para: &mut String, text: &str) {
    match cell {
        Some(cell) => cell.push_str(text),
        None => para.push_str(text),
    }
}

/// No more than one blank line between two blocks, and none at the ends.
fn tidy(text: String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            newlines += 1;
            if newlines > 1 {
                continue;
            }
            out.push('\n');
        } else {
            newlines = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
        )
    }

    fn read_body(body: &str) -> String {
        let bytes = crate::office::tests::package(&[("word/document.xml", &document(body))]);
        let mut pkg = Package::open(&bytes, "a.docx").unwrap();
        read(&mut pkg).unwrap()
    }

    #[test]
    fn paragraphs_of_runs_come_back_whole() {
        let text = read_body(
            r#"<w:p><w:r><w:t>Revenue</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve"> grew 15%</w:t></w:r></w:p><w:p><w:r><w:t>Second.</w:t></w:r></w:p>"#,
        );
        assert_eq!(text, "Revenue grew 15%\n\nSecond.");
    }

    #[test]
    fn a_heading_becomes_a_markdown_heading() {
        let text = read_body(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Summary</w:t></w:r></w:p><w:p><w:r><w:t>Body.</w:t></w:r></w:p>"#,
        );
        assert_eq!(text, "# Summary\n\nBody.");
    }

    #[test]
    fn a_table_becomes_markdown_rows() {
        let text = read_body(
            r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Qty</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Widget</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>12</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        );
        assert!(text.contains("| Item | Qty |"), "{text}");
        assert!(text.contains("| --- | --- |"), "{text}");
        assert!(text.contains("| Widget | 12 |"), "{text}");
    }

    #[test]
    fn deleted_text_is_not_part_of_the_document() {
        let text = read_body(
            r#"<w:p><w:r><w:t>Kept</w:t></w:r><w:del><w:r><w:delText>Gone</w:delText></w:r></w:del></w:p>"#,
        );
        assert_eq!(text, "Kept");
    }

    #[test]
    fn character_references_are_read_as_characters() {
        let text = read_body(r#"<w:p><w:r><w:t>A &amp; B &#8212; done</w:t></w:r></w:p>"#);
        assert_eq!(text, "A & B — done");
    }

    #[test]
    fn a_broken_part_is_an_error_not_a_panic() {
        let bytes = crate::office::tests::package(&[("word/document.xml", "<w:document")]);
        let mut pkg = Package::open(&bytes, "a.docx").unwrap();
        let err = read(&mut pkg).unwrap_err();
        assert!(err.for_model().contains("does not parse"), "{err:?}");
    }
}

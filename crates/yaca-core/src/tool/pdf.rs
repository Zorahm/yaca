//! The text of a PDF, by page, for `Read`.
//!
//! Text only: a page goes to the model as the characters its content stream
//! draws, in the order it draws them. That works with every provider and every
//! local model, which a document block would not. What it cannot do is read a
//! scan, and a page with no text layer is said to be one rather than shown as
//! an empty page the model would take for blank paper.
//!
//! A long document is not handed over whole: a hundred-page manual is most of
//! a context window, and it stays there for every request after. Up to
//! [`WHOLE_PAGES`] pages come back at once; past that the model is told how
//! many there are and asked for a range.

use std::ops::RangeInclusive;
use std::panic::{catch_unwind, AssertUnwindSafe};

use pdf_extract::{output_doc_page, Document, PlainTextOutput};

use crate::error::ToolError;

/// A document this short is read whole when no range is given.
pub(crate) const WHOLE_PAGES: u32 = 10;
/// The most pages one call reads.
pub(crate) const MAX_PAGES: u32 = 20;

/// Fewer visible characters than this on a page is a page without a text
/// layer: a scan, a figure, or a page that is only a picture of text.
const SCANNED_BELOW: usize = 16;

/// The pages of a document that were read, ready for the model.
#[derive(Debug)]
pub(crate) struct PdfText {
    pub text: String,
    /// How many pages the document has.
    pub total: u32,
    /// How many of the requested pages were read.
    pub shown: u32,
    /// The pages that came back with (almost) no text.
    pub empty: Vec<u32>,
}

/// Parses `pages` as the model writes it: `"5"`, `"3-7"`, `"3-"`.
pub(crate) fn parse_pages(raw: &str) -> Result<RangeInclusive<u32>, ToolError> {
    let bad = || ToolError::failed(format!("pages must look like \"3\" or \"3-7\", got \"{raw}\""));
    let raw = raw.trim();
    let (from, to) = match raw.split_once('-') {
        Some((from, to)) => (from.trim(), to.trim()),
        None => (raw, raw),
    };
    let from: u32 = from.parse().map_err(|_| bad())?;
    let to: u32 = if to.is_empty() { u32::MAX } else { to.parse().map_err(|_| bad())? };
    if from == 0 || to < from {
        return Err(bad());
    }
    Ok(from..=to)
}

/// Reads `bytes` as a PDF. `pages` is the range asked for, or `None` for the
/// whole of a short document.
///
/// Blocking: the caller runs it off the executor.
pub(crate) fn read(
    bytes: &[u8],
    shown: &str,
    pages: Option<RangeInclusive<u32>>,
) -> Result<PdfText, ToolError> {
    let mut doc = Document::load_mem(bytes)
        .map_err(|e| ToolError::failed(format!("{shown} is not a readable PDF: {e}")))?;
    // Most "encrypted" PDFs are only locked against editing and open with
    // the empty password; one that needs a real password cannot be read.
    if doc.is_encrypted() && doc.decrypt("").is_err() {
        return Err(ToolError::failed(format!(
            "{shown} is password-protected; its text cannot be read"
        )));
    }

    let numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    let total = numbers.len() as u32;
    if total == 0 {
        return Err(ToolError::failed(format!("{shown} has no pages")));
    }

    let range = match pages {
        Some(range) => {
            if *range.start() > total {
                return Err(ToolError::failed(format!(
                    "{shown} has {total} pages, page {} was requested",
                    range.start()
                )));
            }
            let end = (*range.end()).min(total);
            if end - range.start() + 1 > MAX_PAGES {
                return Err(ToolError::failed_with(
                    format!("at most {MAX_PAGES} pages per call"),
                    format!(
                        "One call reads at most {MAX_PAGES} pages. Ask for a narrower range, \
                         e.g. \"{}-{}\".",
                        range.start(),
                        range.start() + MAX_PAGES - 1
                    ),
                ));
            }
            *range.start()..=end
        }
        None if total <= WHOLE_PAGES => 1..=total,
        None => {
            return Err(ToolError::failed_with(
                format!("{shown} has {total} pages; give a range"),
                format!(
                    "{shown} has {total} pages, too many to read at once. Call Read again \
                     with `pages`, e.g. \"1-{WHOLE_PAGES}\" — at most {MAX_PAGES} pages per call."
                ),
            ))
        }
    };

    let mut text = String::new();
    let mut empty = Vec::new();
    for number in range.clone() {
        // `get_pages` numbers from one in document order; the key is what
        // `output_doc_page` looks the page up by.
        let Some(&key) = numbers.get(number as usize - 1) else { break };
        let page = page_text(&doc, key);
        text.push_str(&format!("--- page {number} of {total} ---\n"));
        match page {
            Some(page) if visible(&page) >= SCANNED_BELOW => {
                text.push_str(page.trim_matches('\n'));
                text.push('\n');
            }
            Some(_) => {
                empty.push(number);
                text.push_str("[no text layer on this page: a scan or an image]\n");
            }
            None => {
                empty.push(number);
                text.push_str("[the text of this page could not be extracted]\n");
            }
        }
        text.push('\n');
    }

    Ok(PdfText {
        text,
        total,
        shown: range.end() - range.start() + 1,
        empty,
    })
}

/// One page's text, or `None` when the extractor failed on it.
///
/// The extractor panics on some fonts it does not understand. One bad page
/// must not cost the others, so each page is caught on its own.
fn page_text(doc: &Document, key: u32) -> Option<String> {
    catch_unwind(AssertUnwindSafe(|| {
        let mut out = String::new();
        {
            let mut sink = PlainTextOutput::new(&mut out);
            output_doc_page(doc, &mut sink, key).ok()?;
        }
        Some(out)
    }))
    .ok()
    .flatten()
}

fn visible(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal PDF with one line of text on each page, built by hand so the
    /// tests need no fixture file. Offsets in the xref table are computed, not
    /// typed: a wrong one and the parser falls back to scanning, which hides bugs.
    pub(crate) fn pdf(pages: &[&str]) -> Vec<u8> {
        let n = pages.len();
        // Objects: 1 catalog, 2 pages, 3 font, then a page and its content per page.
        let mut objects: Vec<String> = Vec::new();
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + i * 2)).collect();
        objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
        objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
        for (i, line) in pages.iter().enumerate() {
            let content = format!("BT /F1 12 Tf 72 720 Td ({line}) Tj ET");
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                5 + i * 2
            ));
            objects.push(format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ));
        }

        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for offset in offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn pages_are_parsed_the_way_a_model_writes_them() {
        assert_eq!(parse_pages("3").unwrap(), 3..=3);
        assert_eq!(parse_pages(" 3 - 7 ").unwrap(), 3..=7);
        assert_eq!(parse_pages("4-").unwrap(), 4..=u32::MAX);
        assert!(parse_pages("0").is_err());
        assert!(parse_pages("7-3").is_err());
        assert!(parse_pages("first").is_err());
    }

    #[test]
    fn a_short_document_is_read_whole_page_by_page() {
        let bytes = pdf(&["Hello from page one", "And this is page two"]);
        let read = read(&bytes, "a.pdf", None).unwrap();
        assert_eq!(read.total, 2);
        assert_eq!(read.shown, 2);
        assert!(read.text.contains("--- page 1 of 2 ---"), "{}", read.text);
        assert!(read.text.contains("Hello from page one"), "{}", read.text);
        assert!(read.text.contains("page two"), "{}", read.text);
        assert!(read.empty.is_empty());
    }

    #[test]
    fn a_range_reads_only_its_pages() {
        let bytes = pdf(&["Alpha page text here", "Bravo page text here", "Charlie page text here"]);
        let read = read(&bytes, "a.pdf", Some(2..=2)).unwrap();
        assert!(read.text.contains("Bravo"));
        assert!(!read.text.contains("Alpha") && !read.text.contains("Charlie"));
        assert_eq!(read.shown, 1);
    }

    #[test]
    fn a_long_document_asks_for_a_range() {
        let texts: Vec<String> = (1..=12).map(|i| format!("Text on page number {i}")).collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let err = read(&pdf(&refs), "big.pdf", None).unwrap_err();
        assert!(err.for_model().contains("pages"), "{}", err.for_model());

        let open = read(&pdf(&refs), "big.pdf", Some(11..=u32::MAX)).unwrap();
        assert_eq!(open.shown, 2, "an open range stops at the last page");
    }

    #[test]
    fn a_range_past_the_end_says_how_many_pages_there_are() {
        let err = read(&pdf(&["Only one page here"]), "a.pdf", Some(5..=6)).unwrap_err();
        assert!(err.to_string().contains("1 pages"), "{err}");
    }

    #[test]
    fn a_page_without_text_is_called_a_scan() {
        let read = read(&pdf(&["Real words on this page", ""]), "a.pdf", None).unwrap();
        assert_eq!(read.empty, vec![2]);
        assert!(read.text.contains("no text layer"));
    }

    #[test]
    fn garbage_is_not_a_pdf() {
        let err = read(b"not a pdf at all", "a.pdf", None).unwrap_err();
        assert!(err.to_string().contains("not a readable PDF"), "{err}");
    }
}

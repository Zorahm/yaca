//! The container every Office format shares: a ZIP of XML parts.
//!
//! A `.docx`, `.xlsx` and `.pptx` are the same idea under three schemas. The
//! part names differ (`word/document.xml`, `xl/workbook.xml`,
//! `ppt/presentation.xml`), and so does what the XML means; opening the
//! archive, reading a part and reading one element is the same work for all
//! three, so it lives here once.
//!
//! Nothing here trusts the archive. A file that is not a ZIP, a part that is
//! missing and XML that does not parse all come back as an error the model can
//! act on, not as a panic: an office file is often the very thing being
//! repaired, and a broken one must not take the session with it.

use std::io::{Cursor, Read};
use std::path::Path;

use quick_xml::events::{BytesRef, BytesStart, BytesText, Event};
use quick_xml::Reader;
use zip::ZipArchive;

use crate::error::ToolError;

/// The formats `Read` opens itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Docx,
    Xlsx,
    Pptx,
}

impl Kind {
    /// The kind by the extension, or `None` for anything else. The
    /// macro-enabled and template variants are the same package under another
    /// extension, and a document opened here is read the same way.
    pub fn of(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Some(Kind::Docx),
            "xlsx" | "xlsm" | "xltx" | "xltm" => Some(Kind::Xlsx),
            "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => Some(Kind::Pptx),
            _ => None,
        }
    }

    /// What the model is told it is reading.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Docx => "Word document",
            Kind::Xlsx => "Excel workbook",
            Kind::Pptx => "PowerPoint presentation",
        }
    }

    /// Where the embedded pictures live inside the package.
    pub fn media_dir(self) -> &'static str {
        match self {
            Kind::Docx => "word/media/",
            Kind::Xlsx => "xl/media/",
            Kind::Pptx => "ppt/media/",
        }
    }
}

/// An opened package.
pub struct Package<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
}

impl<'a> Package<'a> {
    pub fn open(bytes: &'a [u8], shown: &str) -> Result<Package<'a>, ToolError> {
        let zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| {
            ToolError::failed_with(
                format!("{shown} is not a readable Office file"),
                format!(
                    "A .docx, .xlsx or .pptx is a ZIP archive, and this one could not be \
                     opened: {e}. If it came from an older Word, Excel or PowerPoint, save \
                     it again in the current format first."
                ),
            )
        })?;
        Ok(Package { zip })
    }

    /// The bytes of one part, or `None` when the package has no such part.
    pub fn bytes(&mut self, name: &str) -> Result<Option<Vec<u8>>, ToolError> {
        let mut file = match self.zip.by_name(name) {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(e) => return Err(ToolError::failed(format!("{name} could not be read: {e}"))),
        };
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|e| ToolError::failed(format!("{name} could not be read: {e}")))?;
        Ok(Some(bytes))
    }

    /// The text of one part, or `None` when the package has no such part.
    pub fn text(&mut self, name: &str) -> Result<Option<String>, ToolError> {
        Ok(self.bytes(name)?.map(|bytes| decode(&bytes)))
    }

    /// Every part name in the package.
    pub fn names(&self) -> Vec<String> {
        self.zip.file_names().map(str::to_string).collect()
    }
}

/// XML is UTF-8 in every package Word, Excel and PowerPoint write, but the
/// files may carry either byte order mark, and UTF-16 is legal XML.
fn decode(bytes: &[u8]) -> String {
    let bytes = if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        rest
    } else {
        bytes
    };
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// A reader for one part: errors inside the XML are the parser's to report,
/// and the caller stops at the first one.
pub fn reader(xml: &str) -> Reader<&[u8]> {
    let mut reader = Reader::from_str(xml);
    // Whitespace between elements is formatting, not content; inside a run of
    // text it is the text, and the formats mark that with `xml:space`.
    reader.config_mut().trim_text(false);
    reader
}

/// The element name without its namespace prefix: `w:p` → `p`.
pub fn local(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|b| *b == b':') {
        Some(at) => &name[at + 1..],
        None => name,
    }
}

/// The element's name without a prefix.
pub fn name<'a, 'b>(event: &'b BytesStart<'a>) -> &'b [u8] {
    // `into_inner` rather than `as_ref`: the latter borrows the temporary
    // `QName`, and the slice is the event's own.
    local(event.name().into_inner())
}

/// One attribute by its local name: `r:id` is found by `id`.
pub fn attr(event: &BytesStart, key: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|a| local(a.key.as_ref()) == key.as_bytes())
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// The relationship id of an element: `r:id` in every schema here.
///
/// Not [`attr`] with `id`: `sldId` carries both an `id` of its own and the
/// `r:id` that names its part, and the first is not a relationship.
pub fn rel_id(event: &BytesStart) -> Option<String> {
    let mut fallback = None;
    for a in event.attributes().flatten() {
        let raw = a.key.as_ref();
        if raw == b"r:id" {
            return Some(String::from_utf8_lossy(&a.value).into_owned());
        }
        if local(raw) == b"id" && fallback.is_none() {
            fallback = Some(String::from_utf8_lossy(&a.value).into_owned());
        }
    }
    fallback
}

/// The text of a `<t>`-like element. The parser hands character references
/// over as separate events, so what arrives here is literal text; a literal
/// `&` that reached the parser unescaped is left as it is.
pub fn text(event: &BytesText) -> String {
    match event.decode() {
        Ok(decoded) => match quick_xml::escape::unescape(&decoded) {
            Ok(text) => text.into_owned(),
            Err(_) => decoded.into_owned(),
        },
        Err(_) => String::new(),
    }
}

/// The name of a general reference: `&amp;` → `&`, `&#65;` → `A`.
pub fn reference(event: &BytesRef) -> Option<String> {
    let raw = event.decode().ok()?.into_owned();
    Some(match raw.as_str() {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        other => {
            let code = other
                .strip_prefix("#x")
                .or_else(|| other.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    other
                        .strip_prefix('#')
                        .and_then(|dec| dec.parse::<u32>().ok())
                })?;
            char::from_u32(code)?.to_string()
        }
    })
}

/// The next element event, or `Eof`. Comments, declarations and text are
/// skipped: every format here is read for elements it names.
pub fn next_element<'a>(reader: &mut Reader<&'a [u8]>) -> Result<Event<'a>, ToolError> {
    loop {
        match reader
            .read_event()
            .map_err(|e| ToolError::failed(format!("the document's XML does not parse: {e}")))?
        {
            Event::Eof => return Ok(Event::Eof),
            other @ (Event::Start(_)
            | Event::End(_)
            | Event::Empty(_)
            | Event::Text(_)
            | Event::GeneralRef(_)) => return Ok(other),
            _ => continue,
        }
    }
}

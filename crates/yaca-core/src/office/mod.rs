//! Word, Excel and PowerPoint files, read without an office program.
//!
//! The three formats are one idea: a ZIP of XML parts, one schema each. This
//! module opens the archive, hands the parts to the reader for the format, and
//! collects the pictures the file embeds. `Read` calls it; nothing here knows
//! about tools or the feed.
//!
//! The pictures matter as much as the text. A deck of screenshots or a
//! scanned report has no text at all, and a reader that stopped at the XML
//! would hand the model a blank page — the one answer that looks like an empty
//! document rather than a file it could not read. So the media are returned
//! with the text, and `Read` shows them to a model that can see.
//!
//! What is *not* here is writing. A document is produced by a script — the
//! office skills say how — because a text write would leave a file no editor
//! opens. This module only reads.

pub mod docx;
mod package;
pub mod pptx;
pub mod xlsx;

pub use package::Kind;

use crate::error::ToolError;

/// How much text one file hands the model. The whole of a report fits; a
/// spreadsheet with a hundred thousand rows does not, and the readers cut
/// their own output before this as well.
pub const MAX_TEXT_BYTES: usize = 90_000;

/// The most pictures `Read` shows from one file. A deck of thirty slides has
/// more pictures than a model needs to understand it, and every one of them
/// stays in the window for the rest of the conversation.
pub const MAX_PICTURES: usize = 8;

/// A picture found inside the package.
#[derive(Debug, Clone)]
pub struct Media {
    /// Its name inside the package without the folder: `image1.png`.
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What one file gives the model.
#[derive(Debug)]
pub struct Extracted {
    pub text: String,
    /// The pictures the file embeds, in package order.
    pub media: Vec<Media>,
    /// The text was cut at [`MAX_TEXT_BYTES`].
    pub truncated: bool,
}

/// Reads `bytes` as an office file of `kind`.
pub fn read(kind: Kind, bytes: &[u8], shown: &str) -> Result<Extracted, ToolError> {
    let mut package = package::Package::open(bytes, shown)?;
    let text = match kind {
        Kind::Docx => docx::read(&mut package)?,
        Kind::Xlsx => xlsx::read(&mut package)?,
        Kind::Pptx => pptx::read(&mut package)?,
    };
    let media = media_of(&mut package, kind)?;
    let (text, truncated) = cap(text);
    Ok(Extracted {
        text,
        media,
        truncated,
    })
}

/// Just the pictures, for the tool that writes them out.
pub fn media(kind: Kind, bytes: &[u8], shown: &str) -> Result<Vec<Media>, ToolError> {
    let mut package = package::Package::open(bytes, shown)?;
    media_of(&mut package, kind)
}

fn media_of(package: &mut package::Package, kind: Kind) -> Result<Vec<Media>, ToolError> {
    let dir = kind.media_dir();
    let mut names: Vec<String> = package
        .names()
        .into_iter()
        .filter(|name| name.starts_with(dir) && !name.ends_with('/') && !name.contains(['\\', ':']))
        .collect();
    names.sort();
    let mut media = Vec::new();
    for name in names {
        let Some(bytes) = package.bytes(&name)? else {
            continue;
        };
        let short = name.strip_prefix(dir).unwrap_or(&name).replace('/', "-");
        if short.is_empty() || short == "." || short == ".." {
            continue;
        }
        media.push(Media { name: short, bytes });
    }
    Ok(media)
}

/// Cuts the text at a character boundary and says it was cut.
fn cap(text: String) -> (String, bool) {
    if text.len() <= MAX_TEXT_BYTES {
        return (text, false);
    }
    let mut cut = MAX_TEXT_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut text = text[..cut].trim_end().to_string();
    text.push_str(&format!(
        "\n\n[the document is longer than {MAX_TEXT_BYTES} bytes and is cut here]"
    ));
    (text, true)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    /// A package with the given parts, uncompressed.
    pub fn package(files: &[(&str, &str)]) -> Vec<u8> {
        let parts: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(name, body)| (*name, body.as_bytes()))
            .collect();
        package_binary(&parts)
    }

    /// The same with parts that are not text: the pictures in a document.
    pub fn package_binary(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, body) in files {
                writer.start_file(*name, options).unwrap();
                writer.write_all(body).unwrap();
            }
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn a_file_that_is_not_a_zip_says_which_formats_are_read() {
        let err = read(Kind::Docx, b"not a zip at all", "notes.docx").unwrap_err();
        assert!(err.for_model().contains("ZIP"), "{err:?}");
    }

    #[test]
    fn a_binary_media_file_is_listed_by_its_name_inside_the_package() {
        let bytes = package(&[
            (
                "word/document.xml",
                r#"<w:document xmlns:w="w"><w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#,
            ),
            ("word/media/image1.png", "PNG"),
        ]);
        let read = read(Kind::Docx, &bytes, "notes.docx").unwrap();
        assert_eq!(read.text, "Hi");
        assert_eq!(read.media.len(), 1);
        assert_eq!(read.media[0].name, "image1.png");
        assert_eq!(read.media[0].bytes, b"PNG");
    }

    #[test]
    fn long_text_is_cut_on_a_character_and_says_so() {
        let body = "é".repeat(MAX_TEXT_BYTES);
        let screen = package(&[(
            "word/document.xml",
            &format!(
                "<w:document><w:body><w:p><w:r><w:t>{body}</w:t></w:r></w:p></w:body></w:document>"
            ),
        )]);
        let read = read(Kind::Docx, &screen, "long.docx").unwrap();
        assert!(read.truncated);
        assert!(read.text.len() < MAX_TEXT_BYTES + 200);
        assert!(read.text.ends_with("is cut here]"));
    }
}

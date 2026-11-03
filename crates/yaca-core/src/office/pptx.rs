//! The text of a PowerPoint deck: slide by slide, with the speaker notes.
//!
//! A slide's text is spread over shapes, and a shape over paragraphs, and a
//! paragraph over runs: the same shape as a Word document, under the drawing
//! schema. What is put back here is the reading order the file keeps, not the
//! order on the page вЂ” PowerPoint stores shapes in the order they were added,
//! which is usually the order a person wrote them.
//!
//! Speaker notes are read too. In a deck made to be presented, the slide holds
//! the headline and the notes hold what is actually being said, so a summary
//! built from the slides alone would miss half of it.
//!
//! Charts and SmartArt keep their numbers and labels in parts of their own; the
//! text there is not read, and the deck is described by what the slides say
//! about them.

use std::collections::HashMap;

use quick_xml::events::Event;

use super::package::{attr, local, name, next_element, reader, text, Package};
use crate::error::ToolError;

pub fn read(pkg: &mut Package) -> Result<String, ToolError> {
    let slides = slides(pkg)?;
    if slides.is_empty() {
        return Err(ToolError::failed(
            "this .pptx has no slides: ppt/presentation.xml names none",
        ));
    }

    let mut out = String::new();
    for (index, part) in slides.iter().enumerate() {
        let Some(xml) = pkg.text(part)? else {
            out.push_str(&format!(
                "--- slide {} ---\n[the slide's XML is missing]\n\n",
                index + 1
            ));
            continue;
        };
        out.push_str(&format!("--- slide {} ---\n", index + 1));
        let body = slide_text(&xml)?;
        if body.trim().is_empty() {
            out.push_str("[no text on this slide: it is a picture or a chart]\n");
        } else {
            out.push_str(body.trim_end());
            out.push('\n');
        }
        // The notes of this slide, when it has them.
        let rels = relationships(pkg, part)?;
        for target in rels.values().filter(|target| target.contains("notesSlide")) {
            if let Some(notes) = pkg.text(target)? {
                let notes = slide_text(&notes)?;
                let notes = notes.trim();
                if !notes.is_empty() {
                    out.push_str("Notes: ");
                    out.push_str(notes);
                    out.push('\n');
                }
            }
        }
        out.push('\n');
    }
    Ok(out.trim_end().to_string())
}

/// The slides in the order the deck shows them.
fn slides(pkg: &mut Package) -> Result<Vec<String>, ToolError> {
    let Some(xml) = pkg.text("ppt/presentation.xml")? else {
        return Err(ToolError::failed(
            "this .pptx has no ppt/presentation.xml: it is not a PowerPoint presentation",
        ));
    };
    let rels = relationships(pkg, "ppt/presentation.xml")?;
    let mut reader = reader(&xml);
    let mut slides = Vec::new();
    loop {
        let event = next_element(&mut reader)?;
        match &event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if name(e) == b"sldId" => {
                if let Some(part) = super::package::rel_id(e).and_then(|id| rels.get(&id).cloned())
                {
                    slides.push(part);
                }
            }
            _ => {}
        }
    }
    Ok(slides)
}

/// The relationships of one part, by id, with targets resolved to a path
/// inside the package (`../notesSlides/notesSlide1.xml` included).
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
                out.insert(id, join(dir, &target));
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `dir` plus a relationship target, with `.` and `..` resolved.
fn join(dir: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut parts: Vec<&str> = dir
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// The text of one slide (or of its notes): paragraphs on their own lines,
/// shapes apart, cells of a table in a row.
fn slide_text(xml: &str) -> Result<String, ToolError> {
    let mut reader = reader(xml);
    let mut out = String::new();
    let mut in_text = false;
    let mut field = 0usize;
    let mut cell: Option<String> = None;
    let mut row: Vec<String> = Vec::new();

    loop {
        match next_element(&mut reader)? {
            Event::Eof => break,
            Event::Start(e) => match name(&e) {
                b"t" if field == 0 => in_text = true,
                b"fld" => field += 1,
                b"tc" => cell = Some(String::new()),
                _ => {}
            },
            Event::Empty(e) if name(&e) == b"br" => out.push('\n'),
            Event::End(e) => match local(e.name().as_ref()) {
                b"t" => in_text = false,
                b"fld" => field = field.saturating_sub(1),
                b"p" => {
                    if cell.is_some() {
                        out.push(' ');
                    } else {
                        out.push('\n');
                    }
                }
                b"sp" | b"graphicFrame" => {
                    if cell.is_none() {
                        out.push('\n');
                    }
                }
                b"tc" => {
                    if let Some(cell) = cell.take() {
                        row.push(cell.trim().replace('|', "\\|"));
                    }
                }
                b"tr" if !row.is_empty() => {
                    out.push_str(&format!("| {} |\n", row.join(" | ")));
                    row.clear();
                }
                _ => {}
            },
            Event::Text(e) if in_text => match cell.as_mut() {
                Some(cell) => cell.push_str(&text(&e)),
                None => out.push_str(&text(&e)),
            },
            Event::GeneralRef(e) if in_text => {
                if let Some(ch) = super::package::reference(&e) {
                    match cell.as_mut() {
                        Some(cell) => cell.push_str(&ch),
                        None => out.push_str(&ch),
                    }
                }
            }
            _ => {}
        }
    }

    // Runs that end and start a paragraph leave empty lines; a slide reads
    // better with one line per paragraph.
    let mut tidy = String::with_capacity(out.len());
    let mut blank = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        tidy.push_str(line.trim_end());
        tidy.push('\n');
    }
    Ok(tidy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::tests::package;

    fn deck() -> Vec<u8> {
        let presentation = r#"<p:presentation xmlns:p="p" xmlns:r="r"><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#;
        let rels = r#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/></Relationships>"#;
        let slide = r#"<p:sld><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Quarterly Review</a:t></a:r></a:p><a:p><a:r><a:t>Revenue </a:t></a:r><a:r><a:t>up 15%</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#;
        package(&[
            ("ppt/presentation.xml", presentation),
            ("ppt/_rels/presentation.xml.rels", rels),
            ("ppt/slides/slide1.xml", slide),
        ])
    }

    #[test]
    fn slides_keep_their_order_and_their_paragraphs() {
        let bytes = deck();
        let mut pkg = Package::open(&bytes, "a.pptx").unwrap();
        let text = read(&mut pkg).unwrap();
        assert!(text.starts_with("--- slide 1 ---"), "{text}");
        assert!(text.contains("Quarterly Review"), "{text}");
        assert!(text.contains("Revenue up 15%"), "{text}");
    }

    #[test]
    fn a_relative_relationship_is_resolved_inside_the_package() {
        assert_eq!(
            join("ppt/slides/", "../notesSlides/notesSlide1.xml"),
            "ppt/notesSlides/notesSlide1.xml"
        );
        assert_eq!(
            join("xl/", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(join("xl/", "/xl/styles.xml"), "xl/styles.xml");
    }

    #[test]
    fn a_picture_only_slide_says_it_has_no_text() {
        let presentation = r#"<p:presentation><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#;
        let rels = r#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/></Relationships>"#;
        let slide = r#"<p:sld><p:cSld><p:spTree><p:pic><p:nvPicPr><p:cNvPr id="2" name="Picture 1"/></p:nvPicPr></p:pic></p:spTree></p:cSld></p:sld>"#;
        let bytes = package(&[
            ("ppt/presentation.xml", presentation),
            ("ppt/_rels/presentation.xml.rels", rels),
            ("ppt/slides/slide1.xml", slide),
        ]);
        let mut pkg = Package::open(&bytes, "a.pptx").unwrap();
        assert!(read(&mut pkg).unwrap().contains("picture or a chart"));
    }
}

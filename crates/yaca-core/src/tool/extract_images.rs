//! `ExtractImages`: the pictures inside a document, written out as files.
//!
//! `Read` shows the pictures to a model that can see, but a picture seen is
//! not a picture that can be used: to put it into another document, send it,
//! or edit it, the file has to exist on disk. This is that step, and only that
//! step — the text of the document comes from `Read`.
//!
//! It writes bytes, so it is a writing tool: offered in Agent only, and asked
//! about in Manual like any other write. What it writes comes out of the file
//! the human pointed at, and a human who wants the pictures of a document has
//! already decided they may exist as files.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::office::{self, Kind};
use crate::permission::{PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

pub struct ExtractImages;

#[async_trait]
impl Tool for ExtractImages {
    fn name(&self) -> &'static str {
        "ExtractImages"
    }

    fn description(&self) -> &'static str {
        "Write the pictures embedded in a .docx, .xlsx or .pptx out as image files, so they \
         can be inserted into another document, looked at with ReadImage, or sent. Read the \
         document with Read first: it gives the text and shows the pictures. The files go \
         to `dir`, by default a folder named after the document (`report media/`)."
    }

    fn properties(&self) -> Value {
        json!({
            "path": {
                "type": "string",
                "description": "The .docx, .xlsx or .pptx file, relative to the project root.",
            },
            "dir": {
                "type": "string",
                "description": "Folder to write the pictures into, relative to the project \
                                root. Defaults to `<file name without extension> media` next \
                                to the file.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        // The interface knows the files that came out only after the call; how
        // many pictures a document holds is the model's to say.
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        let dir = match input["dir"].as_str() {
            Some(dir) => ws.resolve_new(dir).ok()?,
            None => default_dir(&abs),
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| raw.to_string());
        Some(
            PermissionNeed::new(PermissionKind::EditFile, self.name(), name)
                .detail(ws.display(&dir)),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let abs = ctx.ws.resolve_existing(raw)?;
        let kind = Kind::of(&abs).ok_or_else(|| {
            ToolError::failed_with(
                format!("{raw} is not a Word, Excel or PowerPoint file"),
                "ExtractImages reads .docx, .xlsx and .pptx. Other archives have no office \
                 pictures in them.",
            )
        })?;
        let dir = match input["dir"].as_str() {
            Some(dir) => ctx.ws.resolve_new(dir)?,
            None => default_dir(&abs),
        };

        let shown = ctx.ws.display(&abs);
        let name = shown.clone();
        let media = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&abs).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ToolError::failed(format!("not found: {name}")),
                _ => ToolError::failed(e.to_string()),
            })?;
            office::media(kind, &bytes, &name)
        })
        .await
        .map_err(|_| ToolError::failed("reading the document did not complete"))??;
        ctx.check_cancelled()?;

        if media.is_empty() {
            return Ok(ToolOutput::new(format!(
                "{shown} embeds no pictures. Its text is read with Read."
            )));
        }

        std::fs::create_dir_all(&dir).map_err(|e| {
            ToolError::failed(format!("could not create {}: {e}", ctx.ws.display(&dir)))
        })?;
        let mut written = Vec::new();
        for media in &media {
            let path = dir.join(&media.name);
            std::fs::write(&path, &media.bytes).map_err(|e| {
                ToolError::failed(format!("could not write {}: {e}", ctx.ws.display(&path)))
            })?;
            written.push((ctx.ws.display(&path), media.bytes.len()));
        }

        let mut text = format!(
            "Wrote {} picture(s) to {}:\n",
            written.len(),
            ctx.ws.display(&dir)
        );
        for (path, bytes) in &written {
            text.push_str(&format!("{path} ({bytes} bytes)\n"));
        }
        text.push_str(
            "Look at one with ReadImage, or insert it with the library of the document you are \
             building.\n",
        );
        Ok(ToolOutput::new(text))
    }
}

/// `<same folder>/<file stem> media`.
fn default_dir(file: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "media".into());
    file.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem} media"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::office::tests::package_binary;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    fn document() -> Vec<u8> {
        let picture = crate::picture::tests::png(8, 8);
        package_binary(&[
            (
                "word/document.xml",
                br#"<w:document><w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#,
            ),
            ("word/media/image1.png", &picture),
            ("word/media/image2.png", &picture),
        ])
    }

    #[tokio::test]
    async fn pictures_are_written_to_the_default_folder() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("report.docx"), document()).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ExtractImages
            .run(&ctx, json!({ "path": "report.docx" }))
            .await
            .unwrap();

        assert!(out.text.contains("Wrote 2 picture(s)"), "{}", out.text);
        assert!(dir.path().join("report media/image1.png").is_file());
        assert!(dir.path().join("report media/image2.png").is_file());
    }

    #[tokio::test]
    async fn a_named_folder_is_used_and_may_be_new() {
        let dir = TempDir::new();
        let picture = crate::picture::tests::png(8, 8);
        let deck = package_binary(&[
            (
                "ppt/presentation.xml",
                br#"<p:presentation><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#,
            ),
            (
                "ppt/_rels/presentation.xml.rels",
                br#"<Relationships><Relationship Id="rId2" Target="slides/slide1.xml"/></Relationships>"#,
            ),
            ("ppt/slides/slide1.xml", br#"<p:sld><p:cSld><p:spTree/></p:cSld></p:sld>"#),
            ("ppt/media/image1.png", &picture),
        ]);
        std::fs::write(dir.path().join("deck.pptx"), deck).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ExtractImages
            .run(&ctx, json!({ "path": "deck.pptx", "dir": "out/shots" }))
            .await
            .unwrap();

        assert!(out.text.contains("out/shots"), "{}", out.text);
        assert!(dir.path().join("out/shots/image1.png").is_file());
    }

    #[tokio::test]
    async fn powerpoint_directory_entries_are_not_extracted_as_pictures() {
        use std::io::{Cursor, Write};

        let dir = TempDir::new();
        let picture = crate::picture::tests::png(8, 8);
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            writer.add_directory("ppt/media/", options).unwrap();
            writer.start_file("ppt/media/image1.png", options).unwrap();
            writer.write_all(&picture).unwrap();
            writer.finish().unwrap();
        }
        dir.write("attachments/Уравниловка и OKR.pptx", "");
        std::fs::write(
            dir.path().join("attachments/Уравниловка и OKR.pptx"),
            buffer.into_inner(),
        )
        .unwrap();
        let (ctx, _) = ctx_for(&dir);

        let out = ExtractImages
            .run(
                &ctx,
                json!({ "path": "attachments/Уравниловка и OKR.pptx" }),
            )
            .await
            .unwrap();

        assert!(out.text.contains("Wrote 1 picture(s)"), "{}", out.text);
        assert_eq!(
            std::fs::read(
                dir.path()
                    .join("attachments/Уравниловка и OKR media/image1.png")
            )
            .unwrap(),
            picture
        );
    }

    #[tokio::test]
    async fn a_file_without_pictures_says_so() {
        let dir = TempDir::new();
        let text_only = package_binary(&[(
            "word/document.xml",
            br#"<w:document><w:body><w:p><w:r><w:t>Hi</w:t></w:r></w:p></w:body></w:document>"#,
        )]);
        std::fs::write(dir.path().join("plain.docx"), text_only).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ExtractImages
            .run(&ctx, json!({ "path": "plain.docx" }))
            .await
            .unwrap();
        assert!(out.text.contains("embeds no pictures"));
    }

    #[tokio::test]
    async fn another_kind_of_file_is_refused() {
        let dir = TempDir::new();
        dir.write("notes.md", "hi");
        let (ctx, _ws) = ctx_for(&dir);

        let err = ExtractImages
            .run(&ctx, json!({ "path": "notes.md" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains(".docx"), "{err:?}");
    }
}

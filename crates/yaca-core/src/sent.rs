//! Durable file deliveries. Journals carry metadata, never file bodies.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::id::DeliveryId;
use crate::{ToolError, Workspace};

const MAX_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RenderType {
    Inline,
    Artifact,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SentFile {
    pub id: DeliveryId,
    pub name: String,
    pub title: String,
    pub caption: Option<String>,
    pub mime: String,
    #[ts(type = "number")]
    pub size: u64,
    pub sha256: String,
    pub render_type: RenderType,
}

/// The hint cannot turn executable or unknown bytes into active inline content.
pub fn routing(name: &str, hint: &str) -> Result<(&'static str, RenderType), ToolError> {
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (mime, auto) = match ext.as_str() {
        "png" => ("image/png", RenderType::Inline),
        "jpg" | "jpeg" => ("image/jpeg", RenderType::Inline),
        "webp" => ("image/webp", RenderType::Inline),
        "gif" => ("image/gif", RenderType::Inline),
        "svg" => ("image/svg+xml", RenderType::Inline),
        "mp3" => ("audio/mpeg", RenderType::Inline),
        "wav" => ("audio/wav", RenderType::Inline),
        "ogg" => ("audio/ogg", RenderType::Inline),
        "mp4" => ("video/mp4", RenderType::Inline),
        "webm" => ("video/webm", RenderType::Inline),
        "pdf" => ("application/pdf", RenderType::Artifact),
        "docx" => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            RenderType::Artifact,
        ),
        "xlsx" => (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            RenderType::Artifact,
        ),
        "pptx" => (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            RenderType::Artifact,
        ),
        "html" | "htm" => ("text/html", RenderType::Artifact),
        "md" | "markdown" => ("text/markdown", RenderType::Artifact),
        "json" => ("application/json", RenderType::Artifact),
        "csv" => ("text/csv", RenderType::Artifact),
        "txt" => ("text/plain", RenderType::Artifact),
        "zip" | "jar" => ("application/zip", RenderType::Attachment),
        "gz" => ("application/gzip", RenderType::Attachment),
        "tar" => ("application/x-tar", RenderType::Attachment),
        _ => ("application/octet-stream", RenderType::Attachment),
    };
    let render = match hint {
        "auto" => auto,
        "attachment" => RenderType::Attachment,
        "inline" if auto == RenderType::Inline => RenderType::Inline,
        "artifact" if auto != RenderType::Attachment => RenderType::Artifact,
        "inline" | "artifact" => auto,
        _ => {
            return Err(ToolError::failed(
                "display_hint must be auto, inline, artifact or attachment",
            ))
        }
    };
    Ok((mime, render))
}

pub fn deliver(
    ws: &Workspace,
    raw: &str,
    title: Option<String>,
    caption: Option<String>,
    hint: &str,
) -> Result<SentFile, ToolError> {
    let source = ws.resolve_existing(raw)?;
    let name = source
        .file_name()
        .ok_or_else(|| ToolError::failed("missing file name"))?
        .to_string_lossy()
        .into_owned();
    let (mime, render_type) = routing(&name, hint)?;
    let mut input = File::open(&source).map_err(io_error)?;
    let metadata = input.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ToolError::failed("path must name a regular file"));
    }
    if metadata.len() > MAX_BYTES {
        return Err(ToolError::failed("Send supports files up to 1 GiB"));
    }
    let id = DeliveryId::new();
    // Resolve before creating: an existing .yaca or sent symlink must not escape.
    let directory = ws.resolve_new(&format!(".yaca/sent/{id}"))?;
    fs::create_dir_all(&directory).map_err(io_error)?;
    let result = (|| {
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(directory.join("content"))
            .map_err(io_error)?;
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).map_err(io_error)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > MAX_BYTES {
                return Err(ToolError::failed("Send supports files up to 1 GiB"));
            }
            digest.update(&buffer[..count]);
            output.write_all(&buffer[..count]).map_err(io_error)?;
        }
        output.sync_all().map_err(io_error)?;
        let file = SentFile {
            id,
            title: title
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| name.clone()),
            name,
            caption,
            mime: mime.into(),
            size,
            sha256: format!("{:x}", digest.finalize()),
            render_type,
        };
        let manifest = serde_json::to_vec(&file).map_err(|e| ToolError::failed(e.to_string()))?;
        if manifest.len() > 64 * 1024 {
            return Err(ToolError::failed("delivery title or caption is too large"));
        }
        fs::write(directory.join("manifest.json"), manifest).map_err(io_error)?;
        Ok(file)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&directory);
    }
    result
}

/// Only delivery IDs are accepted; callers cannot use this as a general file server.
pub fn open(ws: &Workspace, id: &str) -> Result<(SentFile, PathBuf), ToolError> {
    if !id
        .strip_prefix("delivery_")
        .is_some_and(|raw| raw.len() == 32 && uuid::Uuid::parse_str(raw).is_ok())
    {
        return Err(ToolError::failed("invalid delivery id"));
    }
    let base = format!(".yaca/sent/{id}");
    let manifest = ws.resolve_existing(&format!("{base}/manifest.json"))?;
    let path = ws.resolve_existing(&format!("{base}/content"))?;
    if fs::metadata(&manifest).map_err(io_error)?.len() > 64 * 1024 {
        return Err(ToolError::failed("delivery metadata is too large"));
    }
    let bytes = fs::read(manifest).map_err(io_error)?;
    let file: SentFile =
        serde_json::from_slice(&bytes).map_err(|e| ToolError::failed(e.to_string()))?;
    if file.id.as_str() != id || !path.is_file() {
        return Err(ToolError::failed("invalid delivery"));
    }
    Ok((file, path))
}

pub fn preview(ws: &Workspace, id: &str) -> Result<String, ToolError> {
    let (file, path) = open(ws, id)?;
    let kind = match Path::new(&file.name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "docx" => Some(crate::office::Kind::Docx),
        "xlsx" => Some(crate::office::Kind::Xlsx),
        "pptx" => Some(crate::office::Kind::Pptx),
        _ => None,
    };
    if fs::metadata(&path).map_err(io_error)?.len() > 20 * 1024 * 1024 {
        return Err(ToolError::failed(
            "Preview supports files up to 20 MiB; download the original to view it",
        ));
    }
    let bytes = fs::read(path).map_err(io_error)?;
    match kind {
        Some(kind) => Ok(crate::office::read(kind, &bytes, &file.name)?.text),
        None => {
            String::from_utf8(bytes).map_err(|_| ToolError::failed("This file has no text preview"))
        }
    }
}

fn io_error(error: std::io::Error) -> ToolError {
    ToolError::failed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn delivery_survives_source_changes_and_preserves_bytes() {
        let dir = TempDir::new();
        dir.write("report.csv", "a,b\n1,2\n");
        let ws = Workspace::open(dir.path()).unwrap();
        let file = deliver(&ws, "report.csv", Some("Report".into()), None, "auto").unwrap();
        dir.write("report.csv", "changed");
        let (saved, path) = open(&ws, file.id.as_str()).unwrap();
        assert_eq!(saved, file);
        assert_eq!(fs::read(path).unwrap(), b"a,b\n1,2\n");
        assert_eq!(file.sha256, format!("{:x}", Sha256::digest(b"a,b\n1,2\n")));
        assert_eq!(file.render_type, RenderType::Artifact);
        assert_eq!(preview(&ws, file.id.as_str()).unwrap(), "a,b\n1,2\n");
    }

    #[test]
    fn boundaries_and_non_files_are_rejected() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        for raw in ["../../secret", "missing", "."] {
            assert!(deliver(&ws, raw, None, None, "auto").is_err());
        }
        assert!(open(&ws, "../secret").is_err());
    }

    #[test]
    fn routing_covers_media_documents_and_untrusted_extensions() {
        assert_eq!(routing("png", "auto").unwrap().1, RenderType::Attachment);
        for (name, render) in [
            ("image.SVG", RenderType::Inline),
            ("audio.mp3", RenderType::Inline),
            ("video.webm", RenderType::Inline),
            ("report.docx", RenderType::Artifact),
            ("table.xlsx", RenderType::Artifact),
            ("deck.pptx", RenderType::Artifact),
            ("page.html", RenderType::Artifact),
            ("notes.md", RenderType::Artifact),
            ("data.json", RenderType::Artifact),
            ("report.pdf", RenderType::Artifact),
            ("archive.tar.gz", RenderType::Attachment),
            ("app.exe", RenderType::Attachment),
        ] {
            assert_eq!(routing(name, "auto").unwrap().1, render, "{name}");
        }
        assert_eq!(
            routing("app.exe", "inline").unwrap().1,
            RenderType::Attachment
        );
        assert_eq!(
            routing("pic.png", "attachment").unwrap().1,
            RenderType::Attachment
        );
        assert!(routing("x.txt", "unknown").is_err());
    }
}

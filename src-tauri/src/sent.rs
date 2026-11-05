//! A read-only protocol over delivery snapshots, never arbitrary workspace files.

use std::io::{Read, Seek, SeekFrom};
use tauri::http::{Request, Response};
use yaca_core::Workspace;

pub fn response(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match serve(&request) {
        Ok(response) => response,
        Err(why) => Response::builder()
            .status(400)
            .body(why.into_bytes())
            .unwrap_or_default(),
    }
}

fn workspace(root: &str) -> Result<Workspace, String> {
    Workspace::open(crate::project_path(root)?).map_err(|e| e.to_string())
}

fn serve(request: &Request<Vec<u8>>) -> Result<Response<Vec<u8>>, String> {
    if request.method() != "GET" && request.method() != "HEAD" {
        return Err("Unsupported method".into());
    }
    let root = request
        .uri()
        .query()
        .and_then(|s| s.strip_prefix("root="))
        .ok_or("Missing root")?;
    if root.len() > 32768 || root.len() % 2 != 0 || !root.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid root".into());
    }
    let bytes: Result<Vec<u8>, _> = (0..root.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&root[i..i + 2], 16))
        .collect();
    let root = String::from_utf8(bytes.map_err(|_| "Invalid root")?).map_err(|_| "Invalid root")?;
    let ws = workspace(&root)?;
    let (file, path) = yaca_core::sent::open(&ws, request.uri().path().trim_start_matches('/'))
        .map_err(|e| e.to_string())?;
    let mut input = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let size = input.metadata().map_err(|e| e.to_string())?.len();
    let range = request.headers().get("range").and_then(|s| s.to_str().ok());
    let (start, end) = match range {
        Some(range) => match byte_range(range, size) {
            Some(range) => range,
            None => {
                return Response::builder()
                    .status(416)
                    .header("Content-Range", format!("bytes */{size}"))
                    .body(Vec::new())
                    .map_err(|e| e.to_string())
            }
        },
        None => (0, size.saturating_sub(1)),
    };
    let count = if size == 0 { 0 } else { end - start + 1 };
    let mut builder = Response::builder()
        .status(if range.is_some() { 206 } else { 200 })
        .header("Content-Type", &file.mime)
        .header("Accept-Ranges", "bytes")
        .header("Content-Length", count)
        .header("X-Content-Type-Options", "nosniff");
    // Sandboxing the PDF response also disables browser PDF plugins.
    if file.mime != "application/pdf" {
        builder = builder.header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:",
        );
    }
    if range.is_some() {
        builder = builder.header("Content-Range", format!("bytes {start}-{end}/{size}"));
    }
    let mut body = Vec::new();
    if request.method() != "HEAD" {
        if count > 64 * 1024 * 1024 {
            return Err("Preview request exceeds 64 MiB; download the file instead".into());
        }
        input
            .seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        input
            .take(count)
            .read_to_end(&mut body)
            .map_err(|e| e.to_string())?;
    }
    builder.body(body).map_err(|e| e.to_string())
}

fn byte_range(header: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }
    let (first, last) = header.strip_prefix("bytes=")?.split_once('-')?;
    let (start, end) = if first.is_empty() {
        let suffix: u64 = last.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        (size.saturating_sub(suffix), size - 1)
    } else {
        let start = first.parse::<u64>().ok()?;
        let end = if last.is_empty() {
            size - 1
        } else {
            last.parse::<u64>().ok()?.min(size - 1)
        };
        (start, end)
    };
    if start >= size || end < start {
        return None;
    }
    // Media clients request an open-ended range. Bound each IPC response.
    Some((start, end.min(start.saturating_add(8 * 1024 * 1024 - 1))))
}

#[tauri::command]
pub async fn sent_preview(project: String, id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        yaca_core::sent::preview(&workspace(&project)?, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_sent_file(
    project: String,
    id: String,
    destination: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, source) =
            yaca_core::sent::open(&workspace(&project)?, &id).map_err(|e| e.to_string())?;
        std::fs::copy(source, destination).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_serves_only_snapshots_and_supports_partial_media_requests() {
        let root = std::env::temp_dir().join(format!(
            "yaca-send-protocol-{}",
            yaca_core::id::SessionId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("clip.mp4"), b"0123456789").unwrap();
        let ws = Workspace::open(&root).unwrap();
        let file = yaca_core::sent::deliver(&ws, "clip.mp4", None, None, "auto").unwrap();
        let encoded: String = root
            .to_string_lossy()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let uri = format!("sent://localhost/{}?root={encoded}", file.id);
        let request = || Request::builder().uri(&uri).body(Vec::new()).unwrap();
        let full = response(request());
        assert_eq!(full.status(), 200);
        assert_eq!(full.body(), b"0123456789");
        assert_eq!(full.headers()["Content-Type"], "video/mp4");
        let mut partial = request();
        partial
            .headers_mut()
            .insert("range", "bytes=2-5".parse().unwrap());
        let partial = response(partial);
        assert_eq!(partial.status(), 206);
        assert_eq!(partial.headers()["Content-Range"], "bytes 2-5/10");
        assert_eq!(partial.body(), b"2345");
        let mut head = request();
        *head.method_mut() = tauri::http::Method::HEAD;
        let head = response(head);
        assert!(head.body().is_empty());
        assert_eq!(head.headers()["Content-Length"], "10");
        let invalid = Request::builder()
            .uri(format!("sent://localhost/../../clip.mp4?root={encoded}"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(response(invalid).status(), 400);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn media_ranges_cover_suffixes_limits_and_invalid_requests() {
        assert_eq!(byte_range("bytes=0-3", 10), Some((0, 3)));
        assert_eq!(byte_range("bytes=5-", 10), Some((5, 9)));
        assert_eq!(byte_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(byte_range("bytes=0-99", 10), Some((0, 9)));
        assert_eq!(
            byte_range("bytes=0-", 99_000_000),
            Some((0, 8 * 1024 * 1024 - 1))
        );
        for header in [
            "bytes=10-",
            "bytes=5-2",
            "bytes=-0",
            "bytes=0-1,3-4",
            "invalid",
        ] {
            assert_eq!(byte_range(header, 10), None);
        }
        assert_eq!(byte_range("bytes=0-", 0), None);
    }
}

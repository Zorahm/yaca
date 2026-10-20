//! What the interface asks of the disk of the machine the host runs on:
//! attachments, pasted images, and — for a window that cannot see that disk —
//! a folder browser.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::project_path;

/// What an attachment is, and its thumbnail if it is an image (§7.2).
///
/// The page gets pixels, not access: it cannot read the disk, and opening
/// the disk to it for thumbnails would open it to everything else on the
/// page. A path from the feed is written from the project root, so the root
/// comes along.
pub async fn attachment_look(project: String, path: String) -> Result<yaca_core::picture::AttachmentLook, String> {
    let root = project_path(&project)?;
    tokio::task::spawn_blocking(move || yaca_core::picture::look(&root.join(path)))
        .await
        .map_err(|e| e.to_string())
}

/// The image whole, for the viewer: the preview is too small to look into.
pub async fn image_full(project: String, path: String) -> Result<String, String> {
    let root = project_path(&project)?;
    tokio::task::spawn_blocking(move || yaca_core::picture::original(&root.join(path)).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Keeps an image pasted into the composer and returns its path from the
/// project root — the form every other attachment has (§7.2).
pub async fn stash_image(project: String, data: String) -> Result<String, String> {
    let root = project_path(&project)?;
    tokio::task::spawn_blocking(move || {
        let path = yaca_core::picture::stash_encoded(&root, &data).map_err(|e| e.to_string())?;
        let shown = path.strip_prefix(&root).unwrap_or(&path);
        Ok(shown.to_string_lossy().replace('\\', "/"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A folder in a listing.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
}

/// The folders under one folder of the host's disk.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    /// The folder as the host spells it.
    pub path: String,
    /// One level up, `None` at the root.
    pub parent: Option<String>,
    /// Where the host's user starts, for a "home" button.
    pub home: String,
    pub dirs: Vec<DirEntry>,
}

fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// Lists the folders of the host's disk: the project picker of a window that
/// is not on the same machine, whose own folder dialog would show the wrong
/// disk. Folders only, hidden ones on request; a project is always a folder.
pub fn list_dir(path: Option<String>, hidden: bool) -> Result<DirListing, String> {
    let home = home();
    let at = match path.filter(|p| !p.is_empty()) {
        Some(path) => project_path(&path)?,
        None => project_path(&home.to_string_lossy())?,
    };
    let mut dirs: Vec<DirEntry> = std::fs::read_dir(&at)
        .map_err(|e| format!("could not read {}: {e}", at.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (hidden || !name.starts_with('.')).then(|| DirEntry {
                path: entry.path().display().to_string(),
                name,
            })
        })
        .collect();
    dirs.sort_by_key(|d| d.name.to_lowercase());
    Ok(DirListing {
        parent: at.parent().map(|p| p.display().to_string()),
        path: at.display().to_string(),
        home: home.display().to_string(),
        dirs,
    })
}

/// Makes a folder named `name` inside `parent` and returns its path. The name
/// is one path component: this is a "new folder" button, not a way to write
/// anywhere with `..`.
pub fn make_dir(parent: &str, name: &str) -> Result<String, String> {
    let name = name.trim();
    let one_component = Path::new(name).components().count() == 1
        && !name.contains(['/', '\\'])
        && name != "."
        && name != "..";
    if !one_component {
        return Err(format!("\"{name}\" is not a folder name"));
    }
    let path = project_path(parent)?.join(name);
    std::fs::create_dir(&path).map_err(|e| format!("could not create {}: {e}", path.display()))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "yaca-host-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_folders_only_and_hides_dotted_ones() {
        let dir = temp();
        std::fs::create_dir(dir.join("beta")).unwrap();
        std::fs::create_dir(dir.join("Alpha")).unwrap();
        std::fs::create_dir(dir.join(".git")).unwrap();
        std::fs::write(dir.join("file.txt"), "x").unwrap();

        let listing = list_dir(Some(dir.display().to_string()), false).unwrap();
        let names: Vec<_> = listing.dirs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "beta"]);

        let all = list_dir(Some(dir.display().to_string()), true).unwrap();
        assert_eq!(all.dirs.len(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_folder_is_one_component() {
        let dir = temp();
        let parent = dir.display().to_string();
        assert!(make_dir(&parent, "../escape").is_err());
        assert!(make_dir(&parent, "a/b").is_err());
        assert!(make_dir(&parent, "..").is_err());
        assert!(make_dir(&parent, "  ").is_err());
        let made = make_dir(&parent, "proj").unwrap();
        assert!(Path::new(&made).is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_is_not_a_place_to_browse() {
        let dir = temp();
        std::fs::write(dir.join("f"), "x").unwrap();
        assert!(list_dir(Some(dir.join("f").display().to_string()), false).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

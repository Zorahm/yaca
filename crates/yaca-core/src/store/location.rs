//! Where chats are kept on disk.
//!
//! All chats live in one folder of the application's, next to its settings
//! (`registry::config_dir`), and not inside the projects they were held in:
//! uninstalling with "delete all data" has to be able to take every chat with
//! it, and a project folder is the human's, not the uninstaller's.
//!
//! ```text
//! <config_dir>/chats/<project>-<hash>/project.json   whose chats these are
//! <config_dir>/chats/<project>-<hash>/<id>.jsonl     the journals, see `store`
//! ```
//!
//! The folder name carries the project's own name so that a human browsing
//! the directory can find their way, and a hash of the full path so that two
//! projects called `app` in different places do not share their history.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::provider::registry;

/// The folder under the configuration directory that holds every chat.
pub const CHATS_DIR: &str = "chats";

/// The sidecar naming the project a chats folder belongs to.
const PROJECT_FILE: &str = "project.json";

/// How many hex digits of the path hash go into a folder name. Eight is 32
/// bits: a collision needs tens of thousands of projects on one machine.
const HASH_CHARS: usize = 8;

/// Longest project name kept in a folder name, in characters. The hash is
/// what makes the name unique; the rest is a hint for a human.
const NAME_CHARS: usize = 40;

/// The application's chats folder — the one the window and `yaca-cli` share.
pub fn chats_dir() -> Result<PathBuf, CoreError> {
    Ok(registry::config_dir()?.join(CHATS_DIR))
}

/// The folder holding the chats of `project`, under the chats folder `chats`.
///
/// Pure: nothing is created. The same project reached by a different spelling
/// of its path — another case of letters on Windows, a trailing separator, a
/// `\\?\` prefix — lands in the same folder, because the hash is taken over the
/// canonical form.
pub fn project_dir(chats: &Path, project: &Path) -> PathBuf {
    let path = display_path(project);
    let hash = blake3::hash(path_key(&path).as_bytes()).to_hex();
    // From the canonical form too, or `C:\KOD` would get a folder of its own.
    let name = folder_name(Path::new(&path));
    chats.join(format!("{name}-{}", &hash[..HASH_CHARS]))
}

/// Creates the project's chats folder and writes down whose it is.
///
/// The sidecar is written once and never read by the core: the folder is found
/// by hashing the path, not by scanning. It is there so that the directory
/// explains itself to a human, and so that a later "all chats" view has
/// something to group by.
pub fn claim(chats: &Path, project: &Path) -> Result<PathBuf, CoreError> {
    let dir = project_dir(chats, project);
    std::fs::create_dir_all(&dir)?;
    let sidecar = dir.join(PROJECT_FILE);
    if !sidecar.exists() {
        let text = serde_json::to_string_pretty(&ProjectFile {
            path: display_path(project),
        })
        .map_err(|e| CoreError::Config(e.to_string()))?;
        std::fs::write(sidecar, text)?;
    }
    Ok(dir)
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    path: String,
}

/// The path as the hash sees it.
fn path_key(path: &str) -> String {
    let text = path.trim_end_matches(['/', '\\']).to_string();
    // NTFS does not tell `C:\Kod` from `c:\kod`; the folder must not either.
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// The project's path, canonical when it exists, without the verbatim prefix
/// `canonicalize` adds on Windows.
fn display_path(project: &Path) -> String {
    let canon = std::fs::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let text = canon.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(&text).to_string(),
    }
}

/// The last component of the path, cut down to what every file system takes.
fn folder_name(project: &Path) -> String {
    let raw = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(NAME_CHARS)
        .collect();
    // A drive root has no name, and a name of dots is not a folder Windows
    // will make.
    let name = name.trim_matches('.');
    if name.is_empty() {
        "project".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn the_folder_is_named_after_the_project() {
        let dir = TempDir::new();
        let project = dir.path().join("my app");
        std::fs::create_dir(&project).unwrap();

        let folder = project_dir(Path::new("chats"), &project);
        let name = folder.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("my_app-"), "{name}");
        assert_eq!(name.len(), "my_app-".len() + HASH_CHARS);
    }

    #[test]
    fn two_projects_with_one_name_do_not_share_a_folder() {
        let dir = TempDir::new();
        let a = dir.path().join("a").join("app");
        let b = dir.path().join("b").join("app");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();

        let chats = Path::new("chats");
        assert_ne!(project_dir(chats, &a), project_dir(chats, &b));
    }

    #[test]
    fn another_spelling_of_the_path_is_the_same_project() {
        let dir = TempDir::new();
        let project = dir.path().join("Kod");
        std::fs::create_dir(&project).unwrap();
        let chats = Path::new("chats");

        let mut trailing = project.as_os_str().to_owned();
        trailing.push(std::path::MAIN_SEPARATOR_STR);
        assert_eq!(
            project_dir(chats, &project),
            project_dir(chats, Path::new(&trailing))
        );
        let canon = std::fs::canonicalize(&project).unwrap();
        assert_eq!(project_dir(chats, &project), project_dir(chats, &canon));
        if cfg!(windows) {
            let upper = PathBuf::from(project.to_string_lossy().to_uppercase());
            assert_eq!(project_dir(chats, &project), project_dir(chats, &upper));
        }
    }

    #[test]
    fn a_claimed_folder_says_whose_it_is() {
        let dir = TempDir::new();
        let project = dir.path().join("site");
        std::fs::create_dir(&project).unwrap();
        let chats = dir.path().join("chats");

        let folder = claim(&chats, &project).unwrap();
        let text = std::fs::read_to_string(folder.join(PROJECT_FILE)).unwrap();
        let file: ProjectFile = serde_json::from_str(&text).unwrap();
        assert_eq!(file.path, display_path(&project));
        assert!(!file.path.starts_with(r"\\?\"), "{}", file.path);
    }

    #[test]
    fn a_drive_root_still_gets_a_name() {
        assert_eq!(folder_name(Path::new("/")), "project");
        assert_eq!(folder_name(Path::new("..")), "project");
    }
}

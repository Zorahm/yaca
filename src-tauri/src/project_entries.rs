//! Project identity belongs to the application; repository settings remain in the primary folder.

use std::path::PathBuf;

use crate::{config_dir, project_path, settings::AppSettings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectDetails {
    pub name: String,
    pub additional_folders: Vec<PathBuf>,
}

/// The primary path stays stable so renaming a project never disconnects its journals.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    pub folders: Vec<String>,
}

pub fn entries(settings: &AppSettings) -> Vec<ProjectEntry> {
    settings
        .projects()
        .into_iter()
        .map(|path| {
            let details = settings.project_details(path);
            let name = if details.name.trim().is_empty() {
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            } else {
                details.name
            };
            let folders = std::iter::once(path.to_path_buf())
                .chain(details.additional_folders)
                .map(|folder| folder.to_string_lossy().into_owned())
                .collect();
            ProjectEntry {
                path: path.to_string_lossy().into_owned(),
                name,
                folders,
            }
        })
        .collect()
}

#[tauri::command]
pub fn project_entries() -> Result<Vec<ProjectEntry>, String> {
    Ok(entries(&AppSettings::load(&config_dir()?)))
}

fn validated(name: &str, folders: &[String]) -> Result<(PathBuf, ProjectDetails), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("project name cannot be empty".into());
    }
    let first = folders
        .first()
        .ok_or("a project needs at least one folder")?;
    let primary = project_path(first)?;
    let extra: Vec<_> = folders.iter().skip(1).map(PathBuf::from).collect();
    let workspace = yaca_core::Workspace::open(&primary)
        .and_then(|workspace| workspace.with_roots(&extra))
        .map_err(|e| e.to_string())?;
    Ok((
        primary,
        ProjectDetails {
            name: name.to_owned(),
            additional_folders: workspace.additional().to_vec(),
        },
    ))
}

#[tauri::command]
pub fn save_project_entry(
    path: Option<String>,
    name: String,
    folders: Vec<String>,
) -> Result<Vec<ProjectEntry>, String> {
    save_entry(&config_dir()?, path, name, folders)
}

fn save_entry(
    dir: &std::path::Path,
    path: Option<String>,
    name: String,
    folders: Vec<String>,
) -> Result<Vec<ProjectEntry>, String> {
    let (primary, details) = validated(&name, &folders)?;
    if let Some(path) = &path {
        if project_path(path)? != primary {
            return Err("the primary project folder cannot be changed".into());
        }
    }
    let mut settings = AppSettings::load(dir);
    if path.is_none() {
        if settings.projects().contains(&primary.as_path()) {
            return Err("a project with this primary folder already exists".into());
        }
        settings.remember_project(primary.clone());
    } else if !settings.projects().contains(&primary.as_path()) {
        return Err("project is no longer in the project list".into());
    }
    settings.set_project_details(primary, details);
    settings.save(dir).map_err(|e| e.to_string())?;
    Ok(entries(&settings))
}

/// Resuming always uses the journal's original boundary, even after a project was edited.
pub fn session_folders(
    settings: &AppSettings,
    root: &std::path::Path,
    resume: Option<&str>,
    store: &yaca_core::store::SessionStore,
) -> Vec<PathBuf> {
    match resume {
        Some(id) => store
            .meta(&yaca_core::id::SessionId::from(id.to_owned()))
            .map(|meta| {
                meta.additional_roots
                    .into_iter()
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default(),
        None => settings.project_details(root).additional_folders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Folders(PathBuf);
    impl Folders {
        fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let root = std::env::temp_dir().join(format!(
                "yaca-project-entries-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("primary/nested")).unwrap();
            fs::create_dir_all(root.join("extra")).unwrap();
            Self(root)
        }
        fn folder(&self, name: &str) -> String {
            self.0.join(name).to_string_lossy().into_owned()
        }
    }
    impl Drop for Folders {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_entries_migrate_paths_and_persist_names_and_folders() {
        let dirs = Folders::new();
        let primary = project_path(&dirs.folder("primary")).unwrap();
        let extra = project_path(&dirs.folder("extra")).unwrap();
        fs::write(
            dirs.0.join("settings.json"),
            serde_json::json!({"projects": [primary]}).to_string(),
        )
        .unwrap();
        let mut settings = AppSettings::load(&dirs.0);
        assert_eq!(entries(&settings)[0].name, "primary");
        let (_, details) = validated(
            "  My workspace  ",
            &[dirs.folder("primary"), dirs.folder("extra")],
        )
        .unwrap();
        settings.set_project_details(primary.clone(), details);
        settings.save(&dirs.0).unwrap();
        let mut settings = AppSettings::load(&dirs.0);
        settings.remember_project(primary.clone());
        let entry = entries(&settings).remove(0);
        assert_eq!(entry.name, "My workspace");
        assert_eq!(
            entry.folders,
            vec![primary.to_string_lossy(), extra.to_string_lossy()]
        );
        settings.forget_project(&primary);
        assert!(entries(&settings).is_empty());
        assert_eq!(
            settings.project_details(&primary),
            ProjectDetails::default()
        );
        assert!(primary.is_dir());
        assert!(extra.is_dir());
    }

    #[test]
    fn project_entries_create_edit_and_reject_duplicate_or_changed_primary() {
        let dirs = Folders::new();
        let folders = vec![dirs.folder("primary"), dirs.folder("extra")];
        let created = save_entry(&dirs.0, None, "Created".into(), folders.clone()).unwrap();
        let path = created[0].path.clone();
        assert_eq!(created[0].folders.len(), 2);
        assert!(save_entry(&dirs.0, None, "Duplicate".into(), folders).is_err());
        assert!(save_entry(
            &dirs.0,
            Some(path.clone()),
            "Moved".into(),
            vec![dirs.folder("extra")]
        )
        .is_err());
        let edited = save_entry(
            &dirs.0,
            Some(path.clone()),
            "  Renamed  ".into(),
            vec![path.clone()],
        )
        .unwrap();
        assert_eq!(edited.len(), 1);
        assert_eq!(edited[0].path, path);
        assert_eq!(edited[0].name, "Renamed");
        assert_eq!(edited[0].folders.len(), 1);
        assert_eq!(entries(&AppSettings::load(&dirs.0))[0].name, "Renamed");
    }

    #[test]
    fn project_entries_validate_boundaries_and_remove_redundant_folders() {
        let dirs = Folders::new();
        let primary = dirs.folder("primary");
        let extra = dirs.folder("extra");
        let (_, details) = validated(
            "Workspace",
            &[
                primary.clone(),
                primary.clone(),
                dirs.folder("primary/nested"),
                extra.clone(),
                extra,
            ],
        )
        .unwrap();
        assert_eq!(
            details.additional_folders,
            vec![project_path(&dirs.folder("extra")).unwrap()]
        );
        assert!(validated(" ", std::slice::from_ref(&primary)).is_err());
        assert!(validated("Workspace", &[]).is_err());
        assert!(validated("Workspace", &[primary.clone(), dirs.folder("missing")]).is_err());
        assert!(validated("Workspace", &[primary, dirs.folder("")]).is_err());
    }

    #[test]
    fn project_entries_resume_uses_recorded_folders_after_project_changes() {
        let dirs = Folders::new();
        let primary = project_path(&dirs.folder("primary")).unwrap();
        let extra = project_path(&dirs.folder("extra")).unwrap();
        let mut settings = AppSettings::default();
        settings.set_project_details(
            primary.clone(),
            ProjectDetails {
                name: "Workspace".into(),
                additional_folders: vec![extra.clone()],
            },
        );
        let chats = dirs.0.join("chats");
        let store = yaca_core::store::SessionStore::open(&chats, &primary);
        assert_eq!(
            session_folders(&settings, &primary, None, &store),
            vec![extra.clone()]
        );
        let id = yaca_core::id::SessionId::from("recorded");
        let mut meta = yaca_core::store::SessionMeta::new(id, "".into(), "".into());
        meta.additional_roots = vec![extra.to_string_lossy().into_owned()];
        meta.save(&yaca_core::store::location::project_dir(&chats, &primary))
            .unwrap();
        settings.set_project_details(primary.clone(), ProjectDetails::default());
        assert_eq!(
            session_folders(&settings, &primary, Some("recorded"), &store),
            vec![extra]
        );
        assert!(session_folders(&settings, &primary, Some("older"), &store).is_empty());
        assert!(session_folders(&settings, &primary, None, &store).is_empty());
    }
}

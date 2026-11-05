//! Application-owned servers, shared by chats working in the same tree.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::config::{self, ServerSpec};
use super::Lsp;
use crate::error::CoreError;
use crate::event::EventSink;

pub struct Registry {
    pub(crate) config_dir: PathBuf,
    projects: Mutex<BTreeMap<PathBuf, Lsp>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("config_dir", &self.config_dir)
            .finish_non_exhaustive()
    }
}

impl Registry {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            projects: Mutex::default(),
        }
    }

    /// Open chats retain their connection; an unused project can release it.
    pub fn forget(&self, root: &Path) {
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        self.projects.lock().unwrap().remove(&root);
    }

    /// Canonical roots keep a worktree separate while sharing ordinary chats.
    pub fn project(&self, root: &Path) -> Result<Lsp, CoreError> {
        let root = std::fs::canonicalize(root)?;
        let mut projects = self.projects.lock().unwrap();
        if let Some(lsp) = projects.get(&root) {
            return Ok(lsp.clone());
        }
        let global = config::load_global(&self.config_dir)?;
        let mut specs = config::resolve(&global, |_| true);
        let extensions = project_extensions(&root);
        specs.retain(|spec| {
            !spec.builtin
                || global.contains_key(&spec.name)
                || spec
                    .settings
                    .extensions
                    .iter()
                    .any(|ext| extensions.contains(ext))
        });
        for spec in &mut specs {
            use_managed_program(spec, &self.config_dir);
        }
        let key = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
        let preferences = self.config_dir.join("lsp").join(format!("{key}.json"));
        let disabled = read_preferences(&preferences)?;
        for spec in &mut specs {
            if let Some(value) = disabled.get(&spec.name) {
                spec.settings.disabled = *value;
            }
        }
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let lsp = Lsp::managed(
            specs,
            &root,
            EventSink::new(tx),
            self.config_dir.clone(),
            preferences,
        );
        lsp.start();
        projects.insert(root, lsp.clone());
        Ok(lsp)
    }
}

pub(crate) fn read_preferences(path: &Path) -> Result<BTreeMap<String, bool>, CoreError> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| CoreError::Config(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn use_managed_program(spec: &mut ServerSpec, dir: &Path) {
    let Some(default) = config::builtin(&spec.name) else {
        return;
    };
    // An explicit command remains the human's choice.
    if spec.settings.command != default.command {
        return;
    }
    let bin = managed_bin(dir, &spec.name, &default.command);
    if bin.is_file() {
        spec.settings.command = bin.to_string_lossy().into_owned();
    }
}

pub(crate) fn managed_bin(dir: &Path, name: &str, program: &str) -> PathBuf {
    let base = dir.join("lsp").join("tools").join(name);
    if name == "gopls" {
        base.join(if cfg!(windows) { "gopls.exe" } else { "gopls" })
    } else {
        base.join("node_modules").join(".bin").join(format!(
            "{program}{}",
            if cfg!(windows) { ".cmd" } else { "" }
        ))
    }
}

/// Ignore dependencies even in a new project without a .gitignore. The bound
/// keeps opening a large tree from monopolizing application startup.
fn project_extensions(root: &Path) -> BTreeSet<String> {
    ignore::WalkBuilder::new(root)
        .require_git(false)
        .max_depth(Some(16))
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some("node_modules" | "target" | "vendor" | "dist" | "build" | ".venv" | "venv")
            )
        })
        .build()
        .take(50_000)
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let path = entry.path();
            match path.file_name()?.to_str()? {
                "Cargo.toml" => Some("rs".into()),
                "tsconfig.json" | "jsconfig.json" | "package.json" => Some("ts".into()),
                "pyproject.toml" | "requirements.txt" => Some("py".into()),
                "go.mod" => Some("go".into()),
                _ => path.extension()?.to_str().map(str::to_ascii_lowercase),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn project_languages_exclude_dependencies_and_include_manifests() {
        let dir = TempDir::new();
        dir.write("nested/src/main.rs", "");
        dir.write("frontend/tsconfig.json", "{}");
        dir.write("node_modules/pkg/test.py", "");
        dir.write("target/generated/a.go", "");
        let extensions = project_extensions(dir.path());
        assert!(extensions.contains("rs") && extensions.contains("ts"));
        assert!(!extensions.contains("py") && !extensions.contains("go"));
    }

    #[test]
    fn a_missing_builtin_is_visible_and_does_not_start_before_installation() {
        let dir = TempDir::new();
        let mut specs =
            config::resolve(&BTreeMap::new(), |program| program == "pyright-langserver");
        specs[0].settings.command = managed_bin(dir.path(), "pyright", "pyright-langserver")
            .to_string_lossy()
            .into_owned();
        let (tx, _) = tokio::sync::broadcast::channel(16);
        let lsp = Lsp::managed(
            specs,
            dir.path(),
            EventSink::new(tx),
            dir.path().to_path_buf(),
            dir.path().join("switches.json"),
        );
        lsp.start();
        let server = &lsp.snapshot()[0];
        assert_eq!(server.status, super::super::LspStatus::Missing);
        assert!(server.installation.as_deref().unwrap().contains("pyright"));
    }
}

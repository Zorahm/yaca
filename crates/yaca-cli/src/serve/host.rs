//! The host `yaca serve` gives the remote server: the projects it was started
//! with, and the chats it runs for a phone.
//!
//! Live chats are kept here in the order they were opened, as the window keeps
//! them. Recorded chats are not kept at all: they are read from the chats folder
//! the window reads too, so a chat a phone starts shows up in the window's list.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::sync::watch;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::provider::registry::{self, Providers};
use yaca_core::router::attach::{self, Status};
use yaca_core::store::{self, SessionStore};
use yaca_core::usage::ModelInfo;
use yaca_core::{Command, SessionMeta};
use yaca_remote::protocol::Startup;
use yaca_remote::{Host, LiveChat};

use crate::session_config::{session_config, Choice};

/// The folders the server is started with, canonical, in the order given, with
/// repeats dropped. A folder that is not there is an error at start, rather than
/// a phone's list that quietly lacks it.
pub fn canonical_projects(requested: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut projects: Vec<PathBuf> = Vec::new();
    for path in requested {
        let canon = std::fs::canonicalize(path).with_context(|| {
            format!(
                "project {} does not exist or cannot be opened",
                path.display()
            )
        })?;
        if !canon.is_dir() {
            bail!("project {} is not a folder", path.display());
        }
        if !projects.contains(&canon) {
            projects.push(canon);
        }
    }
    Ok(projects)
}

/// A served folder as the phone lists it. `canonicalize` gives Windows paths a
/// verbatim prefix, `\\?\C:\Kod`, which the core's store takes off as well; the
/// phone is shown the path a person writes.
pub fn shown(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(&text).to_string(),
    }
}

pub struct CliHost {
    /// Canonical, as [`canonical_projects`] makes them.
    projects: Vec<PathBuf>,
    live: Mutex<Vec<LiveChat>>,
    changed: watch::Sender<u64>,
}

impl CliHost {
    pub fn new(projects: Vec<PathBuf>) -> Self {
        let (changed, _) = watch::channel(0);
        Self {
            projects,
            live: Mutex::new(Vec::new()),
            changed,
        }
    }

    /// The served folder that `project` names, however it is spelled: a trailing
    /// separator or a `.` segment names the same folder. Anything else is refused
    /// with the same words, so a phone cannot tell an unserved folder from a
    /// folder that is not there.
    fn served(&self, project: &str) -> Result<&Path, String> {
        let not_served = || format!("this host does not serve {project}");
        let wanted = std::fs::canonicalize(project).map_err(|_| not_served())?;
        self.projects
            .iter()
            .find(|served| **served == wanted)
            .map(PathBuf::as_path)
            .ok_or_else(not_served)
    }

    fn bump(&self) {
        self.changed
            .send_modify(|count| *count = count.wrapping_add(1));
    }
}

#[async_trait]
impl Host for CliHost {
    fn name(&self) -> String {
        yaca_remote::host::machine_name()
    }

    fn projects(&self) -> Result<Vec<String>, String> {
        Ok(self.projects.iter().map(|p| shown(p)).collect())
    }

    fn sessions(&self, project: &str) -> Result<Vec<SessionMeta>, String> {
        Ok(recorded(self.served(project)?)?.list())
    }

    fn feed(&self, project: &str, session: &SessionId) -> Result<Vec<AgentEvent>, String> {
        Ok(recorded(self.served(project)?)?.feed(session))
    }

    fn models(&self) -> Result<Vec<ModelInfo>, String> {
        let dir = registry::config_dir().map_err(|e| e.to_string())?;
        let providers = Providers::load(&dir).map_err(|e| e.to_string())?;
        let mut models = providers.all_models();
        // The router is listed beside the providers' models when its file is
        // usable, as the window's switcher lists it (`routing::offered` there).
        if let Status::Ready(router) = attach::status(&dir, &providers) {
            models.push(router);
        }
        Ok(models)
    }

    async fn open_chat(
        &self,
        project: &str,
        resume: Option<SessionId>,
        startup: Option<Startup>,
    ) -> Result<LiveChat, String> {
        let root = self.served(project)?;
        let name = shown(root);
        // Held across the start, so two phones asking for the same recorded chat
        // at once get one chat, not two. Nothing below awaits.
        let mut live = lock(&self.live);
        if let Some(id) = &resume {
            if let Some(running) = live.iter().find(|c| c.handle.id() == id) {
                return Ok(running.clone());
            }
            if recorded(root)?.meta(id).is_none() {
                return Err(format!("there is no recorded chat {id} in {name}"));
            }
        }
        let (config, provider) =
            session_config(root, choice_for(startup, resume)).map_err(|e| format!("{e:#}"))?;
        let handle = yaca_core::spawn(config, provider).map_err(|e| e.to_string())?;
        let chat = LiveChat {
            handle: Arc::new(handle),
            project: name,
        };
        live.push(chat.clone());
        drop(live);
        self.bump();
        Ok(chat)
    }

    fn live_chats(&self) -> Vec<LiveChat> {
        lock(&self.live).clone()
    }

    fn chat(&self, session: &SessionId) -> Option<LiveChat> {
        lock(&self.live)
            .iter()
            .find(|c| c.handle.id() == session)
            .cloned()
    }

    async fn close_chat(&self, session: &SessionId) -> Result<(), String> {
        let closed = {
            let mut live = lock(&self.live);
            let index = live.iter().position(|c| c.handle.id() == session);
            index.map(|i| live.remove(i))
        };
        if let Some(chat) = closed {
            // Cancel before the handle goes: otherwise the started turn would
            // finish its work and only then notice that nobody is waiting for
            // it. The window does the same.
            let _ = chat.handle.send(Command::Cancel).await;
            self.bump();
        }
        Ok(())
    }

    fn chats_changed(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
}

/// The choice a phone's start makes. With a `startup`, the four the desktop's
/// home screen makes; without one, the project's settings decide. Every chat
/// `yaca serve` starts is journaled, so it can be resumed and is listed by the
/// window.
fn choice_for(startup: Option<Startup>, resume: Option<SessionId>) -> Choice {
    let (provider, model, mode, profile, effort) = match startup {
        Some(s) => (
            s.provider,
            s.model,
            Some(s.mode),
            Some(s.profile),
            Some(s.effort),
        ),
        None => (String::new(), String::new(), None, None, None),
    };
    Choice {
        provider,
        model,
        mode,
        profile,
        effort,
        resume,
        journal: true,
        isolate: false,
        additional_roots: Vec::new(),
    }
}

fn recorded(root: &Path) -> Result<SessionStore, String> {
    let chats = store::chats_dir().map_err(|e| e.to_string())?;
    Ok(SessionStore::open(&chats, root))
}

/// The lock guards a list that is only ever swapped whole, so a poisoned one
/// still holds a consistent list.
fn lock<T>(list: &Mutex<T>) -> MutexGuard<'_, T> {
    list.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use std::path::MAIN_SEPARATOR;

    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary folder")
    }

    fn host_serving(folder: &Path) -> CliHost {
        CliHost::new(canonical_projects(&[folder.to_path_buf()]).unwrap())
    }

    #[test]
    fn a_served_folder_matches_with_a_trailing_separator_or_a_dot_segment() {
        let dir = temp_dir();
        let host = host_serving(dir.path());
        let trailing = format!("{}{MAIN_SEPARATOR}", dir.path().display());
        let dotted = dir.path().join(".");
        assert!(host.served(&trailing).is_ok());
        assert!(host.served(&dotted.display().to_string()).is_ok());
        assert!(host.served(&dir.path().display().to_string()).is_ok());
    }

    #[test]
    fn an_unserved_folder_is_refused_in_the_same_words_as_a_missing_one() {
        let served = temp_dir();
        let other = temp_dir();
        let host = host_serving(served.path());
        let unserved = other.path().display().to_string();
        assert_eq!(
            host.served(&unserved).unwrap_err(),
            format!("this host does not serve {unserved}")
        );
        let missing = served.path().join("not-there").display().to_string();
        assert_eq!(
            host.served(&missing).unwrap_err(),
            format!("this host does not serve {missing}")
        );
    }

    #[test]
    fn a_parent_of_a_served_folder_is_not_served() {
        let dir = temp_dir();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let host = host_serving(&child);
        assert!(host.served(&dir.path().display().to_string()).is_err());
    }

    #[test]
    fn the_projects_are_listed_in_the_order_given_without_repeats() {
        let a = temp_dir();
        let b = temp_dir();
        let projects = canonical_projects(&[
            b.path().to_path_buf(),
            a.path().to_path_buf(),
            b.path().join("."),
        ])
        .unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0], std::fs::canonicalize(b.path()).unwrap());
        assert_eq!(projects[1], std::fs::canonicalize(a.path()).unwrap());
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error_at_start() {
        let dir = temp_dir();
        let missing = dir.path().join("gone");
        let error = canonical_projects(&[missing]).unwrap_err();
        assert!(error.to_string().contains("does not exist"), "{error}");
    }

    #[test]
    fn a_file_is_not_a_project() {
        let dir = temp_dir();
        let file = dir.path().join("note.txt");
        std::fs::write(&file, b"x").unwrap();
        let error = canonical_projects(&[file]).unwrap_err();
        assert!(error.to_string().contains("is not a folder"), "{error}");
    }

    #[test]
    fn the_listed_name_drops_the_verbatim_prefix_windows_adds() {
        assert_eq!(shown(Path::new(r"\\?\C:\Kod")), r"C:\Kod");
        assert_eq!(
            shown(Path::new(r"\\?\UNC\server\share\dir")),
            r"\\server\share\dir"
        );
        assert_eq!(shown(Path::new("/home/me/project")), "/home/me/project");
    }

    #[test]
    fn a_chat_without_a_startup_leaves_the_project_in_charge() {
        let choice = choice_for(None, None);
        assert!(choice.provider.is_empty() && choice.model.is_empty());
        assert!(choice.mode.is_none() && choice.effort.is_none());
        assert!(choice.journal);
    }
}

//! The recorded sessions of a project (§9), and the slash commands and skills
//! that belong to the project rather than to an open chat.

use yaca_core::event::AgentEvent;
use yaca_core::id::{SessionId, TurnId};
use yaca_core::slash::{self, SlashCommand};
use yaca_core::store::{SearchHit, SessionMeta, SessionStore};

use crate::{config_dir, project_path, Host};

/// The project's session store (§9). It works with the project, not with an
/// open chat: the journals also live with a project in which nobody is talking
/// at the moment.
fn store(project: &str) -> Result<SessionStore, String> {
    let chats = yaca_core::store::chats_dir().map_err(|e| e.to_string())?;
    Ok(SessionStore::open(&chats, &project_path(project)?))
}

pub fn list_sessions(project: &str) -> Result<Vec<SessionMeta>, String> {
    Ok(store(project)?.list())
}

pub fn search_sessions(project: &str, query: &str, limit: Option<usize>) -> Result<Vec<SearchHit>, String> {
    Ok(store(project)?.search(query, limit.unwrap_or(50)))
}

pub fn duplicate_session(project: &str, id: String, through: Option<String>) -> Result<SessionMeta, String> {
    store(project)?
        .duplicate(&SessionId::from(id), through.map(TurnId::from).as_ref())
        .map_err(|e| e.to_string())
}

/// Export to Markdown (§9). The window chooses the path — the save dialog is
/// its own — but the core assembles and writes the file: a conversation that
/// went through the webview only to come back into a file is an extra copy of
/// it in the page's memory.
///
/// The path is on the machine the host runs on. A window connected to a remote
/// host cannot name such a path, which is what [`export_session_text`] is for.
pub fn export_session(project: &str, id: String, path: &str) -> Result<(), String> {
    let text = export_session_text(project, id)?;
    std::fs::write(path, text).map_err(|e| format!("could not write {path}: {e}"))
}

/// The same Markdown as [`export_session`] writes, for a window that has to
/// put it into a file on its own side.
pub fn export_session_text(project: &str, id: String) -> Result<String, String> {
    store(project)?
        .export_markdown(&SessionId::from(id))
        .map_err(|e| e.to_string())
}

/// The feed of a recorded session (§9) — the journal as an open window can
/// replay it again. What falls out of it, and why — `yaca_core::store::replay`.
pub fn session_feed(project: &str, id: String) -> Result<Vec<AgentEvent>, String> {
    Ok(store(project)?.feed(&SessionId::from(id)))
}

/// The project's slash commands (§7.3) — what the composer's menu completes the
/// input with.
///
/// They are read per project, not per chat: the files live in `.yaca/commands`,
/// and several chats in one project may be open. The core expands them at the
/// moment of sending, not the window: what goes to the model is the core's
/// decision.
pub fn project_commands(project: &str) -> Result<Vec<SlashCommand>, String> {
    let mut commands = slash::load(&project_path(project)?);
    // After the project's: the core looks a typed name up in the same order.
    let plugins = yaca_core::plugin::PluginSources::standard(&config_dir()?).load().enabled();
    let taken: std::collections::HashSet<String> = commands.iter().map(|c| c.name.clone()).collect();
    commands.extend(
        yaca_core::plugin::commands(&plugins)
            .into_iter()
            .filter(|c| !taken.contains(&c.name)),
    );
    Ok(commands)
}

/// The skills a chat in this project sees, with what is wrong with those that
/// did not load: for the slash menu and the settings window. Scanned on every
/// call, like the commands, so a skill written a moment ago is there.
pub fn project_skills(project: &str) -> Result<yaca_core::skills::SkillReport, String> {
    let config = config_dir()?;
    let plugins = yaca_core::plugin::PluginSources::standard(&config).load().enabled();
    Ok(yaca_core::skills::SkillRoots::new(&project_path(project)?, yaca_core::skills::user_dirs(&config))
        .with_plugins(&plugins)
        .scan()
        .report())
}

impl Host {
    pub fn rename_session(&self, project: &str, id: String, title: &str) -> Result<(), String> {
        let id = SessionId::from(id);
        // An open chat owns its index and writes it whole when a turn ends: a
        // name put into the file behind its back lasts until then.
        match self.state.chat(&id) {
            Some(chat) => chat.rename(title),
            None => store(project)?.rename(&id, title),
        }
        .map_err(|e| e.to_string())
    }

    /// An open chat is closed first: a deleted session that kept running would go
    /// on editing files with nobody watching it.
    pub async fn delete_session(&self, project: &str, id: String) -> Result<(), String> {
        let id = SessionId::from(id);
        self.release(&id).await;
        store(project)?.delete(&id).map_err(|e| e.to_string())
    }
}

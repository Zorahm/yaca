//! The projects the interface knows and the chats open in them.

use std::path::PathBuf;
use std::sync::Arc;

use yaca_core::event::{AgentEvent, SessionState};
use yaca_core::id::SessionId;
use yaca_core::provider::registry::Providers;
use yaca_core::router::ROUTER_ID;
use yaca_core::{Command, Effort, Mode, SessionConfig};

use crate::settings::AppSettings;
use crate::{bridge, config_dir, project_path, routing, Host};

fn listed(settings: &AppSettings) -> Vec<String> {
    settings
        .projects()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// What a session starts with when it does not exist yet.
///
/// `setMode` and its neighbours are session commands, but the home screen
/// composer (§9.1) picks all of this before there is a session. A type of its
/// own, not three arguments: they always travel together and always mean one
/// thing — "the person chose before the first word".
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub mode: Mode,
    pub provider: String,
    pub model: String,
    pub effort: Effort,
}

/// An open chat: its identifier and its state at the moment it was opened.
///
/// The identifier comes back at once, because the window addresses everything
/// by it from then on: there are many chats, and a command without the
/// session's name does not know who it is for.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OpenedChat {
    pub id: SessionId,
    pub state: SessionState,
    /// The chat's MCP servers, for the same reason as the state: they start
    /// connecting when the session does, before the window listens.
    pub mcp: Vec<yaca_core::McpServerSummary>,
    /// The chat's language servers, listed when the session opens for the
    /// same reason.
    pub lsp: Vec<yaca_core::LspServerSummary>,
}

/// A chat that kept running while the window was gone, as the reloaded window
/// picks it up again.
///
/// `events` is what the chat's feed is the fold of: the journal as written, then
/// the state that is never written down (mode and model, servers, background
/// commands, the queue, the checkpoint list). Folded, it is the feed the window
/// had before the reload, with a turn that is still running still running and a
/// question that is still open still open.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AttachedChat {
    pub id: SessionId,
    /// The project the window opened the chat in.
    pub project: String,
    /// The bridge this snapshot belongs to; frames of any other are not folded.
    pub epoch: u32,
    pub events: Vec<AgentEvent>,
}

impl Host {
    /// The projects the window knows (§9.1), most recent first.
    ///
    /// The list survives a start, open chats do not: the window comes up with an
    /// empty conversation sidebar, not with an attempt to revive yesterday's.
    pub fn projects(&self) -> Result<Vec<String>, String> {
        Ok(listed(&AppSettings::load(&config_dir()?)))
    }

    /// Adds a project to the list without starting a chat (§9.1).
    ///
    /// The home screen is shown before the first message, and `spawn` writes the
    /// session's journal at once — otherwise the §9 list would pile up an empty
    /// entry for every project in which nothing was asked.
    pub fn add_project(&self, path: &str) -> Result<Vec<String>, String> {
        let path = project_path(path)?;
        let dir = config_dir()?;
        let mut settings = AppSettings::load(&dir);
        settings.remember_project(path);
        settings.save(&dir).map_err(|e| e.to_string())?;
        Ok(listed(&settings))
    }

    /// Removes a project from the list. It does not touch the folder: forgetting a
    /// project is an action of the window, not an edit of the disk, and the §9
    /// journals stay in the app's chats folder.
    pub fn forget_project(&self, path: &str) -> Result<Vec<String>, String> {
        // Without `project_path`: a project whose folder is gone must be forgotten
        // too, or the vanished folder would stay in the list forever.
        let dir = config_dir()?;
        let mut settings = AppSettings::load(&dir);
        settings.forget_project(&PathBuf::from(path));
        settings.save(&dir).map_err(|e| e.to_string())?;
        Ok(listed(&settings))
    }

    /// What the project itself would start a session with (§10): the base for the
    /// bottom row of §7.1 on the home screen.
    ///
    /// Without this the composer would show the built-in defaults and silently
    /// overwrite `.yaca/settings.json` with them — §10 requires the reverse order:
    /// the project's settings are the base, and what is chosen in the window is
    /// laid over them.
    ///
    /// The pair is settled against the provider list here, not only at the first
    /// message: a project that names nothing starts on the first provider that
    /// answered, and the composer should show that model rather than an empty
    /// chip. None answered — the error says so, and the home screen does not send.
    pub fn project_startup(&self, path: &str) -> Result<Startup, String> {
        let config = SessionConfig::open(path).map_err(|e| e.to_string())?;
        // The router is not in the provider list, it is made of it: a project that
        // names it is settled against `routing.json`, or the home screen would
        // show an error for a choice that is perfectly valid.
        if config.provider == ROUTER_ID {
            return Ok(Startup {
                mode: config.mode,
                provider: ROUTER_ID.to_string(),
                model: routing::require()?.id,
                effort: config.effort,
            });
        }
        let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
        let (spec, model) = providers
            .resolve(&config.provider, &config.model)
            .map_err(|e| e.to_string())?;
        Ok(Startup {
            mode: config.mode,
            provider: spec.id.clone(),
            model,
            effort: config.effort,
        })
    }

    /// Opens a chat in the project: a new one or a recorded one (§9).
    ///
    /// The chat becomes one more live chat rather than replacing the previous one:
    /// there are as many conversations in the window as were opened, and a turn
    /// started in one goes on while the person looks at another.
    pub async fn open_project(
        &self,
        path: String,
        resume: Option<String>,
        startup: Option<Startup>,
    ) -> Result<OpenedChat, String> {
        let root = project_path(&path)?;
        let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;

        // The project's settings are the base; what is chosen in the window goes on top of them (§10).
        let mut config = SessionConfig::open(root.as_path()).map_err(|e| e.to_string())?;
        // These are application-level settings and are not in the project's
        // file: they reach the core as config fields, like the mode and the effort.
        let prefs = self.state.prefs().get();
        config.respect_robots = prefs.respect_robots;
        config.shell = prefs.shell;
        // The human's own MCP servers and their approvals of a project's: both
        // live in the configuration directory, like the providers.
        let config_home = config_dir()?;
        config.mcp_servers =
            yaca_core::mcp::config::load_global(&config_home).map_err(|e| e.to_string())?;
        // The human's own language servers, from the same directory.
        config.lsp_servers =
            yaca_core::lsp::config::load_global(&config_home).map_err(|e| e.to_string())?;
        config.lsp_autodetect = true;
        config.user_skills = yaca_core::skills::user_dirs(&config_home);
        config.user_agents = yaca_core::agents::user_dirs(&config_home);
        config.plugins = yaca_core::plugin::PluginSources::standard(&config_home).load().enabled();
        config.mcp_approvals = Some(config_home);
        config.journal = Some(yaca_core::store::chats_dir().map_err(|e| e.to_string())?);
        if let Some(id) = resume {
            config.resume_session(SessionId::from(id));
        }

        if let Some(startup) = startup {
            config.mode = startup.mode;
            config.choose_model(startup.provider, startup.model);
            config.effort = startup.effort;
        }

        // The small model the `Decision` tool asks, if there is one.
        config.decision = yaca_core::router::attach::decision_setup(&config_dir()?, &providers);

        // Whatever the shell adds around a chat (the desktop's browser column).
        let prepared = self.shell.prepare_chat(&mut config);

        let provider = if config.provider == ROUTER_ID {
            // The same code the CLI runs: the session gets the router and a model
            // list that prices each request by the model that answered.
            yaca_core::router::attach::attach(&config_dir()?, &providers, &mut config)
                .map_err(|e| e.to_string())?
        } else {
            let (spec, model) = providers
                .resolve(&config.provider, &config.model)
                .map_err(|e| e.to_string())?;
            let spec = spec.clone();

            // No key check here: a missing key is not a reason to refuse the chat.
            // The request goes without it, and a provider that wants one answers with
            // an error naming the key (§10).

            config.provider = spec.id.clone();
            config.model = model;
            // The context window and the prices of §7.1 are counted from the
            // provider's own models, not from the built-in catalogue: a foreign
            // provider has its own.
            config.models = spec.models.clone();

            yaca_core::provider::build(spec)
        };
        let handle = Arc::new(yaca_core::spawn(config, provider).map_err(|e| e.to_string())?);

        // The project the chat was opened in moves to the top of the list: the
        // sidebar is read from the top down, and today's work is at the top.
        let dir = config_dir()?;
        let mut settings = AppSettings::load(&dir);
        settings.remember_project(root);
        settings.save(&dir).map_err(|e| e.to_string())?;

        let id = handle.id().clone();
        self.shell.chat_opened(&id, prepared);
        self.state.open(handle.clone(), path, &self.shell);
        // Read after the bridge is listening, so a server that becomes ready in
        // between is in one of the two: this list or the next frame.
        Ok(OpenedChat {
            id,
            state: handle.state(),
            mcp: handle.mcp(),
            lsp: handle.lsp(),
        })
    }

    /// Closes a chat: the turn stops, the session is released, and background
    /// commands die with it (§6.5). The §9 journal stays — closing is not deleting,
    /// and a closed chat opens again with its history.
    pub async fn close_chat(&self, session: &str) -> Result<(), String> {
        let id = SessionId::from(session.to_owned());
        self.release(&id).await;
        Ok(())
    }

    /// Takes a chat out of the table and stops its turn, if it was open.
    pub(crate) async fn release(&self, id: &SessionId) {
        self.shell.chat_closed(id);
        let Some(handle) = self.state.close(id) else {
            return;
        };
        // Cancel before the handle is released: otherwise the started turn would
        // finish its work and only then notice that nobody is waiting for it.
        let _ = handle.send(Command::Cancel).await;
    }

    /// Hands the chats that are still running back to a window that lost them.
    ///
    /// The webview reloads — Ctrl+R, a hot reload on every edit of the frontend,
    /// a phone that lost its connection — and the sessions do not: the turn goes
    /// on, the commands go on, a question waits for its answer. Stopping them
    /// because nobody could see them was the wrong trade: it threw away work to
    /// avoid showing it. The window calls this on start, before it opens anything,
    /// and gets each chat as a snapshot plus a bridge that carries everything
    /// after it (`AppState::rebind_all`).
    pub async fn attach_chats(&self) -> Result<Vec<AttachedChat>, String> {
        let mut attached = Vec::new();
        for chat in self.state.rebind_all(&self.shell) {
            // What the window was watching went with it. Left set, the core would go
            // on sending the output of a row nobody has open.
            let _ = chat.handle.send(Command::WatchProcess { id: None }).await;

            // The journal is read from disk, and a long chat is a long file.
            let handle = chat.handle.clone();
            let replay = chat.replay;
            let events = tokio::task::spawn_blocking(move || handle.replay(&replay))
                .await
                .map_err(|e| e.to_string())?;

            attached.push(AttachedChat {
                id: chat.handle.id().clone(),
                project: chat.project,
                epoch: chat.epoch,
                events: bridge::coalesce(events),
            });
        }
        Ok(attached)
    }

    pub async fn send_command(&self, session: &str, command: Command) -> Result<(), String> {
        self.chat(session)?
            .send(command)
            .await
            .map_err(|e| e.to_string())
    }

    pub fn session_state(&self, session: &str) -> Option<SessionState> {
        self.chat(session).ok().map(|chat| chat.state())
    }

    /// A chat's row in the §9 list — the title for the header (§3) and the cost.
    pub fn session_meta(&self, session: &str) -> Option<yaca_core::store::SessionMeta> {
        self.chat(session).ok().and_then(|chat| chat.meta())
    }

    /// The name for the §9.1 greeting, taken from the operating system.
    ///
    /// Yaca has no accounts, and asking for a name for the sake of one line is a
    /// poor price for it. If none is found, the greeting goes without a name.
    pub fn user_name(&self) -> Option<String> {
        let var = if cfg!(windows) { "USERNAME" } else { "USER" };
        std::env::var(var)
            .ok()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
    }
}

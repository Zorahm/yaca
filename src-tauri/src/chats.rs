//! Opening and closing chats: the one way a chat enters the shell's table, whether
//! the window asked for it or the phone did, and the notice that the table changed.
//!
//! The window's commands and the remote host both call these functions, so a chat
//! started from either is set up the same way: same config, same browser, same
//! bridge to the window.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use yaca_core::id::SessionId;
use yaca_core::provider::registry::Providers;
use yaca_core::router::ROUTER_ID;
use yaca_core::{Command, SessionConfig, SessionHandle};

use crate::browser;
use crate::config_dir;
use crate::project_path;
use crate::settings::AppSettings;
use crate::startup::Startup;
use crate::state::AppState;

/// Event the window hears when a chat is opened or closed. Its payload is nothing:
/// the window reads the list again with the commands it already has.
pub const CHATS_CHANGED: &str = "chats-changed";

/// Opens a chat in the project: a new one or a recorded one (§9), and puts it in
/// the table so the window's bridge carries its events.
///
/// A `resume` of a chat that is already running is not checked here. The window
/// keeps its own list of the chats it opened and checks that first, and the
/// remote host checks the table before it calls this (`remote::host`); a check
/// here would change what the window gets back.
pub(crate) async fn open_chat(
    app: &AppHandle,
    path: String,
    resume: Option<String>,
    startup: Option<Startup>,
) -> Result<Arc<SessionHandle>, String> {
    let state = app.state::<AppState>();
    let root = project_path(&path)?;
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;

    // The project's settings are the base; what is chosen in the window goes on top of them (§10).
    let mut config = SessionConfig::open(root.as_path()).map_err(|e| e.to_string())?;
    // This one is an application-level setting and is not in the project's
    // file: it reaches the core as a config field, like the mode and the effort.
    config.respect_robots = state.prefs().respect_robots;
    config.summarize_web_fetch = state.prefs().summarize_web_fetch;
    config.web_fetch_model = yaca_core::router::attach::web_fetch_setup(&config_dir()?, &providers);
    config.web_fetch_pricing = config
        .web_fetch_model
        .as_ref()
        .and_then(|setup| yaca_core::router::attach::setup_pricing(&providers, setup));
    config.shell = state.prefs().shell;
    // The human's own MCP servers and their approvals of a project's: both
    // live in the configuration directory, like the providers.
    let config_home = config_dir()?;
    config.mcp_servers =
        yaca_core::mcp::config::load_global(&config_home).map_err(|e| e.to_string())?;
    // The human's own language servers, from the same directory.
    config.lsp_registry = Some(state.lsps(&config_home));
    config.user_skills = yaca_core::skills::user_dirs(&config_home);
    config.user_agents = yaca_core::agents::user_dirs(&config_home);
    config.plugins = yaca_core::plugin::PluginSources::standard(&config_home)
        .load()
        .enabled();
    config.mcp_approvals = Some(config_home);
    config.journal = Some(yaca_core::store::chats_dir().map_err(|e| e.to_string())?);
    let settings = AppSettings::load(&config_dir()?);
    config.additional_roots = crate::project_entries::session_folders(
        &settings,
        &root,
        resume.as_deref(),
        &crate::store(&path)?,
    );
    if let Some(id) = resume {
        config.resume_session(SessionId::from(id));
    }

    if let Some(startup) = startup {
        config.mode = startup.mode;
        config.profile = startup.profile;
        config.choose_model(startup.provider, startup.model);
        config.effort = startup.effort;
    }

    // The small model the `Decision` tool asks, if there is one.
    config.decision = yaca_core::router::attach::decision_setup(&config_dir()?, &providers);
    config.decision_pricing = config
        .decision
        .as_ref()
        .and_then(|setup| yaca_core::router::attach::setup_pricing(&providers, setup));

    // The chat's browser, offered to its session as an MCP server of the
    // window's own. The page itself is made on first use.
    let browser = app.state::<browser::Browsers>().prepare(app);
    if let Some((_, server)) = &browser {
        config.app_servers.push(server.clone());
    }

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
    if root != std::fs::canonicalize(dir.join("conversations")).unwrap_or_default() {
        settings.remember_project(root);
        settings.save(&dir).map_err(|e| e.to_string())?;
    }

    let id = handle.id().clone();
    if let Some((browser, _)) = browser {
        app.state::<browser::Browsers>().bind(id.clone(), browser);
    }
    state.open(handle.clone(), path, app);
    Ok(handle)
}

/// Closes a chat: the turn stops, the session is released, and background
/// commands die with it (§6.5). The §9 journal stays — closing is not deleting,
/// and a closed chat opens again with its history.
pub(crate) async fn close(state: &AppState, browsers: &browser::Browsers, id: &SessionId) {
    browsers.forget(id);
    let Some(handle) = state.close(id) else {
        return;
    };
    // Cancel before the handle is released: otherwise the started turn would
    // finish its work and only then notice that nobody is waiting for it.
    let _ = handle.send(Command::Cancel).await;
}

/// Tells the window whenever the table of chats changes, so a chat the phone
/// opened or closed shows up in the window without a reload.
///
/// Known gap: the window does not listen to [`CHATS_CHANGED`] yet, so a chat the
/// phone opened is not in its list. Resuming that chat from the sidebar then
/// opens a second session on the same journal, because the window's guard in
/// `src/ui/chats.ts` only knows the chats the window opened itself. Listening
/// here (re-reading the chats) closes it; until then the window also does not
/// show the chat as running.
pub fn watch(app: AppHandle) {
    let mut changes = app.state::<AppState>().chats_changed();
    tauri::async_runtime::spawn(async move {
        while changes.changed().await.is_ok() {
            let _ = app.emit(CHATS_CHANGED, ());
        }
    });
}

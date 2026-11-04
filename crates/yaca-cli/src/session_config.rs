//! The configuration a chat starts with, as the CLI builds it.
//!
//! A one-shot run and `yaca serve` both start chats here, so a chat started from
//! a phone and one started from a terminal get the same project settings, the
//! same servers and skills, and the same provider rules. The window builds its
//! own in `src-tauri`, in the same order. The order below matters: a recorded
//! chat's model is taken before the caller's choice is laid over it, and the
//! journal has to exist before a chat can be resumed from it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use yaca_core::id::SessionId;
use yaca_core::provider::registry::{self, Providers};
use yaca_core::provider::{self, Provider};
use yaca_core::router::{self, ROUTER_ID};
use yaca_core::secrets;
use yaca_core::{Effort, Mode, Profile, SessionConfig};

/// What the caller chooses on top of the project's settings. A field left at its
/// empty value leaves the project, or the recorded chat, in charge.
pub struct Choice {
    /// Provider and model, replaced together. Both empty: the project's
    /// settings decide, or the model of a recorded chat when `resume` is set.
    pub provider: String,
    pub model: String,
    /// `None` keeps the project's mode.
    pub mode: Option<Mode>,
    /// `None` keeps the project's profile: code, or documents.
    pub profile: Option<Profile>,
    /// `None` keeps the project's effort.
    pub effort: Option<Effort>,
    /// A recorded chat to continue. Needs `journal`, which is set for it.
    pub resume: Option<SessionId>,
    /// Record the chat in the chats folder the window reads. Off, nothing is kept.
    pub journal: bool,
    pub isolate: bool,
    pub additional_roots: Vec<PathBuf>,
}

/// The config and the provider a chat in `project` starts with. The error is
/// English text for the person: an unknown provider, a setting that cannot be read.
pub fn session_config(
    project: &Path,
    choose: Choice,
) -> Result<(SessionConfig, Arc<dyn Provider>)> {
    // The same `providers.json` the window reads: a gateway configured there
    // must answer from the CLI too, or the core cannot be run without the
    // interface.
    let config_dir = registry::config_dir()?;
    let providers = Providers::load(&config_dir)?;
    let mut config = SessionConfig::open(project)?;
    // The human's own MCP servers, from the same directory and for the same
    // reason as the providers: a server set up in the window must be there
    // from the CLI too.
    config.mcp_servers = yaca_core::mcp::config::load_global(&config_dir)?;
    config.lsp_registry = Some(std::sync::Arc::new(yaca_core::lsp::runtime::Registry::new(
        config_dir.clone(),
    )));
    config.user_skills = yaca_core::skills::user_dirs(&config_dir);
    // The office skills Yaca ships with, into the human's own folder. A
    // folder that cannot be written is not a reason to refuse the chat: the
    // session works, only without the Word, Excel and PowerPoint guides.
    if let Some(skills) = config.user_skills.first() {
        if let Err(why) = yaca_core::skills::bundled::install(skills) {
            eprintln!("warning: {why}");
        }
    }
    config.user_agents = yaca_core::agents::user_dirs(&config_dir);
    config.plugins = yaca_core::plugin::PluginSources::standard(&config_dir)
        .load()
        .enabled();
    config.mcp_approvals = Some(config_dir.clone());
    // The window's chats folder, so that a session recorded here is listed
    // there, and one held there can be resumed here.
    if choose.journal || choose.resume.is_some() {
        config.journal = Some(yaca_core::store::chats_dir()?);
    }
    if let Some(id) = choose.resume {
        config.resume_session(id);
    }
    config.isolate = choose.isolate;
    config.additional_roots = choose.additional_roots;
    // Flags over the project's settings and the resumed session, the pair
    // replaced together — the same rule the window applies, from the same place.
    config.choose_model(choose.provider, choose.model);
    if let Some(mode) = choose.mode {
        config.mode = mode;
    }
    if let Some(profile) = choose.profile {
        config.profile = profile;
    }
    if let Some(effort) = choose.effort {
        config.effort = effort;
    }
    // The small model the `Decision` tool asks, when the Decision provider or
    // the router has one: the same rule the window applies, from the same place.
    config.decision = router::attach::decision_setup(&config_dir, &providers);
    config.decision_pricing = config
        .decision
        .as_ref()
        .and_then(|setup| router::attach::setup_pricing(&providers, setup));
    let provider = if config.provider == ROUTER_ID {
        router::attach::attach(&config_dir, &providers, &mut config)?
    } else {
        resolve_provider(&providers, &mut config)?
    };
    Ok((config, provider))
}

/// Chooses the provider from `providers.json` and sets the provider, model and
/// model list on `config` to match. The key is read here, because only the
/// provider knows whether it needs one.
fn resolve_provider(
    providers: &Providers,
    config: &mut SessionConfig,
) -> Result<Arc<dyn Provider>> {
    let (spec, model) = providers.resolve(&config.provider, &config.model)?;
    let spec = spec.clone();

    // Keychain first, environment second (§10). No key is not an error: the
    // request goes without it, and a server that wants one refuses with a
    // message naming the key. The source is reported on stderr, where it
    // cannot be mistaken for part of the `--jsonl` stream.
    let key = spec
        .needs_key()
        .then(|| secrets::api_key(&spec.id))
        .and_then(|(key, source)| {
            tracing::debug!(?source, provider = %spec.id, "provider key");
            key
        });

    config.provider = spec.id.clone();
    config.model = model;
    config.models = spec.models.clone();
    Ok(provider::build_with_key(spec, key))
}

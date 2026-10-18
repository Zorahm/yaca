//! Putting the router in a session's place, for the CLI and the window alike.
//!
//! One rule in one place: the router is not an entry of `providers.json`, it is
//! made of them, so a session that picks it has to be handed the router itself
//! and a model list that also prices the real models behind it. Two shells
//! doing this by hand would price a routed request differently in one of them.

use std::path::Path;
use std::sync::Arc;

use crate::error::CoreError;
use crate::provider::registry::{ProviderSpec, Providers, DECISION_ID};
use crate::provider::{self, Provider};
use crate::session::SessionConfig;
use crate::tool::decision::DecisionSetup;
use crate::usage::ModelInfo;

use super::file;
use super::provider::RouterProvider;
use super::ROUTER_ID;

/// What `routing.json` amounts to right now.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No file: routing is not configured, and nothing is offered.
    Absent,
    /// The router that would run, as the model switcher lists it.
    Ready(ModelInfo),
    /// The file is there and unusable. Kept apart from `Absent` because a
    /// router that quietly disappears from the switcher is a typo nobody finds.
    Broken(String),
}

/// Reads and checks `routing.json` against the providers, without starting
/// anything: no key is fetched from the keychain, since the model list asks
/// this on every refresh and a prompt per refresh would be a bug of its own.
pub fn status(config_dir: &Path, providers: &Providers) -> Status {
    let cfg = match file::load(config_dir) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return Status::Absent,
        Err(e) => return Status::Broken(e.to_string()),
    };
    // Backends are built without keys: only the validation and the virtual
    // model are wanted here, and neither sends anything.
    match RouterProvider::new(cfg, providers, |spec| {
        provider::build_with_key(spec.clone(), None)
    }) {
        Ok((_, offered)) => Status::Ready(offered),
        Err(e) => Status::Broken(e.to_string()),
    }
}

/// Makes `config` a session on the router and returns the provider to spawn it
/// with. The session is offered the virtual model and the real models behind
/// it, so each request is priced by the one that answered.
pub fn attach(
    config_dir: &Path,
    providers: &Providers,
    config: &mut SessionConfig,
) -> Result<Arc<dyn Provider>, CoreError> {
    let cfg = file::load(config_dir)?.ok_or_else(|| {
        CoreError::Config(format!(
            "маршрутизатор выбран, но routing.json не найден: создайте его в {} рядом с providers.json",
            config_dir.display()
        ))
    })?;
    let (router, offered) = RouterProvider::from_providers(cfg, providers)?;
    config.provider = ROUTER_ID.into();
    config.model = offered.id.clone();
    config.models = std::iter::once(offered).chain(router.targets()).collect();
    Ok(Arc::new(router))
}

/// The model the `Decision` tool asks, when there is one to ask.
///
/// A model chosen for the router wins: whoever set the router up already
/// picked the small model there, and asking a different one in the same
/// session would be two answers to "which is the small model". Without a
/// router, the first model the Decision provider answered with. `None` when
/// neither exists, and then the tool is not offered at all.
pub fn decision_setup(config_dir: &Path, providers: &Providers) -> Option<DecisionSetup> {
    let mut setup =
        decision_setup_with(config_dir, providers, |spec| provider::build(spec.clone()))?;
    // The typed-decision route of the same provider, with its key. Here and
    // not in `decision_setup_with`: the key comes from the keychain, which the
    // tests do not reach for.
    let spec = chosen_spec(config_dir, providers)?;
    setup.systemone = Some(provider::build_systemone(spec.clone()));
    Some(setup)
}

/// [`decision_setup`] with the backend built by the caller: the tests do not
/// reach for the keychain.
pub fn decision_setup_with(
    config_dir: &Path,
    providers: &Providers,
    build: impl Fn(&ProviderSpec) -> Arc<dyn Provider>,
) -> Option<DecisionSetup> {
    let (spec, model) = chosen(config_dir, providers)?;
    Some(DecisionSetup {
        provider: build(spec),
        model,
        systemone: None,
    })
}

fn chosen_spec<'a>(config_dir: &Path, providers: &'a Providers) -> Option<&'a ProviderSpec> {
    chosen(config_dir, providers).map(|(spec, _)| spec)
}

/// The provider and model that answer: the router's small model, else the
/// Decision provider's first.
fn chosen<'a>(config_dir: &Path, providers: &'a Providers) -> Option<(&'a ProviderSpec, String)> {
    // A broken routing.json is not this function's to report — the router's
    // own status says so — and it does not take the tool down with it.
    let chosen = file::load(config_dir).ok().flatten().and_then(|cfg| {
        let wanted = cfg.decision_model.model;
        let spec = providers.get(&wanted.provider)?;
        spec.model(&wanted.model)?;
        Some((spec, wanted.model))
    });
    chosen.or_else(|| {
        let spec = providers.get(DECISION_ID)?;
        let first = spec.models.first()?;
        Some((spec, first.id.clone()))
    })
}

#[cfg(test)]
mod tests;

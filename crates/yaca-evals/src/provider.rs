//! Building the real provider the way `yaca-cli` does, so an eval measures the
//! same provider, model and key the human's own runs would use.
//!
//! Only real providers come out of here. There is no fallback to a canned
//! answer: a run that cannot reach a model exits with the reason, instead of
//! producing numbers that look like results.

use std::sync::Arc;

use anyhow::{bail, Result};
use yaca_core::provider::registry::{self, Providers};
use yaca_core::provider::{self, Provider};
use yaca_core::secrets;
use yaca_core::usage::ModelInfo;

/// Everything a session needs to talk to one provider and model.
pub struct Plan {
    pub provider: Arc<dyn Provider>,
    pub provider_id: String,
    pub model: String,
    /// The provider's model list. It drives the context window and the price,
    /// so a cost in the result means something only with it.
    pub models: Vec<ModelInfo>,
}

/// The providers the window and the CLI configure, from the same file.
pub fn load_providers() -> Result<Providers> {
    Ok(Providers::load(&registry::config_dir()?)?)
}

/// Resolves `provider` and `model` (either may be empty, meaning "the
/// provider decides", as in `yaca`) and builds the provider with its key.
///
/// The key comes from the keychain first and the environment second, and a
/// provider that needs one and has none is refused here, before a request is
/// sent that would only come back as an authentication error.
pub fn real_plan(providers: &Providers, provider: &str, model: &str) -> Result<Plan> {
    let (spec, model) = providers.resolve(provider, model)?;
    let spec = spec.clone();

    let key = if spec.needs_key() {
        match secrets::api_key(&spec.id).0 {
            Some(key) => Some(key),
            None => bail!(
                "no API key for provider `{}`: save it in the app, or set {}",
                spec.id,
                secrets::env_var(&spec.id)
            ),
        }
    } else {
        None
    };

    Ok(Plan {
        provider: provider::build_with_key(spec.clone(), key),
        provider_id: spec.id.clone(),
        model,
        models: spec.models.clone(),
    })
}

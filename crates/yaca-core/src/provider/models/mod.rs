//! Asking a provider what it serves (§10).
//!
//! `providers.json` used to carry six hand-written numbers per model, and every
//! one of them was a guess that goes stale: a context window that grew, an
//! output cap that shrank, an `effort` a new model learned. Both wire shapes
//! answer `GET /v1/models`, so the numbers can be asked for instead.
//!
//! **What is asked for is what makes a model run.** Whether a model accepts
//! `output_config.effort` and adaptive thinking decides between an answer and a
//! 400, so those two are read carefully. The context window only moves a
//! number in the ring §7.1, so a provider that keeps quiet about it gets a
//! conservative default rather than a refusal.
//!
//! Prices are not asked for because nobody serves them. Anthropic's Models API
//! has no price field at all, and the price list behind
//! [`unknown_model`](crate::usage::unknown_model) fills that in by model id; an
//! OpenAI-form gateway that *does* report prices — OpenRouter — is read for
//! them.
//!
//! Nothing here runs on its own. §7.1 reads the model list on every render, and
//! a list that costs a round trip would put the network in front of the window
//! opening. Discovery is an action: a button in the panel §10, and the moment a
//! key is saved — which is the moment there is finally something to ask with.
//! That second one is [`refresh`], and it is the core's to do rather than the
//! window's: `yaca key set` stores a key too, and a provider that is configured
//! from one front end and not from the other is two versions of the truth.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use super::http;
use super::registry::{Protocol, ProviderSpec, Providers};
use crate::error::{CoreError, ProviderError};
use crate::secrets;
use crate::usage::ModelInfo;

mod anthropic;
mod local;
mod openai;

use anthropic::anthropic;
use openai::openai;

/// Every model the provider says it serves.
///
/// The list is returned in the provider's own order: Anthropic answers newest
/// first, which is the order §7.1 should offer them in, and inventing a sort
/// would bury today's model under a decade of ids.
pub async fn discover(
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let client = reqwest::Client::new();
    let mut models = match spec.protocol {
        Protocol::Anthropic => anthropic(&client, spec, key).await?,
        Protocol::OpenAi => openai(&client, spec, key).await?,
    };
    // Ollama and LM Studio say nothing about context on the OpenAI shape, and
    // their own endpoints do; the probe only fires for a loopback address.
    // Auto-compaction reads the same number (§5.11), so whatever is here is
    // what it compacts by.
    local::probe(spec, key, &mut models).await;
    for m in &mut models {
        m.provider_id = spec.id.clone();
    }
    Ok(models)
}

/// Asks a saved provider for its models and writes the answer into
/// `providers.json` in `dir`.
///
/// A refusal leaves the file as it was: the list that was there is what the
/// provider last said, and an error is not a statement that it serves nothing.
pub async fn refresh(dir: &Path, id: &str) -> Result<Providers, CoreError> {
    let missing = || CoreError::Config(format!("provider «{id}» is not in the list"));

    let spec = Providers::load(dir)?.get(id).cloned().ok_or_else(missing)?;
    let models = discover(&spec, stored_key(&spec).as_deref()).await?;

    // Read again rather than kept from above: the question took a round trip,
    // and whatever was saved meanwhile must not be written over with a copy
    // from before it.
    let mut providers = Providers::load(dir)?;
    providers
        .list
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(missing)?
        .models = models;
    providers.save(dir)?;
    Ok(providers)
}

/// The key a request to this provider goes with: none for one that does not
/// ask for it, so a local gateway never touches the keychain.
pub fn stored_key(spec: &ProviderSpec) -> Option<String> {
    spec.needs_key()
        .then(|| secrets::api_key(&spec.id).0)
        .flatten()
}

/// Which of the provider's models are in memory right now, by id (§7.1).
///
/// Only a server that says gets an answer: a local one reports `loaded` per
/// model, and picking one that is not loaded costs a long load or an error.
/// The field is not in the OpenAI shape, so anyone else answers with an
/// empty map, and so does Anthropic, which serves every model it lists.
///
/// Kept out of [`ModelInfo`] on purpose. That is saved to `providers.json`,
/// and residency is live state: stale the moment it is written down. So it is
/// asked for when the model menu opens, and never stored.
pub async fn residency(
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<HashMap<String, bool>, ProviderError> {
    match spec.protocol {
        Protocol::Anthropic => Ok(HashMap::new()),
        Protocol::OpenAi => openai::residency(&reqwest::Client::new(), spec, key).await,
    }
}

/// A `GET` of a protocol path ([`ProviderSpec::api`]), asking for JSON rather
/// than a stream.
///
/// The protocol's own `accept: text/event-stream` is for `/v1/messages`; a
/// gateway that takes it literally here answers 406.
async fn get(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    path: &str,
) -> Result<Value, ProviderError> {
    let mut headers = http::header_map(spec, key)?;
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let resp = client
        .get(spec.api(path))
        .headers(headers)
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let resp = http::check_status(resp, spec, key).await?;
    let text = resp
        .text()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    serde_json::from_str(&text).map_err(|e| {
        ProviderError::Config(format!(
            "{} answered with JSON that the model list does not expect: {e}",
            spec.api(path)
        ))
    })
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ProviderError> {
    serde_json::from_value(value)
        .map_err(|e| ProviderError::Config(format!("the model list could not be parsed: {e}")))
}

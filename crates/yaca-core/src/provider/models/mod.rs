//! Asking a provider what it serves (§10).
//!
//! `providers.json` used to carry six hand-written numbers per model, and every
//! one of them was a guess that goes stale: a context window that grew, an
//! output cap that shrank, an `effort` a new model learned. Most endpoints
//! answer `GET /v1/models`; DeepSeek publishes a shared `GET /models` catalog.
//!
//! **What is asked for is what makes a model run.** Whether a model accepts
//! `output_config.effort` and adaptive thinking decides between an answer and a
//! 400, so those two are read carefully. The context window controls the
//! ring and automatic compaction, so a provider that keeps quiet about it gets a
//! million-token default rather than a refusal.
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
use super::registry::{Protocol, ProviderPoll, ProviderSpec, Providers};
use crate::error::{CoreError, ProviderError};
use crate::secrets;
use crate::usage::{ContextSource, ModelFacts, ModelInfo};

mod anthropic;
mod catalog;
mod deepseek;
mod local;
mod models_dev;
mod openai;
mod price;
mod reasoning;

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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    let mut models = if let Some(url) = deepseek::catalog_url(spec) {
        deepseek::discover(&client, spec, key, url).await?
    } else {
        match spec.protocol {
            Protocol::Anthropic => anthropic(&client, spec, key).await?,
            Protocol::OpenAi => openai(&client, spec, key).await?,
        }
    };
    // Ollama and LM Studio say nothing about context on the OpenAI shape, and
    // their own endpoints do; the probe only fires for a loopback address.
    // Auto-compaction reads the same number (§5.11), so whatever is here is
    // what it compacts by.
    local::probe(spec, key, &mut models).await;
    // The local probe runs after the wire parser captured its facts. Preserve
    // the detected allocation before an external catalog fills the gaps.
    for model in &mut models {
        if model.context_source != ContextSource::DefaultGuess {
            if let Some(facts) = &mut model.fetched {
                facts.context_max = Some(model.context_max);
            }
        }
    }
    // What the provider left unsaid, from a public catalog: a gateway of one's
    // own answers ids and nothing else, and the panel has fields for the rest.
    catalog::enrich(&client, spec, &mut models).await;
    let large_list = models.len() > 30;
    for (index, m) in models.iter_mut().enumerate() {
        m.provider_id = spec.id.clone();
        if large_list {
            m.enabled = index < 8;
        }
    }
    keep_hand_edits(&spec.models, &mut models);
    Ok(models)
}

/// Merges a fresh answer into the rows saved before it (§10).
///
/// Three states per number, and they must not be confused:
///
/// - `fetched` — what the provider (or a public catalog) answered just now;
/// - `overrides` — what a person set by hand;
/// - the numbers the session runs on: `override ?? fetched ?? default`.
///
/// A refresh replaces the first and leaves the second alone: an edit somebody
/// made on purpose is not undone by the provider answering again. That was the
/// bug the panel kept reporting — a maximum output or a capability typed in by
/// hand was back to the provider's word after the next poll, because nothing
/// recorded that it was an edit.
///
/// A row added by hand is kept even when the address never lists it, and stays
/// marked as such.
pub fn keep_hand_edits(saved: &[ModelInfo], models: &mut Vec<ModelInfo>) {
    for model in models.iter_mut() {
        model.fetched = Some(ModelFacts::of(model));
        if let Some(previous) = saved.iter().find(|p| p.id == model.id) {
            model.overrides = hand_edits(previous);
            model.enabled = previous.enabled;
            model.by_hand = previous.by_hand;
            if previous.by_hand {
                model.display_name = previous.display_name.clone();
                model.supported_parameters = previous.supported_parameters.clone();
                model.input_modalities = previous.input_modalities.clone();
                model.supports_images = previous.supports_images;
                model.supports_thinking = previous.supports_thinking;
            }
        }
        model.apply_overrides();
    }
    for previous in saved {
        if previous.by_hand && !models.iter().any(|m| m.id == previous.id) {
            models.push(previous.clone());
        }
    }
}

/// The hand edits of a saved row.
///
/// Includes the ones written before `overrides` existed: `context_source: hand`
/// and `pricing_by_hand` were the only marks of an edit then, and the number
/// beside each mark is the edit itself.
fn hand_edits(previous: &ModelInfo) -> Option<ModelFacts> {
    let mut facts = previous.overrides.clone().unwrap_or_default();
    if facts.context_max.is_none() && previous.context_source == ContextSource::Hand {
        facts.context_max = Some(previous.context_max);
    }
    if facts.pricing.is_none()
        && facts.input_price.is_none()
        && facts.output_price.is_none()
        && previous.pricing_by_hand
    {
        facts.pricing = Some(previous.pricing);
    }
    (!facts.is_empty()).then_some(facts)
}

/// Asks a saved provider for its models and writes the answer into
/// `providers.json` in `dir`.
///
/// A refusal keeps the previous models and records the failed lookup separately.
pub async fn refresh(dir: &Path, id: &str) -> Result<Providers, CoreError> {
    let missing = || CoreError::Config(format!("provider «{id}» is not in the list"));

    let spec = Providers::load(dir)?.get(id).cloned().ok_or_else(missing)?;
    let answer = discover(&spec, stored_key(&spec).as_deref()).await;

    // Read again rather than kept from above: the question took a round trip,
    // and whatever was saved meanwhile must not be written over with a copy
    // from before it.
    let mut providers = Providers::load(dir)?;
    let current = providers
        .list
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(missing)?;
    // An in-flight response for an old endpoint cannot describe a newly edited one.
    if current.base_url != spec.base_url
        || current.protocol != spec.protocol
        || current.headers != spec.headers
    {
        return Err(CoreError::Config(
            "provider changed during model lookup; retry".into(),
        ));
    }
    let poll = ProviderPoll {
        checked_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        error: answer.as_ref().err().map(ToString::to_string),
    };
    let result = match answer {
        Ok(mut models) => {
            keep_hand_edits(&current.models, &mut models);
            current.models = models;
            Ok(())
        }
        Err(error) => Err(CoreError::from(error)),
    };
    providers.polls.insert(id.to_string(), poll);
    providers.save(dir)?;
    result?;
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
        Protocol::OpenAi if deepseek::catalog_url(spec).is_some() => Ok(HashMap::new()),
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
    let url = spec.api(path);
    get_at(client, spec, key, &url, http::header_map(spec, key)?).await
}

async fn get_at(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    url: &str,
    mut headers: reqwest::header::HeaderMap,
) -> Result<Value, ProviderError> {
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    let resp = client
        .get(url)
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
            "{url} answered with JSON that the model list does not expect: {e}"
        ))
    })
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ProviderError> {
    serde_json::from_value(value)
        .map_err(|e| ProviderError::Config(format!("the model list could not be parsed: {e}")))
}

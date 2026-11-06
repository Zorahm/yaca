//! The OpenAI form's `/v1/models`: bare in the specification, and whatever
//! each gateway chose to add on top.

use std::collections::HashMap;

use serde::Deserialize;

use super::{get, parse};
use crate::error::ProviderError;
use crate::provider::registry::ProviderSpec;
use crate::usage::{unknown_model, ContextSource, ModelFacts, ModelInfo, Pricing};

/// Chat Completions' `/v1/models`.
///
/// The specified shape carries only `id`, and that is all Ollama, LM Studio and
/// OpenAI itself answer with. Gateways add to it — vLLM reports
/// `max_model_len`, OpenRouter reports a name, a context length, prices and the
/// parameters it accepts — and everything they add is read, because a number
/// the provider stated beats a default Yaca picked.
pub(super) async fn openai(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    // Catalogs enrich model facts after discovery; they cannot establish that
    // this endpoint and these credentials actually work.
    let page: OpenAiPage = parse(get(client, spec, key, "/models").await?)?;
    Ok(openai_models(&page))
}

fn openai_models(page: &OpenAiPage) -> Vec<ModelInfo> {
    let mut out: Vec<ModelInfo> = page.data.iter().map(openai_model).collect();
    // This shape's order means nothing — `created`, if anything — while §7.1
    // shows the list as it comes. By id at least it is findable.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The same list, asked for what is in memory rather than for what exists.
pub(super) async fn residency(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<HashMap<String, bool>, ProviderError> {
    let page: OpenAiPage = parse(get(client, spec, key, "/models").await?)?;
    Ok(residency_of(&page))
}

/// Only the models the server said something about: silence claims nothing.
fn residency_of(page: &OpenAiPage) -> HashMap<String, bool> {
    page.data
        .iter()
        .filter_map(|m| Some((m.id.clone(), m.loaded?)))
        .collect()
}

#[derive(Debug, Deserialize)]
struct OpenAiPage {
    #[serde(default)]
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
    /// OpenRouter says `name`, a local Unsloth studio says `display_name` —
    /// the same field under two names, and the Anthropic shape above uses the
    /// second. Without the alias the switcher (§7.1) shows a raw id like
    /// `unsloth/Qwen3.5-0.8B-GGUF` for a model whose name the server sent.
    #[serde(default, alias = "display_name")]
    name: Option<String>,
    /// OpenRouter.
    #[serde(default)]
    context_length: Option<u32>,
    /// vLLM.
    #[serde(default)]
    max_model_len: Option<u32>,
    #[serde(default)]
    top_provider: Option<OpenAiTopProvider>,
    #[serde(default)]
    pricing: Option<OpenAiPricing>,
    /// OpenRouter: what the model actually accepts.
    #[serde(default)]
    supported_parameters: Option<Vec<String>>,
    /// OpenRouter: `input_modalities` under it says whether the model sees.
    #[serde(default)]
    architecture: Option<serde_json::Value>,
    /// A local server (Unsloth studio): is the model in memory right now.
    /// Live state, so it is read by [`residency`] and never into `ModelInfo`.
    #[serde(default)]
    loaded: Option<bool>,
    #[serde(default)]
    expiration_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiTopProvider {
    #[serde(default)]
    max_completion_tokens: Option<u32>,
    #[serde(default)]
    context_length: Option<u32>,
}

/// OpenRouter reports dollars per *token*, as strings.
#[derive(Debug, Deserialize)]
struct OpenAiPricing {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    completion: Option<String>,
    #[serde(default)]
    input_cache_read: Option<String>,
    #[serde(default)]
    input_cache_write: Option<String>,
}

fn openai_model(m: &OpenAiModel) -> ModelInfo {
    let mut info = unknown_model(&m.id);
    if let Some(name) = &m.name {
        info.display_name = name.clone();
    }
    if let Some(context) = m
        .context_length
        .or(m.max_model_len)
        .or_else(|| m.top_provider.as_ref()?.context_length)
    {
        info.context_max = context;
        // Stated, not guessed: the probe (see `super::local`) reads this tag
        // first and skips models the list already spoke for.
        info.context_source = ContextSource::ProviderReported;
    }
    if let Some(output) = m
        .top_provider
        .as_ref()
        .and_then(|t| t.max_completion_tokens)
    {
        info.max_output = output;
    }
    if let Some(pricing) = m.pricing.as_ref().and_then(per_million) {
        info.pricing = pricing;
    }
    // An absent list means the gateway said nothing, not that the model cannot
    // reason: the bare shape reports no parameters at all. Silence leaves
    // `effort` off, because a `reasoning_effort` nobody asked for is a 400.
    if let Some(params) = &m.supported_parameters {
        info.supports_effort = params
            .iter()
            .any(|p| p == "reasoning" || p == "reasoning_effort");
    }
    info.reasoning = m.reasoning.as_ref().and_then(super::reasoning::openrouter);
    if let Some(levels) = info
        .reasoning
        .as_ref()
        .and_then(|caps| caps.supported_efforts.as_ref())
    {
        info.supports_effort = !levels.is_empty();
    }
    // Thinking blocks are never sent back in this shape (see `super::openai`),
    // so there is no point asking any model here for them.
    info.supports_thinking = false;
    // OpenRouter lists what a model takes in. A list without "image" is a
    // stated no; no list at all — the bare shape — says nothing, and nothing
    // leaves images on (see `ModelInfo::supports_images`).
    if let Some(inputs) = m
        .architecture
        .as_ref()
        .and_then(|a| a["input_modalities"].as_array())
    {
        info.supports_images = inputs.iter().any(|i| i == "image");
        info.input_modalities = Some(
            inputs
                .iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect(),
        );
    }
    info.supported_parameters = m.supported_parameters.clone();
    info.expiration_date = m.expiration_date.clone();
    info.cache_read = m
        .pricing
        .as_ref()
        .and_then(|p| super::price::per_million(p.input_cache_read.as_deref()));
    info.cache_write = m
        .pricing
        .as_ref()
        .and_then(|p| super::price::per_million(p.input_cache_write.as_deref()));
    let mut facts = ModelFacts::of(&info);
    facts.max_output = m
        .top_provider
        .as_ref()
        .and_then(|p| p.max_completion_tokens);
    facts.pricing = m.pricing.as_ref().and_then(per_million).or(facts.pricing);
    facts.input_price = m
        .pricing
        .as_ref()
        .and_then(|p| super::price::per_million(p.prompt.as_deref()));
    facts.output_price = m
        .pricing
        .as_ref()
        .and_then(|p| super::price::per_million(p.completion.as_deref()));
    facts.supports_effort = m
        .supported_parameters
        .as_ref()
        .map(|_| info.supports_effort);
    if info
        .reasoning
        .as_ref()
        .is_some_and(|caps| caps.supported_efforts.is_some())
    {
        facts.supports_effort = Some(info.supports_effort);
    }
    info.fetched = Some(facts);
    info
}

/// Dollars per million, from dollars per token.
fn per_million(pricing: &OpenAiPricing) -> Option<Pricing> {
    Some(Pricing {
        input: super::price::per_million(pricing.prompt.as_deref())?,
        output: super::price::per_million(pricing.completion.as_deref())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openai_page(json: &str) -> Vec<ModelInfo> {
        openai_models(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn reasoning_metadata_keeps_model_specific_levels_and_defaults() {
        let models = openai_page(
            r#"{"data":[{"id":"models/gemini-3.8-flash",
            "reasoning":{"supported_efforts":["high","medium","low"],
            "default_effort":"medium","default_enabled":true,"mandatory":true}}]}"#,
        );
        let info = &models[0];
        assert!(info.supports_effort);
        assert_eq!(info.request_effort(crate::intent::Effort::Max), None);
        assert_eq!(
            info.request_effort(crate::intent::Effort::Medium),
            Some(crate::intent::Effort::Medium)
        );
        let caps = info.reasoning.as_ref().unwrap();
        assert!(caps.mandatory);
        assert_eq!(caps.default_effort, Some(crate::intent::Effort::Medium));
        assert_eq!(
            info.fetched.as_ref().unwrap().reasoning.as_ref(),
            Some(caps)
        );
    }

    #[test]
    fn explicit_zero_defaults_cache_and_capabilities_survive_the_wire() {
        let models = openai_page(
            r#"{"data":[{"id":"custom/free","context_length":null,
            "top_provider":{"max_completion_tokens":8192},
            "pricing":{"prompt":"0","completion":"0","input_cache_read":"0.00000015"},
            "supported_parameters":["tools","structured_outputs"],
            "architecture":{"input_modalities":["text","image"]},
            "expiration_date":"2026-01-01"}]}"#,
        );
        let model = &models[0];
        let facts = model.fetched.as_ref().unwrap();
        assert_eq!(facts.context_max, None);
        assert_eq!(facts.max_output, Some(8192));
        assert_eq!(
            facts.pricing,
            Some(Pricing {
                input: 0.0,
                output: 0.0
            })
        );
        assert_eq!(model.cache_read, Some(0.15));
        assert_eq!(model.cache_write, None);
        let wire = serde_json::to_value(model).unwrap();
        assert_eq!(wire["supportedParameters"][0], "tools");
        assert_eq!(wire["inputModalities"][1], "image");
        assert_eq!(wire["expirationDate"], "2026-01-01");
        assert!(wire["fetched"].get("contextMax").is_none());
        assert!(wire.get("cacheWrite").is_none());
    }

    #[test]
    fn the_bare_openai_shape_gives_ids_and_nothing_else() {
        // Ollama, LM Studio, OpenAI itself.
        let models = openai_page(
            r#"{"object":"list","data":[
                {"id":"qwen3-coder","object":"model","created":1,"owned_by":"library"},
                {"id":"llama3.3","object":"model","created":2,"owned_by":"library"}
            ]}"#,
        );

        // Ordered by id: this shape has no order of its own.
        assert_eq!(models[0].id, "llama3.3");
        assert_eq!(models[0].display_name, "llama3.3");
        assert_eq!(models[0].context_source, ContextSource::DefaultGuess);
        assert!(models[0].fetched.as_ref().unwrap().context_max.is_none());
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn vllm_reports_its_context_window() {
        let models = openai_page(
            r#"{"data":[{"id":"Qwen/Qwen3-Coder","object":"model",
                         "owned_by":"vllm","max_model_len":262144}]}"#,
        );
        assert_eq!(models[0].context_max, 262_144);
    }

    #[test]
    fn openrouter_reports_the_name_the_limits_and_the_price() {
        let models = openai_page(
            r#"{"data":[{
                "id":"anthropic/claude-opus-5",
                "name":"Anthropic: Claude Opus 5",
                "context_length":1000000,
                "pricing":{"prompt":"0.000005","completion":"0.000025"},
                "top_provider":{"max_completion_tokens":128000},
                "supported_parameters":["tools","reasoning","max_tokens"]
            }]}"#,
        );

        let m = &models[0];
        assert_eq!(m.display_name, "Anthropic: Claude Opus 5");
        assert_eq!(m.context_max, 1_000_000);
        assert_eq!(m.max_output, 128_000);
        // Dollars per token arrive as strings; the feed counts per million.
        assert_eq!(m.pricing.input, 5.0);
        assert_eq!(m.pricing.output, 25.0);
        assert!(m.supports_effort);
        assert!(
            !m.supports_thinking,
            "this shape has no place for a signature"
        );
    }

    #[test]
    fn a_gateway_that_lists_parameters_without_reasoning_gets_no_effort() {
        let models =
            openai_page(r#"{"data":[{"id":"m","supported_parameters":["tools","max_tokens"]}]}"#);
        assert!(!models[0].supports_effort);
    }

    #[test]
    fn a_display_name_is_read_under_either_spelling() {
        // OpenRouter spells it `name`, a local Unsloth studio `display_name`.
        let models = openai_page(
            r#"{"data":[
                {"id":"a/A-GGUF","name":"A"},
                {"id":"b/B-GGUF","display_name":"B"},
                {"id":"c/C-GGUF"}
            ]}"#,
        );
        assert_eq!(models[0].display_name, "A");
        assert_eq!(models[1].display_name, "B");
        // Nothing sent: the id is all there is, and inventing a prettier name
        // would be guesswork the panel stopped doing.
        assert_eq!(models[2].display_name, "c/C-GGUF");
    }

    #[test]
    fn a_local_server_says_which_models_are_in_memory() {
        // Unsloth studio adds `loaded`; the specified shape has no such field.
        let page: OpenAiPage = serde_json::from_str(
            r#"{"data":[
                {"id":"a","loaded":true},
                {"id":"b","loaded":false},
                {"id":"c"}
            ]}"#,
        )
        .unwrap();

        let residency = residency_of(&page);
        assert_eq!(residency.get("a"), Some(&true));
        assert_eq!(residency.get("b"), Some(&false));
        // Silence is not "not loaded": nothing is claimed about `c`.
        assert_eq!(residency.get("c"), None);
    }

    #[test]
    fn a_variable_route_retains_its_price_state() {
        // OpenRouter answers `-1` where the cost is unknown to it.
        let models =
            openai_page(r#"{"data":[{"id":"m","pricing":{"prompt":"-1","completion":"-1"}}]}"#);
        assert_eq!(models[0].pricing.input, -1.0);
    }
}

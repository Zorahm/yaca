//! The OpenAI form's `/v1/models`: bare in the specification, and whatever
//! each gateway chose to add on top.

use std::collections::HashMap;

use serde::Deserialize;

use super::{get, parse};
use crate::error::ProviderError;
use crate::provider::registry::ProviderSpec;
use crate::usage::{unknown_model, ModelInfo, Pricing};


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
    let page: OpenAiPage = parse(get(client, spec, key, "/v1/models").await?)?;
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
    let page: OpenAiPage = parse(get(client, spec, key, "/v1/models").await?)?;
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
    /// A local server (Unsloth studio): is the model in memory right now.
    /// Live state, so it is read by [`residency`] and never into `ModelInfo`.
    #[serde(default)]
    loaded: Option<bool>,
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
    }
    if let Some(output) = m.top_provider.as_ref().and_then(|t| t.max_completion_tokens) {
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
    // Thinking blocks are never sent back in this shape (see `super::openai`),
    // so there is no point asking any model here for them.
    info.supports_thinking = false;
    info
}

/// Dollars per million, from dollars per token.
fn per_million(pricing: &OpenAiPricing) -> Option<Pricing> {
    let parse = |v: &Option<String>| v.as_ref()?.parse::<f64>().ok();
    let input = parse(&pricing.prompt)?;
    let output = parse(&pricing.completion)?;
    // A negative price is not a price: OpenRouter answers `"-1"` for models
    // whose cost it does not know itself.
    if input < 0.0 || output < 0.0 {
        return None;
    }
    Some(Pricing {
        input: input * 1e6,
        output: output * 1e6,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn openai_page(json: &str) -> Vec<ModelInfo> {
        openai_models(&serde_json::from_str(json).unwrap())
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
        assert_eq!(models[0], unknown_model("llama3.3"));
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
        assert!(!m.supports_thinking, "this shape has no place for a signature");
    }

    #[test]
    fn a_gateway_that_lists_parameters_without_reasoning_gets_no_effort() {
        let models = openai_page(
            r#"{"data":[{"id":"m","supported_parameters":["tools","max_tokens"]}]}"#,
        );
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
    fn a_price_the_gateway_does_not_know_is_not_a_price() {
        // OpenRouter answers `-1` where the cost is unknown to it.
        let models = openai_page(
            r#"{"data":[{"id":"m","pricing":{"prompt":"-1","completion":"-1"}}]}"#,
        );
        assert_eq!(models[0].pricing.input, 0.0);
    }
}

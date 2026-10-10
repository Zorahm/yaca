//! Anthropic's own `/v1/models`: paged, and rich enough to decide effort and
//! thinking by.

use serde::Deserialize;
use serde_json::Value;

use super::{get, parse};
use crate::error::ProviderError;
use crate::provider::registry::ProviderSpec;
use crate::usage::{ContextSource, unknown_model, ModelInfo};

/// Pages to walk before giving up on a provider that always says `has_more`.
const MAX_PAGES: usize = 20;

/// One page of Anthropic's list. 1000 is its own maximum.
const PAGE_SIZE: u32 = 1000;


/// Messages API's own `/v1/models`, paged.
pub(super) async fn anthropic(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let mut out = Vec::new();
    let mut after: Option<String> = None;

    for _ in 0..MAX_PAGES {
        let path = match &after {
            None => format!("/v1/models?limit={PAGE_SIZE}"),
            Some(id) => format!("/v1/models?limit={PAGE_SIZE}&after_id={id}"),
        };
        let page: AnthropicPage = parse(get(client, spec, key, &path).await?)?;

        out.extend(page.data.iter().map(anthropic_model));

        // `last_id` is what the next page starts after; without it there is no
        // way to ask for one, whatever `has_more` claims.
        match (page.has_more, page.last_id) {
            (true, Some(id)) => after = Some(id),
            _ => break,
        }
    }

    Ok(out)
}

/// The wire is snake_case here, as in every Anthropic body: no `rename_all`.
#[derive(Debug, Deserialize)]
struct AnthropicPage {
    #[serde(default)]
    data: Vec<AnthropicModel>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    last_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
    /// The context window. Absent on servers older than March 2026.
    #[serde(default)]
    max_input_tokens: Option<u32>,
    /// The output cap.
    #[serde(default)]
    max_tokens: Option<u32>,
    /// A tree of `{ "supported": bool }` leaves; shapes get added over time,
    /// so it is navigated rather than typed.
    #[serde(default)]
    capabilities: Option<Value>,
}

fn anthropic_model(m: &AnthropicModel) -> ModelInfo {
    let mut info = unknown_model(&m.id);
    if let Some(name) = &m.display_name {
        info.display_name = name.clone();
    }
    if let Some(context) = m.max_input_tokens {
        info.context_max = context;
        // Stated, not guessed: see `super::local` for why the tag matters.
        info.context_source = ContextSource::ProviderReported;
    }
    if let Some(output) = m.max_tokens {
        info.max_output = output;
    }
    // A silent provider means "not supported": sending `effort` to a model that
    // does not take it earns a 400, while not sending it to one that does costs
    // nothing but depth.
    info.supports_effort = supported(&m.capabilities, &["effort"]).unwrap_or(false);
    info.supports_thinking =
        supported(&m.capabilities, &["thinking", "types", "adaptive"]).unwrap_or(false);
    // The other way round: only a stated "no" turns images off (see
    // `ModelInfo::supports_images`).
    info.supports_images = supported(&m.capabilities, &["image_input"]).unwrap_or(true);
    info
}

/// A `supported` leaf under `capabilities`, if the provider put one there.
fn supported(capabilities: &Option<Value>, path: &[&str]) -> Option<bool> {
    let mut node = capabilities.as_ref()?;
    for key in path {
        node = node.get(key)?;
    }
    node.get("supported")?.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anthropic_page(json: &str) -> Vec<ModelInfo> {
        let page: AnthropicPage = serde_json::from_str(json).unwrap();
        page.data.iter().map(anthropic_model).collect()
    }

    #[test]
    fn anthropic_answers_with_everything_but_the_price() {
        let models = anthropic_page(
            r#"{"data":[{
                "id":"claude-opus-5",
                "display_name":"Claude Opus 5",
                "max_input_tokens":1000000,
                "max_tokens":128000,
                "capabilities":{
                    "effort":{"supported":true,"max":{"supported":true}},
                    "thinking":{"supported":true,"types":{
                        "enabled":{"supported":false},
                        "adaptive":{"supported":true}}}
                }
            }],"has_more":false}"#,
        );

        let m = &models[0];
        assert_eq!(m.display_name, "Claude Opus 5");
        assert_eq!(m.context_max, 1_000_000);
        assert_eq!(m.max_output, 128_000);
        assert!(m.supports_effort);
        assert!(m.supports_thinking);
        // The Models API serves no price; the price list fills it in by id.
        assert_eq!(m.pricing.input, 5.0);
        assert_eq!(m.pricing.output, 25.0);
    }

    #[test]
    fn a_model_that_only_takes_a_budget_is_not_offered_effort() {
        // Haiku 4.5 reasons by `budget_tokens`; adaptive it does not know.
        let models = anthropic_page(
            r#"{"data":[{
                "id":"claude-haiku-4-5",
                "display_name":"Claude Haiku 4.5",
                "capabilities":{
                    "effort":{"supported":false},
                    "thinking":{"supported":true,"types":{
                        "enabled":{"supported":true},
                        "adaptive":{"supported":false}}}
                }
            }]}"#,
        );

        assert!(!models[0].supports_effort);
        assert!(!models[0].supports_thinking, "adaptive is not among its types");
        assert_eq!(models[0].pricing.input, 1.0);
    }

    #[test]
    fn a_server_that_says_nothing_gets_the_safe_defaults() {
        // An older proxy answers with the id and nothing else.
        let models = anthropic_page(r#"{"data":[{"id":"claude-3-opus-20240229"}]}"#);

        let m = &models[0];
        assert_eq!(m.display_name, "claude-3-opus-20240229");
        assert_eq!(m, &unknown_model("claude-3-opus-20240229"));
        assert!(!m.supports_effort, "silence is a no, not a yes");
    }

    #[test]
    fn a_dated_snapshot_finds_its_row_in_the_price_list() {
        let models = anthropic_page(r#"{"data":[{"id":"claude-haiku-4-5-20251001"}]}"#);
        assert_eq!(models[0].pricing.input, 1.0);
    }

}

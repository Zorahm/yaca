//! What the public catalogs know about a model (§10).
//!
//! A provider's own `/v1/models` has the last word on what it serves, but the
//! bare OpenAI shape carries ids and nothing else. A gateway of one's own —
//! MiMo, a corporate mirror, a proxy in front of three vendors — answers
//! exactly that, and every number in the models panel reads as unknown, which
//! is the one thing the panel has fields for. Two public lists fill the gap,
//! and neither belongs to the provider being configured:
//!
//! - **models.dev** keeps one entry per provider, with the address it serves
//!   (`super::models_dev`), so a gateway of one's own is found by where it
//!   points rather than by what it is called.
//! - **OpenRouter's** `/api/v1/models` lists what its own address serves: some
//!   hundreds of models with limits, prices and the parameters each accepts.
//!   Matched by model id it also covers providers models.dev never heard of.
//!
//! Enrichment only fills what the provider left unsaid. A number the address
//! stated is never replaced by a catalog's — the whole point of asking the
//! provider first — and a row written by hand is not touched at all.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use serde_json::Value;

use super::{models_dev, price::per_million};
use crate::provider::registry::ProviderSpec;
use crate::usage::{ContextSource, ModelFacts, ModelInfo, Pricing};

/// OpenRouter's own list. Public: it needs no key.
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

/// What the catalogs say, indexed by model id.
///
/// Both a full id (`deepseek/deepseek-chat`) and its last segment
/// (`deepseek-chat`) are keys, because a gateway lists the second while the
/// catalog lists the first.
#[derive(Debug, Default)]
struct Catalog {
    by_id: HashMap<String, ModelFacts>,
    details: HashMap<String, Value>,
}

impl Catalog {
    /// Adds what this source says, keeping what another already stated.
    fn insert(&mut self, id: &str, facts: ModelFacts) {
        let key = id.trim().to_lowercase();
        if key.is_empty() || facts.is_empty() {
            return;
        }
        self.merge(&key, &facts);
        if let Some((_, short)) = key.rsplit_once('/') {
            if !short.is_empty() && short != key {
                self.merge(short, &facts);
            }
        }
    }

    fn merge(&mut self, key: &str, facts: &ModelFacts) {
        match self.by_id.entry(key.to_string()) {
            Entry::Occupied(mut kept) => kept.get_mut().fill_missing(facts),
            Entry::Vacant(free) => {
                free.insert(facts.clone());
            }
        }
    }

    fn get(&self, id: &str) -> Option<&ModelFacts> {
        self.by_id.get(&id.trim().to_lowercase())
    }

    fn insert_details(&mut self, id: &str, details: Value) {
        let key = id.trim().to_lowercase();
        self.details
            .entry(key.clone())
            .or_insert_with(|| details.clone());
        if let Some((_, short)) = key.rsplit_once('/') {
            self.details.entry(short.to_string()).or_insert(details);
        }
    }
}

/// Fills in what `models` leave unsaid, from whatever catalog knows the model.
///
/// Best effort, like the rest of discovery: a catalog that is blocked or down
/// leaves the list as the provider answered it, and the panel says those
/// numbers are unknown rather than making them up.
pub async fn enrich(client: &reqwest::Client, spec: &ProviderSpec, models: &mut [ModelInfo]) {
    if !models.iter().any(needs_facts) {
        return;
    }

    let mut catalog = Catalog::default();
    if let Some(dev) = get(client, models_dev::URL).await {
        if let Some(entry) = models_dev::provider(&dev, &spec.base_url, &spec.id) {
            if let Some(models) = entry.get("models").and_then(Value::as_object) {
                for (key, model) in models {
                    let id = model["id"].as_str().unwrap_or(key);
                    catalog.insert(id, models_dev::facts(model));
                    catalog.insert_details(
                        id,
                        serde_json::json!({
                            "supported_parameters": model["tool_call"].as_bool().map(|tools| {
                                if tools { vec!["tools"] } else { vec![] }
                            }),
                            "architecture": { "input_modalities": model["modalities"]["input"] }
                        }),
                    );
                }
            }
        }
    }
    // Asked second and only when something is still missing: it is the wider
    // list, but the provider-specific one has the better word on a model both
    // of them know.
    if models.iter().any(|model| {
        if !needs_facts(model) {
            return false;
        }
        let mut facts = ModelFacts::of(model);
        if let Some(other) = catalog.get(&model.id) {
            facts.fill_missing(other);
        }
        facts.context_max.is_none()
            || facts.max_output.is_none()
            || facts.pricing.is_none()
            || facts
                .reasoning
                .as_ref()
                .is_none_or(|caps| caps.supported_efforts.is_none())
    }) {
        if let Some(page) = get(client, OPENROUTER_MODELS_URL).await {
            absorb_openrouter(&mut catalog, &page);
        }
    }

    apply(&catalog, models);
}

/// Does anyone still have to be asked about this row?
fn needs_facts(model: &ModelInfo) -> bool {
    let facts = ModelFacts::of(model);
    !model.by_hand
        && (facts.context_max.is_none()
            || facts.max_output.is_none()
            || facts.pricing.is_none()
            || facts
                .reasoning
                .as_ref()
                .is_none_or(|caps| caps.supported_efforts.is_none()))
}

/// Only silence may be filled by a catalog. A stated zero price or an output
/// limit equal to our fallback is still a statement and must survive.
fn apply(catalog: &Catalog, models: &mut [ModelInfo]) {
    for model in models.iter_mut() {
        if model.by_hand {
            continue;
        }
        let Some(other) = catalog.get(&model.id) else {
            continue;
        };
        let mut facts = ModelFacts::of(model);
        facts.fill_missing(other);
        if let Some(context) = facts.context_max {
            model.context_max = context;
            model.context_source = ContextSource::ProviderReported;
        }
        if let Some(output) = facts.max_output {
            model.max_output = output;
        }
        if let Some(pricing) = facts.pricing {
            model.pricing = pricing;
        }
        if let Some(input) = facts.input_price {
            model.pricing.input = input;
        }
        if let Some(output) = facts.output_price {
            model.pricing.output = output;
        }
        if let Some(effort) = facts.supports_effort {
            model.supports_effort = effort;
        }
        model.reasoning = facts.reasoning.clone();
        if let Some(levels) = model
            .reasoning
            .as_ref()
            .and_then(|caps| caps.supported_efforts.as_ref())
        {
            model.supports_effort = facts.supports_effort != Some(false) && !levels.is_empty();
        }
        model.cache_read = facts.cache_read;
        model.cache_write = facts.cache_write;
        model.fetched = Some(facts);
        if let Some(details) = catalog.details.get(&model.id.trim().to_lowercase()) {
            let strings = |value: &Value| {
                value.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
            };
            if model.supported_parameters.is_none() {
                model.supported_parameters = strings(&details["supported_parameters"]);
            }
            if model.input_modalities.is_none() {
                model.input_modalities = strings(&details["architecture"]["input_modalities"]);
                if let Some(inputs) = &model.input_modalities {
                    model.supports_images = inputs.iter().any(|i| i == "image");
                }
            }
            if model.expiration_date.is_none() {
                model.expiration_date = details["expiration_date"].as_str().map(str::to_string);
            }
        }
    }
}

fn absorb_openrouter(catalog: &mut Catalog, page: &Value) {
    let Some(models) = page["data"].as_array() else {
        return;
    };
    for model in models {
        let Some(id) = model["id"].as_str() else {
            continue;
        };
        let pricing = &model["pricing"];
        catalog.insert_details(id, model.clone());
        let accepted = model["supported_parameters"].as_array();
        let reasoning = super::reasoning::openrouter(&model["reasoning"]);
        let supports_effort = reasoning
            .as_ref()
            .and_then(|caps| {
                caps.supported_efforts
                    .as_ref()
                    .map(|levels| !levels.is_empty())
            })
            .or_else(|| {
                accepted.map(|parameters| {
                    parameters
                        .iter()
                        .any(|p| matches!(p.as_str(), Some("reasoning") | Some("reasoning_effort")))
                })
            });
        catalog.insert(
            id,
            ModelFacts {
                reasoning,
                context_max: u32_of(&model["context_length"]),
                max_output: u32_of(&model["top_provider"]["max_completion_tokens"]),
                pricing: match (
                    per_million(pricing["prompt"].as_str()),
                    per_million(pricing["completion"].as_str()),
                ) {
                    (Some(input), Some(output)) => Some(Pricing { input, output }),
                    _ => None,
                },
                cache_read: per_million(pricing["input_cache_read"].as_str()),
                cache_write: per_million(pricing["input_cache_write"].as_str()),
                input_price: per_million(pricing["prompt"].as_str()),
                output_price: per_million(pricing["completion"].as_str()),
                supports_effort,
            },
        );
    }
}

/// A JSON number as a token count. `None` for anything that is not one — both
/// catalogs put `null` where a number is not known.
fn u32_of(value: &Value) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

async fn get(client: &reqwest::Client, url: &str) -> Option<Value> {
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "yaca")
        .send()
        .await
        .map_err(|e| tracing::debug!("catalog {url} could not be reached: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        tracing::debug!("catalog {url} answered HTTP {}", resp.status());
        return None;
    }
    resp.json::<Value>()
        .await
        .map_err(|e| tracing::debug!("catalog {url} could not be read: {e}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::unknown_model;

    fn info(id: &str) -> ModelInfo {
        unknown_model(id)
    }

    #[test]
    fn dollars_per_token_become_exact_dollars_per_million() {
        // The prices OpenRouter serves for DeepSeek V4.1 Flash.
        assert_eq!(per_million(Some("0.00000015")), Some(0.15));
        assert_eq!(per_million(Some("0.0000006")), Some(0.6));
        assert_eq!(per_million(Some("0.000003")), Some(3.0));
        assert_eq!(per_million(Some("0.5")), Some(500_000.0));
        assert_eq!(per_million(Some("0")), Some(0.0));
        // Not a price: OpenRouter's "the route decides".
        assert_eq!(per_million(Some("-1")), Some(-1.0));
        assert_eq!(per_million(None), None);
    }

    #[test]
    fn a_catalog_fills_the_gaps_and_leaves_the_answers_alone() {
        let mut models = vec![
            info("deepseek-flash"),
            // The provider stated this one itself.
            ModelInfo {
                context_max: 64_000,
                context_source: ContextSource::ProviderReported,
                pricing: Pricing {
                    input: 1.0,
                    output: 2.0,
                },
                max_output: 4_096,
                ..info("deepseek-v4-pro")
            },
        ];
        let mut catalog = Catalog::default();
        catalog.insert(
            "deepseek/deepseek-flash",
            ModelFacts {
                context_max: Some(1_048_576),
                max_output: Some(393_216),
                pricing: Some(Pricing {
                    input: 0.15,
                    output: 0.6,
                }),
                supports_effort: Some(true),
                ..ModelFacts::default()
            },
        );
        catalog.insert(
            "deepseek/deepseek-v4-pro",
            ModelFacts {
                context_max: Some(1_000_000),
                max_output: Some(64_000),
                pricing: Some(Pricing {
                    input: 9.0,
                    output: 9.0,
                }),
                supports_effort: Some(true),
                ..ModelFacts::default()
            },
        );

        apply(&catalog, &mut models);

        // The catalog's id is `deepseek/…`; the gateway serves the short one.
        assert_eq!(models[0].context_max, 1_048_576);
        assert_eq!(models[0].max_output, 393_216);
        assert_eq!(models[0].pricing.input, 0.15);
        assert_eq!(models[0].pricing.output, 0.6);
        assert_eq!(models[0].context_source, ContextSource::ProviderReported);
        assert!(models[0].supports_effort);

        // Nothing the address stated is overwritten.
        assert_eq!(models[1].context_max, 64_000);
        assert_eq!(models[1].max_output, 4_096);
        assert_eq!(models[1].pricing.input, 1.0);
    }

    #[test]
    fn a_hand_edit_is_never_touched_by_a_catalog() {
        let mut model = info("deepseek-flash");
        model.by_hand = true;
        model.context_max = 512_000;
        let mut models = vec![model];

        let mut catalog = Catalog::default();
        catalog.insert(
            "deepseek-flash",
            ModelFacts {
                context_max: Some(1_048_576),
                ..ModelFacts::default()
            },
        );

        apply(&catalog, &mut models);
        assert_eq!(models[0].context_max, 512_000);
    }

    #[test]
    fn a_price_set_by_hand_survives_the_catalog() {
        let mut model = info("m");
        model.pricing_by_hand = true;
        model.pricing = Pricing {
            input: 3.0,
            output: 12.0,
        };
        let mut models = vec![model];

        let mut catalog = Catalog::default();
        catalog.insert(
            "m",
            ModelFacts {
                pricing: Some(Pricing {
                    input: 1.0,
                    output: 2.0,
                }),
                ..ModelFacts::default()
            },
        );

        apply(&catalog, &mut models);
        assert_eq!(models[0].pricing.input, 3.0);
    }

    #[test]
    fn a_model_only_one_catalog_knows_is_still_filled_in() {
        let model = info("mystery-model");
        assert!(needs_facts(&model));

        let mut catalog = Catalog::default();
        absorb_openrouter(
            &mut catalog,
            &serde_json::from_str(
                r#"{"data":[{"id":"vendor/mystery-model","context_length":200000,
                     "top_provider":{"max_completion_tokens":8000},
                     "pricing":{"prompt":"0.000001","completion":"0.000002"},
                     "supported_parameters":["tools","reasoning"]}]}"#,
            )
            .unwrap(),
        );

        let mut models = vec![model];
        apply(&catalog, &mut models);
        assert_eq!(models[0].context_max, 200_000);
        assert_eq!(models[0].max_output, 8_000);
        assert_eq!(models[0].pricing.input, 1.0);
        assert_eq!(models[0].pricing.output, 2.0);
        assert!(models[0].supports_effort);
    }

    #[test]
    fn complete_numbers_do_not_prevent_fetching_missing_reasoning_controls() {
        let stated = ModelInfo {
            context_max: 128_000,
            context_source: ContextSource::ProviderReported,
            max_output: 16_000,
            pricing: Pricing {
                input: 1.0,
                output: 2.0,
            },
            ..info("m")
        };
        assert!(needs_facts(&stated));
        let complete = ModelInfo {
            reasoning: Some(crate::reasoning::ReasoningCapabilities {
                supported_efforts: Some(vec![]),
                ..Default::default()
            }),
            ..stated
        };
        assert!(!needs_facts(&complete));

        let mut hand = info("m");
        hand.by_hand = true;
        assert!(!needs_facts(&hand));
    }
}

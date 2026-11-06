//! models.dev: one public entry per provider — the address it serves, and for
//! every model the limits, the price per million and what it can do.
//!
//! The database answers as one `api.json` holding all providers; a provider is
//! picked out of it by the address it serves rather than by its name, because
//! the same upstream appears under several names («MiMo», «Xiaomi Token Plan»)
//! and a provider named «deepseek» may well be pointed elsewhere. Which entry
//! describes which configured provider is [`super::catalog`]'s question; this
//! module knows the shape.

use serde_json::Value;

use crate::usage::{ModelFacts, Pricing};

/// The whole database, every provider in one answer.
pub const URL: &str = "https://models.dev/api.json?type=all";

/// The entry describing this provider: the one with the same address first,
/// the one with the same id second.
pub fn provider<'a>(dev: &'a Value, base_url: &str, id: &str) -> Option<&'a Value> {
    let providers = dev.as_object()?;
    if let Some(host) = host_of(base_url) {
        if let Some((_, entry)) = providers
            .iter()
            .find(|(_, entry)| entry["api"].as_str().and_then(host_of).as_deref() == Some(&host))
        {
            return Some(entry);
        }
    }
    providers.get(&id.to_lowercase())
}

/// The host of an address, lower-cased: `https://API.DeepSeek.com/v1` is
/// `api.deepseek.com`. `None` for a placeholder such as `${BASE_URL}/v1`,
/// which the database carries too.
fn host_of(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?.trim();
    let host = host.to_lowercase();
    (host.contains('.') && !host.contains('{')).then_some(host)
}

/// What this entry says about one of its models.
pub fn facts(model: &Value) -> ModelFacts {
    let limit = &model["limit"];
    let cost = &model["cost"];
    ModelFacts {
        context_max: u32_of(&limit["context"]),
        max_output: u32_of(&limit["output"]),
        pricing: match (cost["input"].as_f64(), cost["output"].as_f64()) {
            // Already dollars per million, which is what the meter counts in.
            (Some(input), Some(output)) if input >= 0.0 && output >= 0.0 => {
                Some(Pricing { input, output })
            }
            _ => None,
        },
        supports_effort: super::reasoning::models_dev(model)
            .and_then(|caps| caps.supported_efforts.map(|levels| !levels.is_empty())),
        reasoning: super::reasoning::models_dev(model),
        cache_read: cost["cache_read"].as_f64(),
        cache_write: cost["cache_write"].as_f64(),
        input_price: cost["input"].as_f64().filter(|v| *v >= 0.0),
        output_price: cost["output"].as_f64().filter(|v| *v >= 0.0),
    }
}

/// A JSON number as a token count. `None` for anything that is not one — both
/// catalogs put `null` where a number is not known.
fn u32_of(value: &Value) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = r#"{
      "deepseek": {
        "id": "deepseek",
        "api": "https://api.deepseek.com",
        "models": {
          "deepseek-flash": {
            "id": "deepseek-flash",
            "name": "DeepSeek V4.1 Flash",
            "limit": {"context": 1048576, "output": 393216},
            "cost": {"input": 0.15, "output": 0.6},
            "reasoning": true,
            "modalities": {"input": ["text", "image"], "output": ["text"]}
          }
        }
      },
      "xiaomi-token-plan-sgp": {
        "id": "xiaomi-token-plan-sgp",
        "api": "https://token-plan-sgp.xiaomimimo.com/v1",
        "models": {
          "mimo-v2.6-flash": {
            "id": "mimo-v2.6-flash",
            "name": "MiMo v2.6 Flash",
            "limit": {"context": 262144, "output": 65536},
            "cost": {"input": 0.2, "output": 0.8},
            "reasoning_options": [{"type": "effort", "values": ["low", "high"]}]
          }
        }
      }
    }"#;

    #[test]
    fn an_entry_is_found_by_the_address_it_serves() {
        let dev: Value = serde_json::from_str(DEV).unwrap();
        let entry = provider(&dev, "https://token-plan-sgp.xiaomimimo.com/v1", "mimo")
            .expect("the address is the entry's");
        assert_eq!(entry["id"], "xiaomi-token-plan-sgp");
    }

    #[test]
    fn a_name_finds_an_entry_only_when_the_address_does_not() {
        let dev: Value = serde_json::from_str(DEV).unwrap();
        let entry = provider(&dev, "https://gateway.example.com", "DeepSeek")
            .expect("the id is the entry's");
        assert_eq!(entry["id"], "deepseek");

        // A provider named after one upstream but pointed at another is the
        // address's: a proxy in front of MiMo is not DeepSeek.
        let entry = provider(&dev, "https://token-plan-sgp.xiaomimimo.com", "deepseek")
            .expect("the address wins");
        assert_eq!(entry["id"], "xiaomi-token-plan-sgp");
    }

    #[test]
    fn a_placeholder_address_is_not_a_host() {
        assert_eq!(host_of("${NEON_AI_GATEWAY_BASE_URL}/v1"), None);
        assert_eq!(host_of("http://localhost:11434/v1"), None);
        assert_eq!(
            host_of("https://API.DeepSeek.com/v1").as_deref(),
            Some("api.deepseek.com")
        );
    }

    #[test]
    fn an_entry_provides_the_model_facts_it_states() {
        let dev: Value = serde_json::from_str(DEV).unwrap();
        let model = &dev["deepseek"]["models"]["deepseek-flash"];
        let stated = facts(model);
        assert_eq!(stated.context_max, Some(1_048_576));
        assert_eq!(stated.max_output, Some(393_216));
        assert_eq!(
            stated.pricing,
            Some(Pricing {
                input: 0.15,
                output: 0.6
            })
        );
        assert_eq!(stated.supports_effort, None);
        assert!(stated.reasoning.is_some());
    }
}

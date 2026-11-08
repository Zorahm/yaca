//! Read explicit reasoning controls without inferring them from a capability tag.

use serde_json::Value;

use crate::intent::Effort;
use crate::reasoning::ReasoningCapabilities;

fn effort(value: &Value) -> Option<Effort> {
    match value.as_str()? {
        "none" => Some(Effort::None),
        "minimal" => Some(Effort::Minimal),
        "low" => Some(Effort::Low),
        "medium" => Some(Effort::Medium),
        "high" => Some(Effort::High),
        "xhigh" | "extra" => Some(Effort::Extra),
        "max" => Some(Effort::Max),
        _ => None,
    }
}

fn levels(value: &Value) -> Option<Vec<Effort>> {
    let reported = value.as_array()?;
    Some(
        Effort::LEVELS
            .into_iter()
            .filter(|level| reported.iter().any(|v| effort(v) == Some(*level)))
            .collect(),
    )
}

pub(super) fn openrouter(value: &Value) -> Option<ReasoningCapabilities> {
    value.as_object()?;
    Some(ReasoningCapabilities {
        supported_efforts: match value.get("supported_efforts") {
            Some(Value::Null) => Some(Effort::LEVELS.to_vec()),
            Some(value) => levels(value),
            None => Some(vec![]),
        },
        default_effort: effort(&value["default_effort"]),
        default_enabled: value["default_enabled"].as_bool(),
        mandatory: value["mandatory"].as_bool().unwrap_or(false),
        supports_max_tokens: value["supports_max_tokens"].as_bool().unwrap_or(false),
    })
}

pub(super) fn deepseek(value: &Value) -> Option<ReasoningCapabilities> {
    value.as_object()?;
    Some(ReasoningCapabilities {
        supported_efforts: levels(&value["supported_levels"]),
        default_effort: effort(&value["default_level"]),
        ..Default::default()
    })
}

pub(super) fn models_dev(model: &Value) -> Option<ReasoningCapabilities> {
    let options = model["reasoning_options"].as_array();
    let supported_efforts = options.and_then(|options| {
        options.iter().find_map(|option| {
            matches!(option["type"].as_str(), Some("effort") | Some("reasoning"))
                .then(|| levels(&option["values"]))
                .flatten()
        })
    });
    let budget = options.is_some_and(|options| {
        options.iter().any(|option| {
            matches!(
                option["type"].as_str(),
                Some("budget") | Some("budget_tokens") | Some("thinking_budget")
            )
        })
    });
    (model["reasoning"].as_bool() == Some(true) || supported_efforts.is_some() || budget).then_some(
        ReasoningCapabilities {
            supported_efforts,
            supports_max_tokens: budget,
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_reasoning_null_efforts_and_an_omitted_selector_are_distinct() {
        assert!(openrouter(&Value::Null).is_none());
        assert_eq!(
            openrouter(&json!({})).unwrap().supported_efforts,
            Some(vec![])
        );
        assert_eq!(
            openrouter(&json!({"supported_efforts": null}))
                .unwrap()
                .supported_efforts,
            Some(Effort::LEVELS.to_vec())
        );
        assert_eq!(
            openrouter(&json!({"supported_efforts": []}))
                .unwrap()
                .supported_efforts,
            Some(vec![])
        );
    }

    #[test]
    fn capability_tags_do_not_invent_effort_levels() {
        let caps = models_dev(&json!({"reasoning": true})).unwrap();
        assert_eq!(caps.supported_efforts, None);
        let caps = models_dev(&json!({"reasoning_options": [{"type": "effort", "values": ["high", "low", "future", "xhigh"]}]})).unwrap();
        assert_eq!(
            caps.supported_efforts,
            Some(vec![Effort::Low, Effort::High, Effort::Extra])
        );
    }
}

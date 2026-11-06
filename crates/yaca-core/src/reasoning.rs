//! Model-specific reasoning controls, independent of the transport's spelling.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::intent::Effort;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningCapabilities {
    /// Absent means unknown, an empty list means there is no effort selector.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_efforts: Option<Vec<Effort>>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<Effort>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_enabled: Option<bool>,
    #[serde(default)]
    pub mandatory: bool,
    #[serde(default)]
    pub supports_max_tokens: bool,
}

impl ReasoningCapabilities {
    pub fn allows(&self, effort: Effort) -> bool {
        effort != Effort::Auto
            && !(effort == Effort::None && self.mandatory)
            && self
                .supported_efforts
                .as_ref()
                .is_some_and(|levels| levels.contains(&effort))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_controls_fill_a_tag_but_cannot_replace_an_explicit_provider_list() {
        let reported = ReasoningCapabilities {
            supported_efforts: Some(vec![Effort::Low, Effort::High]),
            default_effort: Some(Effort::High),
            mandatory: true,
            ..Default::default()
        };
        let mut tag = crate::usage::ModelFacts {
            reasoning: Some(ReasoningCapabilities::default()),
            ..Default::default()
        };
        tag.fill_missing(&crate::usage::ModelFacts {
            reasoning: Some(reported.clone()),
            ..Default::default()
        });
        assert_eq!(tag.reasoning, Some(reported.clone()));
        let mut explicit = crate::usage::ModelFacts {
            reasoning: Some(ReasoningCapabilities {
                supported_efforts: Some(vec![]),
                ..Default::default()
            }),
            ..Default::default()
        };
        explicit.fill_missing(&tag);
        assert_eq!(explicit.reasoning.unwrap().supported_efforts, Some(vec![]));
    }

    #[test]
    fn mandatory_reasoning_cannot_be_disabled_even_with_inconsistent_metadata() {
        let caps = ReasoningCapabilities {
            supported_efforts: Some(vec![Effort::None, Effort::High]),
            mandatory: true,
            ..Default::default()
        };
        assert!(!caps.allows(Effort::None));
        assert!(caps.allows(Effort::High));
        assert!(!caps.allows(Effort::Max));
        assert!(!ReasoningCapabilities::default().allows(Effort::High));
    }
}

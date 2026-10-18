//! `routing.json`: what the router asks, what it matches, where it sends.
//!
//! Application-level, next to `providers.json` and for the same reason: a
//! project does not get to decide where the conversation is sent. A file that
//! does not parse stops the caller instead of falling back to "no routing".
//!
//! The shape follows the "decision model" idea of vLLM Semantic Router: small
//! typed questions about the request (`signals`), rules over the answers
//! (`decisions`), and a model per matched rule. Only what the core can execute
//! is declared here — see the module note in [`super`] for what is left out.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A concrete model on a concrete provider from `providers.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

/// The whole of `routing.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RoutingConfig {
    /// The id the router is offered under in the model switcher, for example
    /// `auto`. The provider id it lives under is always [`super::ROUTER_ID`].
    pub name: String,
    /// The model that answers the questions.
    pub decision_model: DecisionModel,
    /// Where a turn goes when no decision matched or the questions could not
    /// be answered. Required: a router without a floor would have to fail the
    /// turn, and a routing hiccup is not worth a lost message.
    pub default: ModelRef,
    #[serde(default)]
    pub signals: Vec<Signal>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DecisionModel {
    #[serde(flatten)]
    pub model: ModelRef,
    /// How long the questions may take before they count as unanswered.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u32,
}

fn default_timeout_ms() -> u32 {
    1000
}

/// One named question about the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub name: String,
    pub question: Question,
}

/// What is asked, in plain language, and what shape the answer takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Is this true? Answer: the probability of yes.
    Noul { instructions: String },
    /// Which one fits? Answer: a probability per option.
    Choice {
        instructions: String,
        choices: Vec<Choice>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Choice {
    pub key: String,
    pub description: String,
}

/// A rule set and the model it routes to. Evaluated in descending `priority`;
/// among equal priorities the earlier one in the file wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub name: String,
    #[serde(default)]
    pub priority: i32,
    pub rules: Rule,
    /// What an unanswered signal in `rules` means for this decision.
    #[serde(default)]
    pub on_unknown: OnUnknown,
    pub model: ModelRef,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum OnUnknown {
    /// The decision does not match. The safe side: an unanswered question must
    /// not send a conversation to an expensive model.
    #[default]
    NoMatch,
    Match,
}

/// A boolean expression over signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Rule {
    All(Vec<Rule>),
    Any(Vec<Rule>),
    Not(Box<Rule>),
    Signal(Condition),
}

/// One signal read as a fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Name of a [`Signal`].
    pub name: String,
    /// For a `choice` signal: which option is read. Absent for `noul`.
    #[serde(default)]
    pub label: Option<String>,
    /// Absent: `noul` needs P(yes) >= 0.5, `choice` needs the label to be the
    /// most probable option.
    #[serde(default)]
    pub predicate: Option<Predicate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Predicate {
    pub gte: f32,
}

/// What the decision model said about one signal.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalValue {
    /// P(yes).
    Noul(f32),
    /// Probability per option key.
    Choice(BTreeMap<String, f32>),
    /// The model was late, refused, or answered something unusable.
    Unknown,
}

/// Every signal's answer, by signal name.
pub type Answers = BTreeMap<String, SignalValue>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The example in the README is what people copy; a type change that
    /// breaks it has to break this test too.
    #[test]
    fn the_readme_example_parses_as_a_routing_config() {
        let readme = include_str!("../../../../README.md");
        let section = readme
            .split("## Model Router")
            .nth(1)
            .expect("the README has a Model Router section");
        let json = section
            .split("```json")
            .nth(1)
            .and_then(|rest| rest.split("```").next())
            .expect("the section has a json example");

        let cfg: RoutingConfig = serde_json::from_str(json).expect("the example parses");

        assert_eq!(cfg.name, "auto");
        assert_eq!(cfg.decision_model.timeout_ms, 3000);
        assert_eq!(cfg.decision_model.model.provider, "decision");
        assert_eq!(cfg.signals.len(), 2);
        assert_eq!(cfg.decisions.len(), 1);
        assert_eq!(cfg.decisions[0].on_unknown, OnUnknown::NoMatch);
        assert!(matches!(&cfg.decisions[0].rules, Rule::All(rules) if rules.len() == 2));
    }

    #[test]
    fn the_decision_models_timeout_defaults_when_omitted() {
        let json = r#"{"name":"auto",
            "decisionModel":{"provider":"p","model":"m"},
            "default":{"provider":"p","model":"m"}}"#;
        let cfg: RoutingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.decision_model.timeout_ms, 1000);
        assert!(cfg.signals.is_empty() && cfg.decisions.is_empty());
    }
}

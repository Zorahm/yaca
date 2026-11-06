//! One caller-supplied judgment in the typed API and chat shapes.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ProviderError, ToolError};
use crate::usage::TokenCounts;

#[derive(Deserialize)]
pub(super) struct Question {
    question: String,
    state: Option<String>,
    choices: Option<BTreeMap<String, String>>,
}

impl Question {
    pub fn parse(input: &Value) -> Result<Self, ToolError> {
        let mut question: Self = serde_json::from_value(input.clone())
            .map_err(|e| ToolError::failed(format!("invalid Decision question: {e}")))?;
        if question.question.trim().is_empty() {
            return Err(ToolError::failed("missing question"));
        }
        question.question = super::capped(question.question.trim());
        question.state = question.state.map(|state| super::capped(state.trim()));
        if let Some(choices) = &question.choices {
            if !(2..=255).contains(&choices.len())
                || choices.iter().any(|(key, description)| {
                    key.trim().is_empty() || description.trim().is_empty()
                })
            {
                return Err(ToolError::failed(
                    "choices must contain 2 to 255 nonempty option names and descriptions",
                ));
            }
        }
        Ok(question)
    }

    pub fn state(&self) -> &str {
        self.state.as_deref().unwrap_or(&self.question)
    }

    pub fn typed(&self) -> Value {
        let question = match &self.choices {
            Some(choices) => json!({
                "type": "choice", "instructions": self.question, "criteria": choices,
            }),
            None => json!({ "type": "noul", "instructions": self.question }),
        };
        json!({ "q": question })
    }

    pub fn chat(&self) -> String {
        let mut text = self.question.clone();
        if let Some(state) = &self.state {
            text.push_str(&format!("\n\nContext:\n{state}"));
        }
        if let Some(choices) = &self.choices {
            text.push_str("\n\nOptions:\n");
            for (name, description) in choices {
                text.push_str(&format!("- {name}: {description}\n"));
            }
        }
        text
    }

    pub fn read_answer(&self, reply: &Value) -> Result<String, ProviderError> {
        let answer = &reply["answers"]["q"];
        let malformed =
            || ProviderError::Protocol("no valid answer for the Decision question".into());
        if let Some(choices) = &self.choices {
            let selected = answer["choice"].as_str().ok_or_else(malformed)?;
            if !choices.contains_key(selected) {
                return Err(malformed());
            }
            let mut probabilities = BTreeMap::new();
            for key in choices.keys() {
                probabilities.insert(
                    key.as_str(),
                    probability(&answer["probabilities"][key]).ok_or_else(malformed)?,
                );
            }
            let selected_p = probabilities[selected];
            if selected_p == 0.0 || probabilities.values().any(|p| *p > selected_p) {
                return Err(malformed());
            }
            Ok(
                json!({ "type": "choice", "choice": selected, "probabilities": probabilities })
                    .to_string(),
            )
        } else {
            let p = probability(&answer["noul"]).ok_or_else(malformed)?;
            Ok(json!({ "type": "noul", "noul": p }).to_string())
        }
    }
}

fn probability(value: &Value) -> Option<f64> {
    value.as_f64().filter(|p| (0.0..=1.0).contains(p))
}

pub(super) fn usage(reply: &Value) -> TokenCounts {
    let count = |key: &str| {
        reply["usage"][key]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    };
    TokenCounts {
        input: count("input_tokens"),
        output: count("output_tokens"),
        ..TokenCounts::default()
    }
}

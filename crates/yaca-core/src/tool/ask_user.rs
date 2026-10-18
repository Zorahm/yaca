//! `AskUser`: a question with ready options, answered before the turn goes on.
//!
//! The call blocks until the human answers, dismisses, or stops the turn. The
//! answer is the result of this call, so the model carries on in the same turn
//! with what was chosen. Nothing here decides whether to ask — the model does
//! that, and a question is never a substitute for a permission.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::question::{Question, QuestionOption, QuestionReply, MAX_OPTIONS, MIN_OPTIONS};

pub struct AskUser;

#[async_trait]
impl Tool for AskUser {
    fn name(&self) -> &'static str {
        "AskUser"
    }

    fn description(&self) -> &'static str {
        "Ask the human a question with ready options and wait for the answer. Use it when \
         the next step depends on a choice only the human can make (which approach, which \
         file, which of several readings of the request) and the choices are known. Give \
         2 to 6 options with short labels. Unless allow_other is false, the human may \
         also type an answer of their own. Choose the kind of choice with multi_select: \
         false (default) is radio, one option; if the human also types text, only the \
         text comes back. true is checkboxes, any number of options; ticked options \
         and typed text all come back together. The answer comes back as this call's \
         result and the turn goes on from there. Do not use it to ask permission for an action: \
         actions that need permission are asked for by the system when you make them, \
         and do not use it for questions you can settle by reading the code."
    }

    fn properties(&self) -> Value {
        json!({
            "question": {
                "type": "string",
                "minLength": 1,
                "description": "The question itself, one or two sentences, in the \
                                conversation's language.",
            },
            "options": {
                "type": "array",
                "minItems": MIN_OPTIONS,
                "maxItems": MAX_OPTIONS,
                "description": "The ready answers, in the order they should be shown.",
                "items": {
                    "type": "object",
                    "properties": {
                        "label": {
                            "type": "string",
                            "minLength": 1,
                            "description": "Short name of the option, shown on its button.",
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional line under the label saying what \
                                            picking it means.",
                        },
                    },
                    "required": ["label"],
                    "additionalProperties": false,
                },
            },
            "allow_other": {
                "type": "boolean",
                "default": true,
                "description": "Whether the human may type an answer of their own. \
                                Default true; set false only when the options are \
                                exhaustive.",
            },
            "multi_select": {
                "type": "boolean",
                "default": false,
                "description": "false (default): radio, the human picks one option, and \
                                text they type replaces the pick. true: checkboxes, \
                                the human ticks any number of options, and text they \
                                type is added to them. Use true when the options are \
                                not mutually exclusive.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["question", "options"]
    }

    /// The row in the feed says what was asked in the model's words, like any
    /// other tool whose meaning the interface cannot read from its facts.
    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["question"].as_str().map(str::to_string)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let question = parse(&input)?;
        let questions = ctx
            .questions
            .as_ref()
            .ok_or_else(|| ToolError::failed("no receiver for question"))?;

        let reply = questions.ask(question.clone(), &ctx.cancel).await?;

        // The feed gets its line in the conversation's language through progress,
        // and the model gets its own in English through the output. Progress marks
        // the stream as used, so the session does not print the output a second time.
        ctx.progress.push(for_human(&question.options, &reply));
        Ok(ToolOutput::new(for_model(&question.options, &reply)))
    }
}

/// Reads the arguments into a question, refusing what the form does not allow.
///
/// The schema states the same bounds (`minItems`, `maxItems`); the check here is
/// for what a schema cannot say and for calls whose arguments reach the tool
/// without the provider enforcing the schema.
fn parse(input: &Value) -> Result<Question, ToolError> {
    let question = input["question"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ToolError::failed_with(
                "question is required",
                "AskUser needs a non-empty `question`. Make the call again with the question text.",
            )
        })?;

    let raw = input["options"].as_array().ok_or_else(|| {
        ToolError::failed_with(
            "options are required",
            "AskUser needs `options`: an array of 2 to 6 objects with a `label`.",
        )
    })?;
    if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&raw.len()) {
        return Err(ToolError::failed_with(
            format!("{count} options provided, expected between {MIN_OPTIONS} and {MAX_OPTIONS}", count = raw.len()),
            format!(
                "AskUser takes between {MIN_OPTIONS} and {MAX_OPTIONS} options, and this \
                 call gave {}. Make the call again with options in that range: a choice \
                 of one is not a question, and more than {MAX_OPTIONS} do not fit on \
                 screen.",
                raw.len()
            ),
        ));
    }

    let options = raw
        .iter()
        .map(|option| {
            let label = option["label"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    ToolError::failed_with(
                        "option has no label",
                        "Every option of AskUser needs a non-empty `label`.",
                    )
                })?;
            let description = option["description"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Ok(QuestionOption {
                label: label.to_string(),
                description,
            })
        })
        .collect::<Result<Vec<_>, ToolError>>()?;

    Ok(Question {
        question: question.to_string(),
        options,
        allow_other: input["allow_other"].as_bool().unwrap_or(true),
        multi_select: input["multi_select"].as_bool().unwrap_or(false),
    })
}

/// The feed's line for the expanded row, in the conversation's language.
fn for_human(options: &[QuestionOption], reply: &QuestionReply) -> String {
    match reply {
        QuestionReply::Dismissed => "Question skipped".to_string(),
        QuestionReply::Answered { selected, other } => {
            let picked = labels(options, selected);
            match (picked.is_empty(), other) {
                (false, None) => format!("Answer: {}", picked.join(", ")),
                (false, Some(text)) => format!("Answer: {} · custom text: \"{text}\"", picked.join(", ")),
                (true, Some(text)) => format!("Answer in custom text: \"{text}\""),
                (true, None) => "No answer".to_string(),
            }
        }
    }
}

/// What the model reads back. The label is the option's own words, so the
/// model gets the choice the human made and not a number it would have to map.
fn for_model(options: &[QuestionOption], reply: &QuestionReply) -> String {
    match reply {
        QuestionReply::Dismissed => "The user dismissed the question without answering. Do not \
             assume an answer. Carry on only if the choice does not change the outcome; \
             otherwise stop and wait for the user's next message."
            .to_string(),
        QuestionReply::Answered { selected, other } => {
            let picked = labels(options, selected);
            match (picked.is_empty(), other) {
                (false, None) => format!("The user chose: {}.", quoted(&picked)),
                (false, Some(text)) => format!(
                    "The user chose: {}. They also wrote: \"{text}\".",
                    quoted(&picked)
                ),
                (true, Some(text)) => {
                    format!("The user answered in their own words: \"{text}\".")
                }
                (true, None) => "The user gave no answer.".to_string(),
            }
        }
    }
}

fn labels(options: &[QuestionOption], selected: &[u32]) -> Vec<String> {
    selected
        .iter()
        .filter_map(|&i| options.get(i as usize))
        .map(|option| option.label.clone())
        .collect()
}

fn quoted(labels: &[String]) -> String {
    labels
        .iter()
        .map(|label| format!("\"{label}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(n: usize) -> Value {
        Value::Array(
            (0..n)
                .map(|i| json!({ "label": format!("option {i}") }))
                .collect(),
        )
    }

    fn call(options: Value) -> Value {
        json!({ "question": "Which way?", "options": options })
    }

    #[test]
    fn one_option_is_rejected() {
        assert!(parse(&call(options(1))).is_err());
    }

    #[test]
    fn seven_options_are_rejected() {
        assert!(parse(&call(options(7))).is_err());
    }

    #[test]
    fn the_bounds_of_two_and_six_are_accepted() {
        assert!(parse(&call(options(2))).is_ok());
        assert!(parse(&call(options(6))).is_ok());
    }

    #[test]
    fn the_schema_states_the_same_bounds() {
        let schema = AskUser.properties();
        assert_eq!(schema["options"]["minItems"], json!(2));
        assert_eq!(schema["options"]["maxItems"], json!(6));
        assert_eq!(schema["options"]["items"]["required"], json!(["label"]));
    }

    #[test]
    fn allow_other_defaults_on_and_multi_select_off() {
        let question = parse(&call(options(2))).unwrap();
        assert!(question.allow_other);
        assert!(!question.multi_select);

        let mut input = call(options(2));
        input["allow_other"] = json!(false);
        input["multi_select"] = json!(true);
        let question = parse(&input).unwrap();
        assert!(!question.allow_other);
        assert!(question.multi_select);
    }

    #[test]
    fn an_empty_label_is_refused() {
        let input = call(json!([{ "label": "  " }, { "label": "fine" }]));
        assert!(parse(&input).is_err());
    }

    #[test]
    fn a_blank_description_is_no_description() {
        let input = call(json!([
            { "label": "a", "description": "  " },
            { "label": "b", "description": "does b" },
        ]));
        let question = parse(&input).unwrap();
        assert_eq!(question.options[0].description, None);
        assert_eq!(question.options[1].description.as_deref(), Some("does b"));
    }

    #[test]
    fn the_model_reads_the_labels_it_offered() {
        let opts = parse(&call(options(3))).unwrap().options;
        let reply = QuestionReply::Answered {
            selected: vec![2],
            other: None,
        };
        assert_eq!(for_model(&opts, &reply), "The user chose: \"option 2\".");
    }

    #[test]
    fn a_dismissal_tells_the_model_not_to_guess() {
        let opts = parse(&call(options(2))).unwrap().options;
        let text = for_model(&opts, &QuestionReply::Dismissed);
        assert!(text.contains("dismissed"));
        assert!(text.contains("Do not"));
    }

    #[test]
    fn the_feed_line_is_in_the_conversation_language() {
        let opts = parse(&call(options(2))).unwrap().options;
        let reply = QuestionReply::Answered {
            selected: Vec::new(),
            other: Some("third way".into()),
        };
        assert_eq!(
            for_human(&opts, &reply),
            "Answer in custom text: \"third way\""
        );
    }

    #[test]
    fn the_target_is_the_question() {
        assert_eq!(
            AskUser.target(&call(options(2))).as_deref(),
            Some("Which way?")
        );
    }
}

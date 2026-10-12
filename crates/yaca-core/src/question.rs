//! The question the agent puts to the human, with options (`AskUser`).
//!
//! Not a permission. A permission is a yes or no about an action the agent is
//! about to take; a question is a choice the agent cannot make for the human
//! and needs before it can go on. It comes back as the result of the tool call
//! that asked, so the turn continues in the same place (§5.7 has no block for
//! it; the feed draws one in the spirit of the permission block).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CallId, RequestId};

/// Fewest options a question may offer. One option is not a choice.
pub const MIN_OPTIONS: usize = 2;
/// Most options a question may offer. More than six stop fitting on a screen
/// next to the keys that pick them (1–6, §5.7-style) and stop being read.
pub const MAX_OPTIONS: usize = 6;

/// One ready answer the human can pick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    /// Short; it is what the model reads back when this option is picked.
    pub label: String,
    /// One line under the label, saying what picking it means.
    pub description: Option<String>,
}

/// A question on screen, as the feed draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct QuestionRequest {
    pub id: RequestId,
    /// Ties the block to the call that asked.
    pub call_id: CallId,
    pub question: String,
    pub options: Vec<QuestionOption>,
    /// The human may type an answer of their own instead of, or next to, the
    /// options.
    pub allow_other: bool,
    /// Several options may be picked at once.
    pub multi_select: bool,
}

/// What the human did with a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuestionReply {
    /// Picked options by their index in [`QuestionRequest::options`], and/or
    /// wrote an answer of their own.
    #[serde(rename_all = "camelCase")]
    Answered {
        selected: Vec<u32>,
        other: Option<String>,
    },
    /// Closed without an answer. The agent is told so and is not left to guess.
    Dismissed,
}

/// A question as the tool asks it, before it has an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub question: String,
    pub options: Vec<QuestionOption>,
    pub allow_other: bool,
    pub multi_select: bool,
}

impl Question {
    pub fn into_request(self, id: RequestId, call_id: CallId) -> QuestionRequest {
        QuestionRequest {
            id,
            call_id,
            question: self.question,
            options: self.options,
            allow_other: self.allow_other,
            multi_select: self.multi_select,
        }
    }
}

impl QuestionRequest {
    /// Why a reply cannot stand, in the words the feed shows. `None` when it can.
    ///
    /// Checked where the reply arrives, before it reaches the tool: a reply
    /// that names an option which does not exist would otherwise reach the
    /// model as an answer nobody gave.
    pub fn refuses(&self, reply: &QuestionReply) -> Option<&'static str> {
        let QuestionReply::Answered { selected, other } = reply else {
            return None;
        };

        if selected.iter().any(|&i| i as usize >= self.options.len()) {
            return Some("такого варианта нет");
        }
        let mut seen = selected.clone();
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != selected.len() {
            return Some("вариант выбран дважды");
        }
        if !self.multi_select && selected.len() > 1 {
            return Some("можно выбрать только один вариант");
        }
        match other {
            Some(_) if !self.allow_other => Some("свой ответ здесь не предусмотрен"),
            Some(text) if text.trim().is_empty() => Some("ответ пустой"),
            _ if selected.is_empty() && other.is_none() => Some("ответ пустой"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(options: usize, allow_other: bool, multi_select: bool) -> QuestionRequest {
        Question {
            question: "Which one?".into(),
            options: (0..options)
                .map(|i| QuestionOption {
                    label: format!("option {i}"),
                    description: None,
                })
                .collect(),
            allow_other,
            multi_select,
        }
        .into_request(RequestId::from("req_1"), CallId::from("call_1"))
    }

    fn picked(selected: &[u32]) -> QuestionReply {
        QuestionReply::Answered {
            selected: selected.to_vec(),
            other: None,
        }
    }

    #[test]
    fn a_pick_inside_the_options_stands() {
        assert_eq!(request(3, true, false).refuses(&picked(&[2])), None);
    }

    #[test]
    fn an_index_past_the_options_is_refused() {
        assert_eq!(
            request(3, true, false).refuses(&picked(&[3])),
            Some("такого варианта нет")
        );
    }

    #[test]
    fn two_picks_need_multi_select() {
        assert_eq!(
            request(3, true, false).refuses(&picked(&[0, 1])),
            Some("можно выбрать только один вариант")
        );
        assert_eq!(request(3, true, true).refuses(&picked(&[0, 1])), None);
    }

    #[test]
    fn the_same_option_twice_is_refused() {
        assert_eq!(
            request(3, true, true).refuses(&picked(&[1, 1])),
            Some("вариант выбран дважды")
        );
    }

    #[test]
    fn an_own_answer_needs_allow_other() {
        let reply = QuestionReply::Answered {
            selected: Vec::new(),
            other: Some("neither".into()),
        };
        assert_eq!(
            request(2, false, false).refuses(&reply),
            Some("свой ответ здесь не предусмотрен")
        );
        assert_eq!(request(2, true, false).refuses(&reply), None);
    }

    #[test]
    fn an_empty_answer_is_refused_and_a_blank_one_too() {
        assert_eq!(
            request(2, true, false).refuses(&picked(&[])),
            Some("ответ пустой")
        );
        let blank = QuestionReply::Answered {
            selected: Vec::new(),
            other: Some("   ".into()),
        };
        assert_eq!(
            request(2, true, false).refuses(&blank),
            Some("ответ пустой")
        );
    }

    #[test]
    fn a_dismissal_is_always_accepted() {
        assert_eq!(
            request(2, false, false).refuses(&QuestionReply::Dismissed),
            None
        );
    }
}

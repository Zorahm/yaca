//! A session as Markdown (§9).
//!
//! Built from the model's history rather than from the feed journal, because
//! the history is whole: the journal carries text in deltas and tool output in
//! chunks, and reassembling a document out of a stream is work already done
//! once when the messages were written.
//!
//! What comes out is a transcript, not a replay. Thinking is left out — §5.3
//! keeps it folded away in the feed too — and tool output is cut short. A
//! forty-thousand-line build log is not what anyone exports a session for, and
//! keeping it whole would make the export unreadable in the same stroke as
//! making it unopenable.

use std::collections::HashMap;

use super::history::Record;
use super::meta::SessionMeta;
use crate::message::{ContentBlock, Role};

/// How much of a tool's output survives into the document.
const RESULT_MAX_CHARS: usize = 600;
/// How much of a tool's arguments are shown beside its name.
const INPUT_MAX_CHARS: usize = 200;

pub fn markdown(meta: &SessionMeta, records: &[Record]) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", heading(&meta.title)));
    out.push_str(&format!(
        "`{}` · {} message(s) · ${:.4}\n\n",
        meta.model, meta.messages, meta.cost_usd
    ));
    if let Some(from) = &meta.forked_from {
        out.push_str(&format!("Duplicated from `{from}`\n\n"));
    }
    if !meta.files.is_empty() {
        out.push_str("Files touched:\n\n");
        for path in &meta.files {
            out.push_str(&format!("- `{path}`\n"));
        }
        out.push('\n');
    }

    let results = results_by_call(records);

    for record in records {
        match record {
            Record::Prompt { text, .. } => {
                out.push_str("---\n\n### You\n\n");
                out.push_str(text.trim());
                out.push_str("\n\n");
            }
            Record::Reset { .. } => {
                // §5.11: what follows is a summary, not the conversation it
                // replaced. Saying so is the difference between a transcript
                // and a transcript with a hole in it.
                out.push_str("---\n\n_History was compacted here._\n\n");
            }
            Record::Message { message, .. } if message.role == Role::Assistant => {
                let body = assistant(&message.content, &results);
                if !body.trim().is_empty() {
                    out.push_str("### Yaca\n\n");
                    out.push_str(&body);
                }
            }
            // The user message that carries the prompt is already above it,
            // and the ones carrying tool results are folded into the calls.
            Record::Message { .. } => {}
            // Bookkeeping for the model's reasoning, which the export leaves
            // out anyway.
            Record::Prefix { .. } => {}
        }
    }

    out
}

fn assistant(blocks: &[ContentBlock], results: &HashMap<&str, &ContentBlock>) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                out.push_str(text.trim());
                out.push_str("\n\n");
            }
            ContentBlock::ToolUse { id, name, input } => {
                out.push_str(&format!("**{}** {}\n\n", name, arguments(input)));
                if let Some(ContentBlock::ToolResult {
                    content, is_error, ..
                }) = results.get(id.as_str())
                {
                    let label = if *is_error { "failed" } else { "" };
                    out.push_str(&format!(
                        "```{label}\n{}\n```\n\n",
                        clip(content, RESULT_MAX_CHARS)
                    ));
                }
            }
            // Folded away in the feed (§5.3), and folded away here.
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
            ContentBlock::ToolResult { .. } | ContentBlock::Image { .. } => {}
        }
    }
    out
}

fn results_by_call(records: &[Record]) -> HashMap<&str, &ContentBlock> {
    let mut out = HashMap::new();
    for record in records {
        let Record::Message { message, .. } = record else {
            continue;
        };
        for block in &message.content {
            if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                out.insert(tool_use_id.as_str(), block);
            }
        }
    }
    out
}

/// Arguments on one line, so the call reads as a line and not as a document.
fn arguments(input: &serde_json::Value) -> String {
    let text = match input {
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(k, v)| match v {
                serde_json::Value::String(s) => format!("{k}={}", one_line(s)),
                other => format!("{k}={other}"),
            })
            .collect::<Vec<_>>()
            .join(" "),
        other => other.to_string(),
    };
    format!("`{}`", clip(&text, INPUT_MAX_CHARS))
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(max).collect();
    format!("{head}\n… truncated")
}

/// A session that was never named still needs a heading.
fn heading(title: &str) -> &str {
    if title.trim().is_empty() {
        "Session"
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{SessionId, TurnId};
    use crate::message::Message;

    fn session() -> (SessionMeta, Vec<Record>) {
        let mut meta = SessionMeta::new(SessionId::from("ses_1"), "anthropic".into(), "claude-opus-5".into());
        meta.title = "Date parsing".into();
        meta.messages = 1;
        meta.cost_usd = 0.31;
        meta.files = vec!["src/a.rs".into()];

        let turn = TurnId::from("t1");
        let records = vec![
            Record::Prompt {
                turn: turn.clone(),
                text: "fix date parsing".into(),
            },
            Record::Message {
                turn: turn.clone(),
                message: Message::assistant(vec![
                    ContentBlock::Thinking {
                        thinking: "pondering".into(),
                        signature: None,
                    },
                    ContentBlock::Text {
                        text: "Looking at parser.".into(),
                    },
                    ContentBlock::ToolUse {
                        id: "call_1".into(),
                        name: "Read".into(),
                        input: serde_json::json!({ "path": "src/a.rs" }),
                    },
                ]),
            },
            Record::Message {
                turn,
                message: Message::tool_results(vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "fn parse() {}".into(),
                    is_error: false,
                }]),
            },
        ];
        (meta, records)
    }

    #[test]
    fn the_transcript_has_both_sides_and_the_calls_between_them() {
        let (meta, records) = session();
        let md = markdown(&meta, &records);

        assert!(md.starts_with("# Date parsing"));
        assert!(md.contains("### You\n\nfix date parsing"));
        assert!(md.contains("### Yaca"));
        assert!(md.contains("**Read** `path=src/a.rs`"));
        assert!(md.contains("fn parse() {}"), "{md}");
    }

    #[test]
    fn thinking_stays_folded_away() {
        let (meta, records) = session();
        // §5.3 keeps it collapsed in the feed; an export is not a place to
        // publish it either.
        assert!(!markdown(&meta, &records).contains("pondering"));
    }

    #[test]
    fn a_long_result_is_cut_rather_than_pasted_whole() {
        let (meta, mut records) = session();
        records.push(Record::Message {
            turn: TurnId::from("t1"),
            message: Message::assistant(vec![ContentBlock::ToolUse {
                id: "call_2".into(),
                name: "Bash".into(),
                input: serde_json::json!({ "command": "cargo build" }),
            }]),
        });
        records.push(Record::Message {
            turn: TurnId::from("t1"),
            message: Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "call_2".into(),
                content: "line\n".repeat(5_000),
                is_error: false,
            }]),
        });

        let md = markdown(&meta, &records);
        assert!(md.contains("truncated"));
        assert!(md.chars().count() < 4_000, "{}", md.chars().count());
    }

    #[test]
    fn a_compaction_is_marked_and_not_papered_over() {
        let (meta, mut records) = session();
        records.push(Record::Reset {
            turn: TurnId::from("t2"),
            kept: 0,
        });
        assert!(markdown(&meta, &records).contains("compacted"));
    }

    #[test]
    fn an_unnamed_session_still_gets_a_heading() {
        let (mut meta, records) = session();
        meta.title = String::new();
        assert!(markdown(&meta, &records).starts_with("# Session"));
    }
}

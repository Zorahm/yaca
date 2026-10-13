//! Looking back into the conversation.
//!
//! While a message is in the window the model has it, and this tool would
//! only hand it a second copy. It is for what the window no longer holds:
//! compaction replaces the early turns with a summary, and the summary is not
//! the words. The journal still has them (`store::transcript`), so a phrase the
//! human used an hour ago, or the exact text they asked to have changed, is
//! one call away instead of gone.
//!
//! Two uses of one tool, by which argument is given: `query` lists the
//! messages a phrase occurs in, `number` returns one of them whole.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::message::Role;
use crate::store::search::Needle;
use crate::store::transcript::Entry;

/// Matches listed by one search. More than this is a query to narrow, and the
/// most recent are the ones kept: a phrase is usually looked for near where
/// the conversation is now.
const MAX_HITS: usize = 20;
/// Characters of one message returned by one call. A pasted log can be a
/// message too, and it would take the window the tool exists to spare.
const DEFAULT_LIMIT: usize = 20_000;

pub struct SearchMessage;

#[async_trait]
impl Tool for SearchMessage {
    fn name(&self) -> &'static str {
        "SearchMessage"
    }

    fn description(&self) -> &'static str {
        "Look up earlier messages of this conversation: what the human wrote and the \
         text of your own answers, including everything from before the conversation \
         was compacted into a summary. Give `query` to list the messages a phrase \
         occurs in, each with its number and an excerpt; give `number` to get one \
         message back word for word. Use it when you need the exact wording of \
         something that is no longer in front of you. Messages you can still see \
         do not need it. Tool calls and their output are not searched."
    }

    fn properties(&self) -> Value {
        json!({
            "query": {
                "type": "string",
                "description": "A phrase to look for, matched as written, ignoring case.",
            },
            "role": {
                "type": "string",
                "enum": ["user", "assistant"],
                "description": "Search only the human's messages or only your own. Omit for both.",
            },
            "number": {
                "type": "integer",
                "minimum": 1,
                "description": "The number of the message to return in full, as a search listed it.",
            },
            "offset": {
                "type": "integer",
                "minimum": 0,
                "description": "With `number`: the character to start from, for a message longer than one call returns.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &[]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Reads the journal and changes nothing.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        match (input["number"].as_u64(), input["query"].as_str()) {
            (Some(number), _) => Some(format!("#{number}")),
            (None, Some(query)) => Some(query.to_string()),
            (None, None) => None,
        }
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        // Offered only to a session that keeps a journal, so `None` is a call
        // made outside one.
        let transcript = ctx
            .transcript
            .clone()
            .ok_or_else(|| ToolError::failed("у этой сессии нет журнала сообщений"))?;
        let entries = tokio::task::spawn_blocking(move || transcript.entries())
            .await
            .map_err(|_| ToolError::failed("чтение журнала не завершилось"))?
            .map_err(|e| ToolError::failed(format!("журнал сообщений не читается: {e}")))?;

        if let Some(number) = input["number"].as_u64() {
            let offset = input["offset"].as_u64().unwrap_or(0) as usize;
            return whole(&entries, number as usize, offset);
        }

        let Some(needle) = input["query"].as_str().and_then(Needle::new) else {
            return Err(ToolError::failed_with(
                "не задан ни query, ни number",
                "Give `query` to search the conversation or `number` to get one message.",
            ));
        };
        let role = match input["role"].as_str() {
            Some("user") => Some(Role::User),
            Some("assistant") => Some(Role::Assistant),
            _ => None,
        };
        Ok(found(&entries, &needle, role))
    }
}

fn who(role: Role) -> &'static str {
    match role {
        Role::User => "человек",
        Role::Assistant => "агент",
    }
}

/// The messages the phrase occurs in, one line each.
fn found(entries: &[Entry], needle: &Needle, role: Option<Role>) -> ToolOutput {
    let hits: Vec<String> = entries
        .iter()
        .filter(|entry| role.is_none_or(|role| entry.role == role))
        .filter_map(|entry| {
            let excerpt = needle.excerpt(&entry.text)?;
            Some(format!("#{} · {} · {excerpt}\n", entry.number, who(entry.role)))
        })
        .collect();

    let total = hits.len();
    if total == 0 {
        return ToolOutput::new("Совпадений нет.\n".to_string()).items(0);
    }
    let mut text: String = hits[total.saturating_sub(MAX_HITS)..].concat();
    if total > MAX_HITS {
        text.push_str(&format!(
            "\n[совпадений {total}, показаны последние {MAX_HITS}: уточни запрос]\n"
        ));
    }
    ToolOutput::new(text)
        .items(total as u32)
        .truncated(total > MAX_HITS)
}

/// One message as it was written, from `offset` characters in.
fn whole(entries: &[Entry], number: usize, offset: usize) -> Result<ToolOutput, ToolError> {
    let entry = number
        .checked_sub(1)
        .and_then(|index| entries.get(index))
        .ok_or_else(|| {
            ToolError::failed(format!(
                "сообщения #{number} нет: в разговоре их {}",
                entries.len()
            ))
        })?;

    let chars: Vec<char> = entry.text.chars().collect();
    let total = chars.len();
    if offset >= total && total > 0 {
        return Err(ToolError::failed(format!(
            "в сообщении #{number} {total} символов, запрошен offset {offset}"
        )));
    }
    let end = (offset + DEFAULT_LIMIT).min(total);
    let body: String = chars[offset..end].iter().collect();

    let mut text = format!("#{number} · {} · {total} симв.\n\n{body}\n", who(entry.role));
    let truncated = end < total;
    if offset > 0 || truncated {
        text.push_str(&format!("\n[показаны символы {}–{end} из {total}", offset + 1));
        if truncated {
            text.push_str(&format!("; дальше — offset {end}"));
        }
        text.push_str("]\n");
    }
    Ok(ToolOutput::new(text).truncated(truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(number: usize, role: Role, text: &str) -> Entry {
        Entry {
            number,
            role,
            text: text.into(),
        }
    }

    #[test]
    fn more_hits_than_a_listing_holds_keeps_the_most_recent_and_says_so() {
        let entries: Vec<Entry> = (1..=25)
            .map(|n| entry(n, Role::User, "про даты"))
            .collect();

        let out = found(&entries, &Needle::new("даты").unwrap(), None);

        let lines: Vec<&str> = out.text.lines().filter(|l| l.starts_with('#')).collect();
        assert_eq!(lines.len(), 20);
        assert!(lines[0].starts_with("#6 "), "{}", lines[0]);
        assert!(lines[19].starts_with("#25 "), "{}", lines[19]);
        assert!(out.text.contains("совпадений 25"), "{}", out.text);
        assert_eq!(out.meta.items, Some(25));
        assert!(out.meta.truncated);
    }

    #[test]
    fn a_long_message_comes_in_parts_that_say_where_the_next_starts() {
        let long = "я".repeat(DEFAULT_LIMIT + 5);
        let entries = [entry(1, Role::User, &long)];

        let first = whole(&entries, 1, 0).unwrap();
        assert!(first.meta.truncated);
        assert!(first.text.contains(&format!("offset {DEFAULT_LIMIT}")), "{}", &first.text[first.text.len() - 120..]);

        let rest = whole(&entries, 1, DEFAULT_LIMIT).unwrap();
        assert!(!rest.meta.truncated);
        assert_eq!(rest.text.matches('я').count(), 5);
    }

    #[test]
    fn an_offset_past_the_end_is_an_error_not_an_empty_answer() {
        let entries = [entry(1, Role::User, "коротко")];
        assert!(whole(&entries, 1, 7).is_err());
        assert!(whole(&entries, 0, 0).is_err());
    }
}

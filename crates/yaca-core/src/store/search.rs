//! Searching the sessions of a project (§9).
//!
//! Two things are searched, and §9 names both: the text of messages, and the
//! paths of the files a session touched. The title comes along because it is
//! the first message, cut short.
//!
//! The cheap half — title and paths — is answered from the index alone. The
//! text costs a read of the session's message log, and only sessions that could
//! not be answered cheaply pay it. There is no ranking: a project holds tens or
//! hundreds of sessions, not millions, and a list ordered by recency is the
//! order the panel shows anyway.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::history::Record;
use super::meta::SessionMeta;
use crate::message::{ContentBlock, Role};

/// How much text surrounds a match in the line the panel shows.
const EXCERPT_RADIUS: usize = 40;
/// Enough to show that a session matches; the session itself holds the rest.
pub const MATCHES_PER_SESSION: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum MatchKind {
    Title,
    /// A path the session changed.
    Path,
    /// The text of a message, from either side.
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub kind: MatchKind,
    /// The matching text with a little around it, whitespace collapsed.
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub session: SessionMeta,
    pub matches: Vec<Match>,
}

/// What the index alone can answer.
pub fn in_meta(meta: &SessionMeta, needle: &Needle) -> Vec<Match> {
    let mut out = Vec::new();
    if let Some(excerpt) = needle.excerpt(&meta.title) {
        out.push(Match {
            kind: MatchKind::Title,
            excerpt,
        });
    }
    for path in &meta.files {
        if out.len() >= MATCHES_PER_SESSION {
            break;
        }
        if needle.position(path).is_some() {
            out.push(Match {
                kind: MatchKind::Path,
                excerpt: path.clone(),
            });
        }
    }
    out
}

/// What the message log answers, which is the text of the conversation.
///
/// Both sides. A session is as often remembered by what the agent said as by
/// what was asked of it.
pub fn in_messages(records: &[Record], needle: &Needle, room: usize) -> Vec<Match> {
    let mut out = Vec::new();
    for record in records {
        if out.len() >= room {
            break;
        }
        let texts = match record {
            Record::Prompt { text, .. } => vec![text.as_str()],
            Record::Message { message, .. } if message.role == Role::Assistant => message
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect(),
            // Tool results are the model's working notes, not the
            // conversation: a build log matching "error" is not a session
            // about errors.
            _ => continue,
        };
        for text in texts {
            if let Some(excerpt) = needle.excerpt(text) {
                out.push(Match {
                    kind: MatchKind::Message,
                    excerpt,
                });
                break;
            }
        }
    }
    out
}

/// A query, lowercased once instead of once per candidate.
///
/// Case folding is done character by character so that positions stay lined up
/// with the original text: an excerpt is cut out of what the human wrote, not
/// out of the lowercased copy.
pub struct Needle {
    chars: Vec<char>,
}

impl Needle {
    pub fn new(query: &str) -> Option<Needle> {
        let chars: Vec<char> = fold(query.trim());
        (!chars.is_empty()).then_some(Needle { chars })
    }

    /// Where the query sits in the text, in characters.
    fn position(&self, text: &str) -> Option<usize> {
        let hay = fold(text);
        if self.chars.len() > hay.len() {
            return None;
        }
        (0..=hay.len() - self.chars.len()).find(|&i| hay[i..i + self.chars.len()] == self.chars[..])
    }

    /// The match with a little text around it, or `None` if there is no match.
    pub fn excerpt(&self, text: &str) -> Option<String> {
        let at = self.position(text)?;
        let chars: Vec<char> = text.chars().collect();
        let from = at.saturating_sub(EXCERPT_RADIUS);
        let to = (at + self.chars.len() + EXCERPT_RADIUS).min(chars.len());

        let body: String = chars[from..to].iter().collect();
        let body = body.split_whitespace().collect::<Vec<_>>().join(" ");

        let mut out = String::new();
        if from > 0 {
            out.push('…');
        }
        out.push_str(&body);
        if to < chars.len() {
            out.push('…');
        }
        Some(out)
    }

    pub fn matches(&self, text: &str) -> bool {
        self.position(text).is_some()
    }
}

/// One character in, one character out, so indexes keep meaning something.
/// `ß` folding to `ss` would shift every position after it.
fn fold(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{SessionId, TurnId};
    use crate::message::Message;

    fn meta() -> SessionMeta {
        let mut m = SessionMeta::new(SessionId::from("ses_1"), "anthropic".into(), "claude-opus-5".into());
        m.title = "Разбор дат в сегментах".into();
        m.files = vec!["src/segments.ts".into(), "src/app.rs".into()];
        m
    }

    fn needle(q: &str) -> Needle {
        Needle::new(q).unwrap()
    }

    #[test]
    fn an_empty_query_is_not_a_query() {
        assert!(Needle::new("   ").is_none());
    }

    #[test]
    fn the_title_matches_whatever_the_case() {
        let hits = in_meta(&meta(), &needle("РАЗБОР"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, MatchKind::Title);
        // Cut out of what the human wrote, not out of the lowercased copy.
        assert_eq!(hits[0].excerpt, "Разбор дат в сегментах");
    }

    #[test]
    fn a_path_matches_by_any_part_of_it() {
        // §9 searches "по путям затронутых файлов", and nobody types the
        // whole path.
        let hits = in_meta(&meta(), &needle("segments"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, MatchKind::Path);
        assert_eq!(hits[0].excerpt, "src/segments.ts");
    }

    #[test]
    fn both_sides_of_the_conversation_are_searched() {
        let records = vec![
            Record::Prompt {
                turn: TurnId::from("t1"),
                text: "почему падает разбор".into(),
            },
            Record::Message {
                turn: TurnId::from("t1"),
                message: Message::assistant(vec![ContentBlock::Text {
                    text: "виноват часовой пояс".into(),
                }]),
            },
        ];
        assert_eq!(in_messages(&records, &needle("падает"), 5).len(), 1);
        // A session is as often remembered by the answer as by the question.
        assert_eq!(in_messages(&records, &needle("часовой"), 5).len(), 1);
    }

    #[test]
    fn tool_output_is_not_the_conversation() {
        let records = vec![Record::Message {
            turn: TurnId::from("t1"),
            message: Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "t".into(),
                content: "error: cannot find value".into(),
                is_error: true,
            }]),
        }];
        // Otherwise every session that ever ran a failing build matches
        // "error", and the search stops meaning anything.
        assert!(in_messages(&records, &needle("error"), 5).is_empty());
    }

    #[test]
    fn a_long_match_is_shown_with_its_surroundings() {
        let text = "a".repeat(200) + " игла " + &"b".repeat(200);
        let excerpt = needle("игла").excerpt(&text).unwrap();
        assert!(excerpt.starts_with('…') && excerpt.ends_with('…'));
        assert!(excerpt.contains("игла"));
        assert!(excerpt.chars().count() < 100);
    }

    #[test]
    fn a_query_longer_than_the_text_does_not_panic() {
        assert!(!needle("длинный запрос целиком").matches("нет"));
    }
}

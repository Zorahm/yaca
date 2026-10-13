//! Naming the conversation.
//!
//! Without this the title is the first line of the first message (§3), which
//! is often «привет» or a pasted stack trace. The model has read the whole
//! request by the time it answers, so it is the one that can say what the
//! conversation is about.
//!
//! The model names a conversation once, at its start, and never again: a
//! second name would race the human's rename for the same field.
//!
//! The tool only emits [`AgentEvent::TitleSet`]. The index folds that event
//! like any other (`SessionMeta::absorb`), which is also why a rebuilt index
//! comes back with the same title, and why a name the human typed in wins
//! even if the tool is not told.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::event::AgentEvent;
use crate::intent::Intent;
use crate::store::meta::{title_of, TITLE_MAX_CHARS};

pub struct SetTitle;

#[async_trait]
impl Tool for SetTitle {
    fn name(&self) -> &'static str {
        "SetTitle"
    }

    fn description(&self) -> &'static str {
        "Set the title of this conversation, shown in the human's chat list. \
         Call it once, in your first response, as soon as you know what the \
         conversation is about, together with your other first calls. The title \
         can be set only once: later calls are refused, and so is any call after \
         the human has renamed the chat. A title is a short noun phrase in the \
         human's language, at most 60 characters, with no quotes and no \
         trailing period: «Разбор дат в логах», not «Помогаю разобрать даты»."
    }

    fn properties(&self) -> Value {
        json!({
            "title": {
                "type": "string",
                "description": "The title, at most 60 characters.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["title"]
    }

    /// The new name reaches the interface as `TitleSet`; a row for the call
    /// would be the model's bookkeeping shown to the human.
    fn silent(&self) -> bool {
        true
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["title"].as_str().map(title_of)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["title"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан title"))?;

        // A name is given once, and a human's name is final: the model cannot
        // race them for it. The index would ignore a second one anyway; the
        // refusal is so the model is told, not left thinking it worked.
        if ctx.title_locked {
            return Err(ToolError::failed(
                "название чата уже задано и больше не меняется",
            ));
        }

        let title = clean(raw);
        if title.is_empty() {
            return Err(ToolError::failed("title пуст"));
        }

        ctx.events.emit(AgentEvent::TitleSet {
            title: title.clone(),
        });
        Ok(ToolOutput::new(format!("Название чата: {title}\n")))
    }
}

/// What reaches the list: one line, no wrapping quotes or closing full stop,
/// cut to the header's width.
fn clean(raw: &str) -> String {
    // One trim for all of it: the full stop may sit outside the quotes
    // («Разбор дат».) or inside them, and the order is not worth guessing.
    let line = title_of(raw);
    let line = line.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '«' | '»' | '“' | '”' | '.')
    });
    debug_assert!(line.chars().count() <= TITLE_MAX_CHARS);
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_with_events;

    #[test]
    fn quotes_and_the_closing_full_stop_are_not_part_of_a_title() {
        assert_eq!(clean("«Разбор дат в логах»."), "Разбор дат в логах");
        assert_eq!(clean("  \"Fix the parser\"  "), "Fix the parser");
    }

    #[test]
    fn only_the_first_line_is_kept_and_a_long_one_is_cut() {
        assert_eq!(clean("Parser fix\nand more"), "Parser fix");
        assert!(clean(&"слово ".repeat(40)).chars().count() <= TITLE_MAX_CHARS);
    }

    #[tokio::test]
    async fn the_title_goes_out_as_an_event() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);

        let out = SetTitle
            .run(&ctx, json!({ "title": "«Разбор дат»." }))
            .await
            .unwrap();

        assert!(out.text.contains("Разбор дат"));
        match rx.try_recv().unwrap() {
            AgentEvent::TitleSet { title } => assert_eq!(title, "Разбор дат"),
            other => panic!("не то событие: {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_empty_title_is_an_error_the_model_can_read() {
        let dir = TempDir::new();
        let (ctx, _rx) = ctx_with_events(&dir);

        let err = SetTitle
            .run(&ctx, json!({ "title": " «» " }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("пуст"));
    }
}

#[cfg(test)]
mod once_tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_with_events;

    #[tokio::test]
    async fn a_locked_title_is_refused_and_sends_nothing() {
        let dir = TempDir::new();
        let (mut ctx, mut rx) = ctx_with_events(&dir);
        ctx.title_locked = true;

        let err = SetTitle
            .run(&ctx, json!({ "title": "Другое название" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("уже задано"));
        assert!(rx.try_recv().is_err());
    }
}

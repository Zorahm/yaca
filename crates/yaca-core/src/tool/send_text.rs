//! Putting a file's text in front of the human.
//!
//! Without this the only way the model has to show a text is to write it out:
//! a draft it has just saved, a file the human asked to see, a report a script
//! left behind — each is read in and then typed again as the answer, and the
//! output tokens are the expensive ones. Here the text goes from the disk to
//! the feed, and the model is told only that it went.
//!
//! The tool emits [`AgentEvent::TextShown`] and has no row: the block is what
//! the human sees, and a row beside it would say the same thing twice.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::event::AgentEvent;
use crate::intent::Intent;

/// How much one call may show. The text is written into the journal and kept
/// by the feed for as long as the session is open, and a block longer than
/// this is not read by anyone: it is a file to open, not a part of an answer.
const MAX_BYTES: usize = 200_000;

pub struct SendText;

#[async_trait]
impl Tool for SendText {
    fn name(&self) -> &'static str {
        "SendText"
    }

    fn description(&self) -> &'static str {
        "Show the human the text of a project file, or a range of its lines, as a \
         block in the conversation. Use it instead of writing the text out yourself \
         whenever what you want to show already exists in a file: a draft you saved \
         or edited, a file they asked to see, output a command wrote to a file. The \
         human sees the text exactly as it is in the file; you get back only a note \
         that it was shown, so do not repeat the text in your answer. You do not \
         need to Read the file first, and showing a file does not count as reading it. \
         Instead of `path`, `command` shows what a background command has printed, \
         by its id — the human sees its output without you copying it."
    }

    fn properties(&self) -> Value {
        json!({
            "path": {
                "type": "string",
                "description": "Path relative to the project root.",
            },
            "command": {
                "type": "integer",
                "description": "Id of a background command, instead of path: shows \
                                what it has printed.",
            },
            "start_line": {
                "type": "integer",
                "minimum": 1,
                "description": "First line to show, 1-indexed. Omit to start at the top.",
            },
            "end_line": {
                "type": "integer",
                "minimum": 1,
                "description": "Last line to show, inclusive. Omit to go to the end.",
            },
        })
    }

    /// One of `path` and `command`, which the schema cannot say.
    fn required(&self) -> &'static [&'static str] {
        &[]
    }

    /// The text reaches the interface as `TextShown`.
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
        input["path"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| input["command"].as_u64().map(|id| format!("command {id}")))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let (label, command, text) = match (input["path"].as_str(), input["command"].as_u64()) {
            (Some(_), Some(_)) => return Err(ToolError::failed("either path or command, not both")),
            (None, None) => return Err(ToolError::failed("missing path or command")),
            (None, Some(id)) => {
                let id = crate::process::ProcessId(
                    u32::try_from(id).map_err(|_| ToolError::failed("no such background command"))?,
                );
                let (command, text) = ctx
                    .processes
                    .full_output(id)
                    .ok_or_else(|| ToolError::failed(format!("no background command with id {id}")))?;
                (format!("command {id}"), Some(command), text)
            }
            (Some(raw_path), None) => {
                let (label, text) = file_text(ctx, raw_path).await?;
                (label, None, text)
            }
        };
        show(ctx, &input, label, command, &text)
    }
}

/// The text of a file, with the name it is shown under.
async fn file_text(ctx: &ToolCtx, raw_path: &str) -> Result<(String, String), ToolError> {
    {

        if ctx.ws.resolve_new(raw_path).is_ok_and(|abs| crate::picture::is_picture(&abs)) {
            return Err(ToolError::failed_with(
                format!("{raw_path} is an image, not text"),
                "This file is an image, and SendText shows text only.",
            ));
        }

        // Off the executor for the reason `Read` gives: a large file on a slow
        // disk would otherwise hold the whole session.
        let ws = ctx.ws.clone();
        let raw = raw_path.to_string();
        let target = tokio::task::spawn_blocking(move || files::look_in(&ws, &raw))
            .await
            .map_err(|_| ToolError::failed("failed to read file"))??;

        if target.abs.is_dir() {
            return Err(ToolError::failed(format!("{raw_path} is a directory, not a file")));
        }
        let Some(text) = target.current else {
            return Err(ToolError::failed(format!("not found: {raw_path}")));
        };
        Ok((target.rel, text))
    }
}

/// Puts the asked-for lines of `text` in front of the human.
fn show(
    ctx: &ToolCtx,
    input: &Value,
    label: String,
    command: Option<String>,
    text: &str,
) -> Result<ToolOutput, ToolError> {
    let raw_path = label.as_str();
    {
        let all: Vec<&str> = text.lines().collect();
        let total = all.len();
        if total == 0 {
            return Err(ToolError::failed(format!("{raw_path} is empty, nothing to show")));
        }

        let first = input["start_line"].as_u64().unwrap_or(1).max(1) as usize;
        let last = input["end_line"].as_u64().map_or(total, |n| n as usize).min(total);
        if first > total {
            return Err(ToolError::failed(format!(
                "file has {total} lines, requested line {first}"
            )));
        }
        if last < first {
            return Err(ToolError::failed(format!(
                "end_line ({last}) is less than start_line ({first})"
            )));
        }

        let shown = all[first - 1..last].join("\n");
        if shown.len() > MAX_BYTES {
            return Err(ToolError::failed_with(
                format!("{raw_path}: too much text for a single block"),
                format!(
                    "Lines {first}–{last} are {} bytes, and one block holds at most {MAX_BYTES}. \
                     Show a narrower range with start_line and end_line, or tell the human \
                     where the file is.",
                    shown.len()
                ),
            ));
        }

        let lines = last - first + 1;
        ctx.events.emit(AgentEvent::TextShown {
            path: label.clone(),
            command,
            first_line: first as u32,
            lines: lines as u32,
            total: total as u32,
            text: shown,
        });

        // English, unlike other results: the call has no row, so the model is
        // the only one who reads this.
        let what = if lines == total {
            format!("all {total} lines")
        } else {
            format!("lines {first}–{last} of {total}")
        };
        Ok(ToolOutput::new(format!(
            "Shown to the user: {label}, {what}. They are looking at the text itself; \
             do not repeat it in your answer.\n"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::{ctx_for, ctx_with_events};

    #[tokio::test]
    async fn showing_a_file_does_not_count_as_reading_it() {
        let dir = TempDir::new();
        dir.write("a.rs", "one\n");
        let (ctx, ws) = ctx_for(&dir);

        SendText.run(&ctx, json!({ "path": "a.rs" })).await.unwrap();

        // The model has not seen a line of it, so §8 must still refuse an edit.
        assert!(!ws.was_read(&ws.resolve_existing("a.rs").unwrap()));
    }

    #[tokio::test]
    async fn a_range_past_the_end_stops_at_the_end() {
        let dir = TempDir::new();
        dir.write("a.rs", "one\ntwo\nthree\n");
        let (ctx, mut rx) = ctx_with_events(&dir);

        SendText
            .run(&ctx, json!({ "path": "a.rs", "start_line": 2, "end_line": 99 }))
            .await
            .unwrap();

        match rx.try_recv().unwrap() {
            AgentEvent::TextShown { text, lines, .. } => {
                assert_eq!(text, "two\nthree");
                assert_eq!(lines, 2);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_range_that_starts_past_the_end_or_runs_backwards_shows_nothing() {
        let dir = TempDir::new();
        dir.write("a.rs", "one\ntwo\n");
        let (ctx, mut rx) = ctx_with_events(&dir);

        for range in [
            json!({ "path": "a.rs", "start_line": 3 }),
            json!({ "path": "a.rs", "start_line": 2, "end_line": 1 }),
        ] {
            assert!(SendText.run(&ctx, range.clone()).await.is_err(), "{range}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn more_text_than_a_block_holds_is_refused_with_a_way_on() {
        let dir = TempDir::new();
        dir.write("big.log", &format!("{}\n", "x".repeat(999)).repeat(300));
        let (ctx, mut rx) = ctx_with_events(&dir);

        let err = SendText.run(&ctx, json!({ "path": "big.log" })).await.unwrap_err();

        assert!(err.for_model().contains("start_line"), "{}", err.for_model());
        assert!(rx.try_recv().is_err());
        // A part of the same file still goes through.
        SendText
            .run(&ctx, json!({ "path": "big.log", "end_line": 10 }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_binary_file_and_an_image_are_refused() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("bin"), [0u8, 1, 2, 3]).unwrap();
        std::fs::write(dir.path().join("shot.png"), crate::picture::tests::png(8, 8)).unwrap();
        let (ctx, mut rx) = ctx_with_events(&dir);

        for path in ["bin", "shot.png"] {
            assert!(SendText.run(&ctx, json!({ "path": path })).await.is_err(), "{path}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_background_commands_output_can_be_shown() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);
        let id = ctx.processes.register("npm test".into(), ".".into(), None);
        ctx.processes.output(id, vec!["PASS a".into(), "PASS b".into()]);

        let out = SendText.run(&ctx, json!({ "command": id.0 })).await.unwrap();
        assert!(out.text.contains(&format!("command {}", id.0)), "{}", out.text);
        let mut shown = None;
        while let Ok(event) = rx.try_recv() {
            if let AgentEvent::TextShown { text, command, .. } = event {
                shown = Some((text, command));
            }
        }
        assert_eq!(shown, Some(("PASS a\nPASS b".to_string(), Some("npm test".to_string()))));

        // Showing is not reading: the model's cursor has not moved.
        let read = ctx.processes.read(id, 10_000).unwrap();
        assert!(read.text.contains("PASS a"));
    }

    #[tokio::test]
    async fn path_and_command_are_one_or_the_other() {
        let dir = TempDir::new();
        dir.write("a.rs", "x\n");
        let (ctx, _ws) = ctx_for(&dir);
        assert!(SendText.run(&ctx, json!({ "path": "a.rs", "command": 1 })).await.is_err());
        assert!(SendText.run(&ctx, json!({})).await.is_err());
        assert!(SendText.run(&ctx, json!({ "command": 42 })).await.is_err());
    }
}

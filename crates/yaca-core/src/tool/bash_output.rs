//! Reading what a background command has printed since last time.
//!
//! §6.5 describes the panel a human watches; this is the same ring buffer read
//! from the other side. Without it an agent that starts `npm run dev` goes
//! blind: it cannot tell that the server came up, and it cannot tell that the
//! server died. Starting something you can never look at is not a feature.
//!
//! Each line is handed over once. A cursor per process, not a `from` argument:
//! the model asking twice should get what is new, and making it track offsets
//! is asking it to keep state it has no reason to keep.
//!
//! `BashWait` reads the same buffer, after waiting for something worth reading:
//! a line that matches, or the command ending. Without it "wait until the dev
//! server is up" is a loop of `BashOutput` and `sleep`, each round a request.

use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};

use super::bash::MODEL_OUTPUT_MAX_BYTES;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::process::{ProcessId, ProcessState, Waited};

/// How long `BashWait` waits when the model does not say.
const WAIT_DEFAULT_SECS: u64 = 60;
/// The longest one `BashWait` may wait. A turn idle for longer than this is
/// one the human would rather hear from.
const WAIT_MAX_SECS: u64 = 600;

pub struct BashOutput;

#[async_trait]
impl Tool for BashOutput {
    fn name(&self) -> &'static str {
        "BashOutput"
    }

    fn description(&self) -> &'static str {
        "Read what a background command has printed since you last asked, and \
         whether it is still running. Each line comes back once."
    }

    fn properties(&self) -> Value {
        json!({
            "id": {
                "type": "integer",
                "description": "Number the command was given when it started.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["id"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Reads the process's ring buffer — and nothing else.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let number = input["id"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| ToolError::failed("id is required"))?;
        let (text, lines) = read_new(ctx, ProcessId(number))?;
        Ok(ToolOutput::new(text).lines(lines))
    }
}

/// What the command printed since the model last asked, and how it stands:
/// the model's text and the number of lines read.
fn read_new(ctx: &ToolCtx, id: ProcessId) -> Result<(String, u32), ToolError> {
    let read = ctx
        .processes
        .read(id, MODEL_OUTPUT_MAX_BYTES)
        .ok_or_else(|| ToolError::failed(format!("no background command with id {id}")))?;

    // The human pressed "Stop" on this row. Whatever the command managed
    // to print stops mattering: the person wants a word before the agent
    // does anything else, and the turn ends here (§5.5).
    if read.stopped_by_human {
        return Err(ToolError::Interrupted);
    }

    let mut text = String::new();
    if read.missed > 0 {
        // Said rather than hidden: output the buffer lost is otherwise
        // indistinguishable from output the command never produced.
        text.push_str(&format!(
            "[{} lines evicted from buffer and lost]\n",
            read.missed
        ));
    }
    if read.text.is_empty() {
        text.push_str("[no new output]\n");
    } else {
        text.push_str(&read.text);
    }
    text.push_str(state_line(&read.state, read.stopped_by_model));

    Ok((text, read.lines))
}

pub struct BashWait;

#[async_trait]
impl Tool for BashWait {
    fn name(&self) -> &'static str {
        "BashWait"
    }

    fn description(&self) -> &'static str {
        "Wait for a background command instead of polling it: until a line it prints \
         matches `until`, until it ends, or until `timeout` seconds pass — whichever \
         comes first. Then returns what it printed since you last read it, like \
         BashOutput. Use it for \"wait until the server is listening\" (until: \
         \"listening|ready\") or \"wait for the build to finish\" (no until). A \
         line printed before the call that you have not read yet counts too."
    }

    fn properties(&self) -> Value {
        json!({
            "id": {
                "type": "integer",
                "description": "Number the command was given when it started.",
            },
            "until": {
                "type": "string",
                "description": "Regular expression for a line of output to wait for. \
                    Without it, waits for the command to end.",
            },
            "timeout": {
                "type": "integer",
                "minimum": 1,
                "maximum": WAIT_MAX_SECS,
                "description": "Seconds to wait at most. Default 60.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["id"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Waits and reads; changes nothing.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let number = input["id"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| ToolError::failed("id is required"))?;
        let id = ProcessId(number);
        let until = match input["until"].as_str().filter(|u| !u.is_empty()) {
            Some(pattern) => Some(Regex::new(pattern).map_err(|e| {
                ToolError::failed(format!("until is not a valid regular expression: {e}"))
            })?),
            None => None,
        };
        let secs = input["timeout"]
            .as_u64()
            .unwrap_or(WAIT_DEFAULT_SECS)
            .clamp(1, WAIT_MAX_SECS);

        let waited = ctx
            .processes
            .wait_for(id, until.as_ref(), Duration::from_secs(secs), &ctx.cancel)
            .await
            .ok_or_else(|| ToolError::failed(format!("no background command with id {id}")))?;

        let head = match waited {
            Waited::Cancelled => return Err(ToolError::Cancelled),
            Waited::Matched => format!(
                "[output matched /{}/]\n",
                until.as_ref().map_or("", |re| re.as_str())
            ),
            Waited::Ended => match &until {
                Some(re) => format!("[command ended without printing a line matching /{}/]\n", re.as_str()),
                None => "[command ended]\n".to_string(),
            },
            Waited::TimedOut => format!("[still waiting after {secs} s: gave up]\n"),
        };
        let (text, lines) = read_new(ctx, id)?;
        Ok(ToolOutput::new(head + &text).lines(lines))
    }
}

fn state_line(state: &ProcessState, by_model: bool) -> &'static str {
    match state {
        ProcessState::Stopped if by_model => "[command stopped via BashKill]\n",
        ProcessState::Running => "[command is running]\n",
        ProcessState::Exited { code: Some(0) } => "[command completed successfully]\n",
        ProcessState::Exited { code: Some(_) } => "[command exited with error]\n",
        ProcessState::Exited { code: None } => "[command killed by signal]\n",
        ProcessState::Stopped => "[command stopped by human]\n",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn each_line_comes_back_once() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx
            .processes
            .register("npm run dev".into(), ".".into(), None);
        ctx.processes.output(id, vec!["ready".into()]);

        let first = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(first.text.contains("ready"), "{}", first.text);
        assert!(first.text.contains("command is running"), "{}", first.text);

        let second = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(second.text.contains("no new output"), "{}", second.text);
        assert!(!second.text.contains("ready"), "{}", second.text);
    }

    #[tokio::test]
    async fn a_finished_command_says_how_it_ended() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx
            .processes
            .register("cargo test".into(), ".".into(), None);
        ctx.processes.finished(id, Some(1));

        let out = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("with error"), "{}", out.text);
    }

    #[tokio::test]
    async fn a_row_the_human_stopped_ends_the_turn_once() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx
            .processes
            .register("npm run dev".into(), ".".into(), None);
        ctx.processes.output(id, vec!["ready".into()]);

        // The panel's "Stop", then the waiter noticing the process go.
        ctx.processes.stop(id);
        ctx.processes.finished(id, None);

        let err = BashOutput
            .run(&ctx, json!({ "id": id.0 }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Interrupted), "got {err:?}");

        // Only once. A command stopped this morning must not end every turn
        // that glances at it for the rest of the day.
        let out = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("stopped by human"), "{}", out.text);
    }

    #[tokio::test]
    async fn an_unknown_number_is_a_failure_not_an_empty_answer() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = BashOutput.run(&ctx, json!({ "id": 7 })).await.unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "got {err:?}");
    }

    #[test]
    fn reading_a_buffer_asks_no_permission() {
        // The output is already ours; §5.7 has no row for looking at it.
        let dir = TempDir::new();
        let ws = crate::workspace::Workspace::open(dir.path()).unwrap();
        assert!(BashOutput.permission(&ws, &json!({ "id": 1 })).is_none());
    }

    #[tokio::test]
    async fn waiting_stops_at_a_matching_line() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("npm run dev".into(), ".".into(), None);
        ctx.processes.output(id, vec!["compiling".into()]);

        let processes = ctx.processes.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            processes.output(id, vec!["Local: http://localhost:5173".into()]);
        });

        let out = BashWait
            .run(&ctx, json!({ "id": id.0, "until": "localhost:\\d+", "timeout": 5 }))
            .await
            .unwrap();
        assert!(out.text.starts_with("[output matched"), "{}", out.text);
        // Everything unread comes back, the lines before the match included.
        assert!(out.text.contains("compiling") && out.text.contains("5173"), "{}", out.text);
        assert!(out.text.contains("command is running"), "{}", out.text);
    }

    #[tokio::test]
    async fn waiting_without_a_pattern_ends_with_the_command() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("cargo build".into(), ".".into(), None);

        let processes = ctx.processes.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            processes.output(id, vec!["Finished".into()]);
            processes.finished(id, Some(0));
        });

        let out = BashWait.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.starts_with("[command ended]"), "{}", out.text);
        assert!(out.text.contains("completed successfully"), "{}", out.text);
    }

    #[tokio::test]
    async fn a_command_that_ends_before_the_line_says_so() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("npm run dev".into(), ".".into(), None);
        ctx.processes.output(id, vec!["Error: port in use".into()]);
        ctx.processes.finished(id, Some(1));

        let out = BashWait
            .run(&ctx, json!({ "id": id.0, "until": "ready" }))
            .await
            .unwrap();
        assert!(out.text.contains("ended without printing"), "{}", out.text);
        assert!(out.text.contains("port in use"), "{}", out.text);
    }

    #[tokio::test]
    async fn waiting_gives_up_at_the_timeout() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("tail -f log".into(), ".".into(), None);

        let out = BashWait
            .run(&ctx, json!({ "id": id.0, "until": "never", "timeout": 1 }))
            .await
            .unwrap();
        assert!(out.text.starts_with("[still waiting after 1 s"), "{}", out.text);
    }

    #[tokio::test]
    async fn a_bad_pattern_or_number_fails_before_waiting() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("x".into(), ".".into(), None);

        let err = BashWait.run(&ctx, json!({ "id": id.0, "until": "(" })).await.unwrap_err();
        assert!(err.to_string().contains("regular expression"), "{err}");
        let err = BashWait.run(&ctx, json!({ "id": 99 })).await.unwrap_err();
        assert!(err.to_string().contains("no background command"), "{err}");
    }
}

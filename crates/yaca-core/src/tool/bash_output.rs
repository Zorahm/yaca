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

use async_trait::async_trait;
use serde_json::{json, Value};

use super::bash::MODEL_OUTPUT_MAX_BYTES;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::process::{ProcessId, ProcessState};

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

    /// Читает кольцевой буфер процесса — и только.
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
            .ok_or_else(|| ToolError::failed("не задан id"))?;
        let id = ProcessId(number);

        let read = ctx
            .processes
            .read(id, MODEL_OUTPUT_MAX_BYTES)
            .ok_or_else(|| ToolError::failed(format!("нет фоновой команды под номером {id}")))?;

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
                "[{} строк вытеснено из буфера и потеряно]\n",
                read.missed
            ));
        }
        if read.text.is_empty() {
            text.push_str("[нового вывода нет]\n");
        } else {
            text.push_str(&read.text);
        }
        text.push_str(state_line(&read.state, read.stopped_by_model));

        Ok(ToolOutput::new(text).lines(read.lines))
    }
}

fn state_line(state: &ProcessState, by_model: bool) -> &'static str {
    match state {
        ProcessState::Stopped if by_model => "[команда остановлена через BashKill]\n",
        ProcessState::Running => "[команда работает]\n",
        ProcessState::Exited { code: Some(0) } => "[команда завершилась успешно]\n",
        ProcessState::Exited { code: Some(_) } => "[команда завершилась с ошибкой]\n",
        ProcessState::Exited { code: None } => "[команда убита сигналом]\n",
        ProcessState::Stopped => "[команда остановлена человеком]\n",
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
        assert!(first.text.contains("команда работает"), "{}", first.text);

        let second = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(second.text.contains("нового вывода нет"), "{}", second.text);
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
        assert!(out.text.contains("с ошибкой"), "{}", out.text);
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
        assert!(matches!(err, ToolError::Interrupted), "получили {err:?}");

        // Only once. A command stopped this morning must not end every turn
        // that glances at it for the rest of the day.
        let out = BashOutput.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("остановлена человеком"), "{}", out.text);
    }

    #[tokio::test]
    async fn an_unknown_number_is_a_failure_not_an_empty_answer() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = BashOutput.run(&ctx, json!({ "id": 7 })).await.unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "получили {err:?}");
    }

    #[test]
    fn reading_a_buffer_asks_no_permission() {
        // The output is already ours; §5.7 has no row for looking at it.
        let dir = TempDir::new();
        let ws = crate::workspace::Workspace::open(dir.path()).unwrap();
        assert!(BashOutput.permission(&ws, &json!({ "id": 1 })).is_none());
    }
}

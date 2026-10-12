//! Stopping and restarting background commands.
//!
//! `Bash` with `background: true` and `BashOutput` let the model start a dev
//! server and watch it; without these two it could only abandon one, and a
//! server that holds its port forever turns "restart it with the new config"
//! into a task only the human can do from the panel.
//!
//! Neither tool asks permission. Killing touches nothing but what the session
//! itself started. Restarting runs the very command line the human already
//! approved for that row, in the same directory: the tool takes a number, not a
//! command, so it cannot be used to run anything else.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::bash_background::collect;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::invariant;
use crate::process::{kill, shell, ProcessId, ProcessState};

/// How long a kill is given to land. A tree that has not gone in five seconds
/// is not going on its own, and the model should hear that rather than wait.
const STOP_WAIT: Duration = Duration::from_secs(5);

fn process_id(input: &Value) -> Result<ProcessId, ToolError> {
    input["id"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .map(ProcessId)
        .ok_or_else(|| ToolError::failed("не задан id"))
}

/// Kills the command and waits for it to be gone. `false` — it is still there.
async fn stop(ctx: &ToolCtx, id: ProcessId) -> bool {
    let processes = ctx.processes.clone();
    // Killing a tree is `taskkill` on Windows: blocking work.
    let _ = tokio::task::spawn_blocking(move || processes.kill(id)).await;
    ctx.processes.wait_ended(id, STOP_WAIT).await
}

pub struct BashKill;

#[async_trait]
impl Tool for BashKill {
    fn name(&self) -> &'static str {
        "BashKill"
    }

    fn description(&self) -> &'static str {
        "Stop a background command, together with everything it started. Use it \
         for a dev server or watcher that is no longer needed or that holds a \
         port you need. A command that has already ended is reported as such."
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

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let id = process_id(&input)?;
        let summary = ctx
            .processes
            .summary(id)
            .ok_or_else(|| ToolError::failed(format!("нет фоновой команды под номером {id}")))?;

        if summary.state != ProcessState::Running {
            return Ok(ToolOutput::new(format!(
                "Команда {id} уже не работает.\n"
            )));
        }
        if !stop(ctx, id).await {
            return Err(ToolError::failed(format!(
                "команда {id} не остановилась за {} с",
                STOP_WAIT.as_secs()
            )));
        }
        Ok(ToolOutput::new(format!("Команда {id} остановлена.\n")))
    }
}

pub struct BashRestart;

#[async_trait]
impl Tool for BashRestart {
    fn name(&self) -> &'static str {
        "BashRestart"
    }

    fn description(&self) -> &'static str {
        "Stop a background command if it is still running and start the same \
         command line again in the same directory. The number stays the same, \
         and what the previous run printed is still there to be read before the \
         new output. Use it after changing config or code a dev server does not \
         pick up on its own, or to bring back one that crashed."
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

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let id = process_id(&input)?;
        let summary = ctx
            .processes
            .summary(id)
            .ok_or_else(|| ToolError::failed(format!("нет фоновой команды под номером {id}")))?;
        // Restarting runs the command again without a question, which is only
        // sound for commands that were never questions of their own (§8). A
        // commit or a push is asked about every time it runs, however it got
        // started, so it is run again through `Bash`, where the question is.
        if invariant::of_command(&summary.command).is_some() {
            return Err(ToolError::failed(format!(
                "команду {id} нельзя перезапустить: коммит и пуш запускаются                  только через Bash, с вопросом человеку"
            )));
        }
        // Resolved before anything is killed: a directory that is gone should
        // leave the running command alone rather than end it and then fail.
        let dir = ctx.ws.resolve_existing(&summary.cwd)?;

        if summary.state == ProcessState::Running && !stop(ctx, id).await {
            return Err(ToolError::failed(format!(
                "команда {id} не остановилась за {} с, перезапуск отменён",
                STOP_WAIT.as_secs()
            )));
        }

        let head = summary.command.split_whitespace().next().unwrap_or("");
        let shell::Spawned { child, pid, output } = shell::spawn(ctx.shell, &summary.command, &dir)
            .map_err(|e| ToolError::failed(format!("не удалось запустить «{head}»: {e}")))?;

        if !ctx.processes.restart(id, pid) {
            // Someone else restarted it first, or the row is gone. The process
            // just started belongs to nobody and must not be left running.
            if let Some(pid) = pid {
                let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
            }
            return Err(ToolError::failed(format!(
                "команда {id} уже запущена заново"
            )));
        }
        tokio::spawn(collect(ctx.processes.downgrade(), id, child, output));

        Ok(ToolOutput::new(format!(
            "Команда {id} перезапущена. Читать её вывод — BashOutput.\n"
        ))
        .lines(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::bash::Bash;
    use crate::tool::tests_support::ctx_for;

    fn sleeps() -> &'static str {
        if cfg!(windows) {
            "ping -n 30 127.0.0.1 >nul"
        } else {
            "sleep 30"
        }
    }

    async fn start(ctx: &ToolCtx, command: &str) -> ProcessId {
        Bash.run(ctx, json!({ "command": command, "background": true }))
            .await
            .unwrap();
        ctx.processes.summaries().last().unwrap().id
    }

    #[tokio::test]
    async fn killing_ends_the_command_without_ending_the_turn() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = start(&ctx, sleeps()).await;

        let out = BashKill.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("остановлена"), "{}", out.text);
        assert_eq!(ctx.processes.summary(id).unwrap().state, ProcessState::Stopped);

        // It was the model's own act: reading the row afterwards is not an
        // interruption, and it is not news either.
        let read = ctx.processes.read(id, 1000).unwrap();
        assert!(read.stopped_by_model);
        assert!(!read.stopped_by_human);
        assert!(ctx.processes.take_notices().is_empty());
    }

    #[tokio::test]
    async fn killing_a_finished_command_says_so() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx.processes.register("cargo test".into(), ".".into(), None);
        ctx.processes.finished(id, Some(0));

        let out = BashKill.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("уже не работает"), "{}", out.text);
    }

    #[tokio::test]
    async fn an_unknown_number_is_a_failure() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        for err in [
            BashKill.run(&ctx, json!({ "id": 9 })).await.unwrap_err(),
            BashRestart.run(&ctx, json!({ "id": 9 })).await.unwrap_err(),
        ] {
            assert!(matches!(err, ToolError::Failed { .. }), "получили {err:?}");
        }
    }

    #[tokio::test]
    async fn restarting_keeps_the_number_and_runs_the_same_command_again() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = start(&ctx, sleeps()).await;
        ctx.processes.output(id, vec!["first run".into()]);

        let out = BashRestart.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(out.text.contains("перезапущена"), "{}", out.text);

        // One row, running again, under the same number.
        let panel = ctx.processes.summaries();
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].id, id);
        assert_eq!(panel[0].state, ProcessState::Running);
        assert!(panel[0].finished_at.is_none());

        // What the old run printed has not been lost, and the runs are told
        // apart.
        let read = ctx.processes.read(id, 10_000).unwrap();
        assert!(read.text.contains("first run"), "{}", read.text);
        assert!(read.text.contains("перезапуск"), "{}", read.text);
        assert!(!read.stopped_by_human && !read.stopped_by_model);

        BashKill.run(&ctx, json!({ "id": id.0 })).await.unwrap();
    }

    #[tokio::test]
    async fn a_command_that_died_can_be_brought_back() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = start(&ctx, "echo hi").await;
        assert!(ctx.processes.wait_ended(id, Duration::from_secs(10)).await);
        assert_eq!(
            ctx.processes.summary(id).unwrap().state,
            ProcessState::Exited { code: Some(0) }
        );

        BashRestart.run(&ctx, json!({ "id": id.0 })).await.unwrap();
        assert!(ctx.processes.wait_ended(id, Duration::from_secs(10)).await);

        let read = ctx.processes.read(id, 10_000).unwrap();
        assert_eq!(read.text.matches("hi").count(), 2, "{}", read.text);
    }

    #[tokio::test]
    async fn a_command_that_is_an_invariant_is_not_restarted_unasked() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let id = ctx
            .processes
            .register("git -C . push origin main".into(), ".".into(), None);
        ctx.processes.finished(id, Some(0));

        let err = BashRestart.run(&ctx, json!({ "id": id.0 })).await.unwrap_err();
        assert!(err.to_string().contains("нельзя перезапустить"), "{err}");
        // Nothing was started: the row is still the finished one.
        assert_eq!(
            ctx.processes.summary(id).unwrap().state,
            ProcessState::Exited { code: Some(0) }
        );
    }

    #[test]
    fn neither_tool_asks_permission() {
        let dir = TempDir::new();
        let ws = crate::workspace::Workspace::open(dir.path()).unwrap();
        assert!(BashKill.permission(&ws, &json!({ "id": 1 })).is_none());
        assert!(BashRestart.permission(&ws, &json!({ "id": 1 })).is_none());
    }
}

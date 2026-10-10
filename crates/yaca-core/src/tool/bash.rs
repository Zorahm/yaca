//! Running a command.
//!
//! Output reaches the feed while the command is still running (§15,
//! `ToolCallProgress`), not in one piece afterwards: a two-minute test run that
//! shows nothing until it ends is an agent working out of sight, and §0 says
//! the agent works in full view.
//!
//! The model and the human get different things out of the same run. The human
//! gets everything, streamed, with §5.5.1 showing the last twenty lines and the
//! rest behind a button. The model gets the head and the tail within a byte
//! budget: the middle of a build log is a list of files, and paying for it in
//! tokens buys nothing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Child;
use tokio::sync::mpsc;

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::checkpoint::CheckpointReason;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::process::output::{clamp, Lines, Ring, Utf8Stream, RING_LINES};
use crate::process::{display_cwd, kill, shell, ProcessId, WeakProcesses};
use crate::workspace::Workspace;

/// A foreground command that has not finished in two minutes is usually not a
/// command but a server. The message that ends it says so, which is how the
/// model learns `background` exists without being told twice.
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// How much of the output the model sees. Shared with `bash_output`:
/// the budget belongs to the model, not to one way of reaching it.
pub(crate) const MODEL_OUTPUT_MAX_BYTES: usize = 30_000;

pub struct Bash;

#[async_trait]
impl Tool for Bash {
    fn name(&self) -> &'static str {
        "Bash"
    }

    fn description(&self) -> &'static str {
        "Run a command in the project. Its output reaches the human as it appears. \
         Anything that does not end by itself — a dev server, a watcher, a long \
         build you want to keep an eye on — belongs in the background rather than \
         behind a longer timeout."
    }

    fn properties(&self) -> Value {
        json!({
            "command": {
                "type": "string",
                "description": "The command line to run.",
            },
            "cwd": {
                "type": "string",
                "description": "Directory to run in, relative to the project root. \
                                Defaults to the root.",
            },
            "timeout_ms": {
                "type": "integer",
                "description": "Stop the command after this long. Default 120000, \
                                ceiling 600000. Ignored for background commands.",
            },
            "background": {
                "type": "boolean",
                "description": "Start the command and return at once. Read what it \
                                has printed since with BashOutput.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["command"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Agent only. Ask and plan promise to change nothing, and a command can
    /// change anything — the core cannot tell `cargo tree` from `rm -rf`. The
    /// tool is therefore absent from those modes rather than forbidden by a
    /// sentence in the prompt, which a weak model would step over (§7.1).
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    /// §5.5.1: the first line of the expanded row is the full command.
    fn target(&self, input: &Value) -> Option<String> {
        command_of(input).map(str::to_owned)
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let command = command_of(input)?;
        // §5.7, row "Команда": the target is the first word, the detail is the
        // whole line, and the dimmed second line is the working directory. The
        // detail is never shortened here — it is the place a human notices
        // `&& rm -rf`, and shortening happens in the feed if anywhere.
        //
        // The directory goes in whole, unlike the panel's relative column: the
        // question is about to let a command loose somewhere, and `.` does not
        // say where.
        let cwd = working_dir(ws, input).unwrap_or_else(|_| ws.root().to_path_buf());
        Some(
            PermissionNeed::new(PermissionKind::RunCommand, self.name(), head(command))
                .detail(command)
                .note(cwd.to_string_lossy())
                .command(command),
        )
    }

    /// §5.8: a snapshot before every permitted command. This is the one tool
    /// that declares it — everything else that reaches the disk goes through
    /// the change set, and the change set is snapshotted where it is applied.
    ///
    /// Background commands included. A watcher rewrites files for as long as it
    /// lives, and the point worth coming back to is the one before it started.
    fn checkpoint(&self, input: &Value) -> Option<CheckpointReason> {
        Some(CheckpointReason::Command {
            command: command_of(input)?.to_string(),
        })
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let command = command_of(&input)
            .ok_or_else(|| ToolError::failed("не задан command"))?
            .to_string();
        let cwd = working_dir(ctx.ws.as_ref(), &input)?;

        if input["background"].as_bool().unwrap_or(false) {
            return background(ctx, &command, &cwd);
        }
        foreground(ctx, &command, &cwd, timeout_of(&input)).await
    }
}

fn command_of(input: &Value) -> Option<&str> {
    input["command"]
        .as_str()
        .map(str::trim)
        .filter(|c| !c.is_empty())
}

/// The first word — what §5.7 shows as the target and what an "always allow"
/// rule keys on.
fn head(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn timeout_of(input: &Value) -> Duration {
    let ms = input["timeout_ms"]
        .as_u64()
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1, MAX_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// Resolves `cwd` and keeps it inside the project. The command itself can walk
/// out with `cd ..`; what the tool can enforce, it enforces (§8).
fn working_dir(ws: &Workspace, input: &Value) -> Result<PathBuf, ToolError> {
    let Some(raw) = input["cwd"].as_str().filter(|s| !s.trim().is_empty()) else {
        return Ok(ws.root().to_path_buf());
    };
    let dir = ws.resolve_existing(raw)?;
    if !dir.is_dir() {
        return Err(ToolError::failed(format!("{raw} — не каталог")));
    }
    Ok(dir)
}

enum Ending {
    /// Both pipes reached EOF: the command has said everything it will say.
    Done,
    TimedOut,
    Cancelled,
}

async fn foreground(
    ctx: &ToolCtx,
    command: &str,
    cwd: &Path,
    limit: Duration,
) -> Result<ToolOutput, ToolError> {
    let spawned = shell::spawn(command, cwd)
        .map_err(|e| ToolError::failed(format!("не удалось запустить «{}»: {e}", head(command))))?;
    let shell::Spawned {
        mut child,
        pid,
        mut output,
    } = spawned;

    let mut ring = Ring::new(RING_LINES);
    let mut bytes = Utf8Stream::default();
    let mut lines = Lines::default();

    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);

    let ending = loop {
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => break Ending::Cancelled,
            _ = &mut deadline => break Ending::TimedOut,
            chunk = output.recv() => match chunk {
                Some(raw) => {
                    let text = bytes.push(&raw);
                    for line in lines.push(&text) {
                        ring.push(line);
                    }
                    ctx.progress.push(text);
                }
                None => break Ending::Done,
            },
        }
    };

    let tail = bytes.flush();
    if !tail.is_empty() {
        for line in lines.push(&tail) {
            ring.push(line);
        }
        ctx.progress.push(tail);
    }
    if let Some(last) = lines.flush() {
        ring.push(last);
    }

    if !matches!(ending, Ending::Done) {
        if let Some(pid) = pid {
            let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
        }
        // Reaped even though the answer is discarded: a child nobody waits on
        // stays in the table as a zombie.
        let _ = child.wait().await;
    }

    let produced = ring.next_index();
    let (text, truncated) = clamp(&ring.lines(), MODEL_OUTPUT_MAX_BYTES);

    match ending {
        Ending::Cancelled => return Err(ToolError::Cancelled),
        Ending::TimedOut => {
            return Err(ToolError::failed_with(
                format!(
                    "«{}» не уложилась в {} с и остановлена",
                    head(command),
                    limit.as_secs()
                ),
                format!(
                    "{text}\n[остановлена по таймауту {} с; если это сервер или \
                     наблюдатель, запусти её с background: true]",
                    limit.as_secs()
                ),
            ))
        }
        Ending::Done => {}
    }

    let status = child
        .wait()
        .await
        .map_err(|e| ToolError::failed(format!("не дождался «{}»: {e}", head(command))))?;

    let body = if text.is_empty() {
        "[команда ничего не вывела]\n".to_string()
    } else {
        text
    };
    let out = ToolOutput::new(body)
        .lines(produced.min(u32::MAX as u64) as u32)
        .truncated(truncated);

    if status.success() {
        return Ok(out);
    }
    Err(ToolError::failed_with(
        match status.code() {
            Some(code) => format!("«{}» завершилась с кодом {code}", head(command)),
            None => format!("«{}» убита сигналом", head(command)),
        },
        out.text,
    ))
}

/// Starts the command and leaves it in the process panel (§6.5).
fn background(ctx: &ToolCtx, command: &str, cwd: &Path) -> Result<ToolOutput, ToolError> {
    let spawned = shell::spawn(command, cwd)
        .map_err(|e| ToolError::failed(format!("не удалось запустить «{}»: {e}", head(command))))?;
    let shell::Spawned { child, pid, output } = spawned;

    let id = ctx
        .processes
        .register(command.to_string(), display_cwd(ctx.ws.root(), cwd), pid);
    tokio::spawn(collect(ctx.processes.downgrade(), id, child, output));

    Ok(ToolOutput::new(format!(
        "Запущена в фоне под номером {id}. Читать её вывод — BashOutput, \
         остановить — из панели процессов.\n"
    ))
    .lines(1))
}

/// Feeds a background command's output into its ring buffer until it ends.
///
/// Holds the registry weakly on purpose. A strong handle would keep the
/// session's processes alive for as long as `npm run dev` runs — that is,
/// forever — and the drop that kills them at the end of a session would never
/// fire. Here the session going away closes the pipes instead, and this task
/// notices and leaves.
async fn collect(
    processes: WeakProcesses,
    id: ProcessId,
    mut child: Child,
    mut output: mpsc::Receiver<Vec<u8>>,
) {
    let mut bytes = Utf8Stream::default();
    let mut lines = Lines::default();

    while let Some(raw) = output.recv().await {
        let text = bytes.push(&raw);
        let Some(processes) = processes.upgrade() else {
            return;
        };
        processes.output(id, lines.push(&text));
    }

    let mut last = lines.push(&bytes.flush());
    last.extend(lines.flush());
    let code = child.wait().await.ok().and_then(|s| s.code());

    if let Some(processes) = processes.upgrade() {
        processes.output(id, last);
        processes.finished(id, code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessState;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::{ctx_for, ctx_with_events};
    use crate::AgentEvent;

    /// The shell differs by platform, so the scripts the tests run do too.
    /// Written out rather than abstracted: two lines of `cfg!` beat a helper
    /// that hides which shell is actually under test.
    fn two_bursts() -> &'static str {
        if cfg!(windows) {
            "echo one; Start-Sleep -Milliseconds 400; echo two"
        } else {
            "echo one; sleep 0.4; echo two"
        }
    }

    fn sleeps() -> &'static str {
        if cfg!(windows) {
            "Start-Sleep -Seconds 20"
        } else {
            "sleep 20"
        }
    }

    #[tokio::test]
    async fn output_and_exit_code_reach_the_model() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = Bash
            .run(&ctx, json!({ "command": "echo hello" }))
            .await
            .unwrap();

        assert!(out.text.contains("hello"), "получили: {}", out.text);
        assert_eq!(out.meta.lines, 1);
    }

    #[tokio::test]
    async fn a_command_that_printed_nothing_says_so() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = Bash.run(&ctx, json!({ "command": "cd ." })).await.unwrap();

        assert!(out.text.contains("ничего не вывела"), "{}", out.text);
    }

    #[tokio::test]
    async fn output_streams_before_the_command_ends() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);

        Bash.run(&ctx, json!({ "command": two_bursts() }))
            .await
            .unwrap();

        let mut chunks = Vec::new();
        while let Ok(AgentEvent::ToolCallProgress { chunk, .. }) = rx.try_recv() {
            chunks.push(chunk);
        }

        // The point of the whole design: `one` and `two` are separate events.
        // Handed over in one piece after the command finished, they would sit
        // in the same chunk — which is what this test exists to forbid.
        let first = chunks
            .iter()
            .position(|c| c.contains("one"))
            .expect("«one» не доехало в ленту");
        let second = chunks
            .iter()
            .position(|c| c.contains("two"))
            .expect("«two» не доехало в ленту");
        assert!(first < second, "вывод приехал одним куском: {chunks:?}");
    }

    #[tokio::test]
    async fn a_non_zero_exit_is_a_failed_action_that_still_carries_the_output() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = Bash
            .run(&ctx, json!({ "command": "echo broken; exit 3" }))
            .await
            .unwrap_err();

        let ToolError::Failed { message, detail } = err else {
            panic!("ожидали неудачу, получили {err:?}");
        };
        // §5.5: the feed gets a line, not five hundred lines of build log.
        assert!(message.contains("кодом 3"), "{message}");
        assert!(!message.contains("broken"), "{message}");
        // The model gets the output, or it has nothing to fix.
        assert!(detail.unwrap_or_default().contains("broken"));
    }

    #[tokio::test]
    async fn a_command_that_overruns_is_stopped_and_told_about_the_background() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = Bash
            .run(&ctx, json!({ "command": sleeps(), "timeout_ms": 300 }))
            .await
            .unwrap_err();

        let ToolError::Failed { message, detail } = err else {
            panic!("ожидали неудачу, получили {err:?}");
        };
        assert!(message.contains("не уложилась"), "{message}");
        assert!(detail.unwrap_or_default().contains("background"));
    }

    #[tokio::test]
    async fn cancelling_the_turn_stops_the_command() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let cancel = ctx.cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            cancel.cancel();
        });

        let err = Bash
            .run(&ctx, json!({ "command": sleeps() }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Cancelled), "получили {err:?}");
    }

    #[tokio::test]
    async fn a_background_command_returns_at_once_and_lands_in_the_panel() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let started = std::time::Instant::now();
        let out = Bash
            .run(&ctx, json!({ "command": sleeps(), "background": true }))
            .await
            .unwrap();

        assert!(
            started.elapsed() < Duration::from_secs(3),
            "фоновая команда не должна ждать завершения"
        );
        assert!(out.text.contains("в фоне"), "{}", out.text);

        let panel = ctx.processes.summaries();
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].state, ProcessState::Running);
        assert_eq!(panel[0].cwd, ".");

        ctx.processes.stop(panel[0].id);
    }

    #[tokio::test]
    async fn output_of_a_background_command_collects_in_its_buffer() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        Bash.run(&ctx, json!({ "command": two_bursts(), "background": true }))
            .await
            .unwrap();

        let id = ctx.processes.summaries()[0].id;
        // Nothing to wait on but the process itself: the panel is the only
        // place its output goes.
        for _ in 0..60 {
            if ctx.processes.summaries()[0].state != ProcessState::Running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let read = ctx.processes.read(id, 10_000).unwrap();
        assert!(read.text.contains("one"), "получили: {}", read.text);
        assert!(read.text.contains("two"), "получили: {}", read.text);
        assert_eq!(read.state, ProcessState::Exited { code: Some(0) });
    }

    #[tokio::test]
    async fn the_working_directory_stays_inside_the_project() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = Bash
            .run(&ctx, json!({ "command": "echo hi", "cwd": "../.." }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
    }

    #[test]
    fn the_question_names_the_command_and_the_directory() {
        let dir = TempDir::new();
        dir.mkdir("crates");
        let ws = Workspace::open(dir.path()).unwrap();

        let need = Bash
            .permission(
                &ws,
                &json!({ "command": "npm ci --prefer-offline", "cwd": "crates" }),
            )
            .unwrap();

        assert_eq!(need.kind, PermissionKind::RunCommand);
        assert_eq!(need.target, "npm");
        assert_eq!(need.detail, "npm ci --prefer-offline");
        assert_eq!(
            need.note.as_deref(),
            Some(dir.path().join("crates").to_string_lossy().as_ref())
        );
        // An "always allow" rule keys on the first word (§5.7).
        assert_eq!(need.command.as_deref(), Some("npm ci --prefer-offline"));
    }

    #[test]
    fn a_missing_command_asks_nothing_and_the_tool_says_why() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(Bash.permission(&ws, &json!({ "command": "   " })).is_none());
    }
}

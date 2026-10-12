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

use super::bash_background::{background, to_background, Waited};
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::checkpoint::CheckpointReason;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{invariant, readonly, unsaved, PermissionKind, PermissionNeed};
use crate::process::output::{clamp, Lines, Ring, Utf8Stream, RING_LINES};
use crate::process::{kill, shell};
use crate::workspace::Workspace;

/// A foreground command that has not finished in two minutes is usually not a
/// command but a server, or a build the model should not sit and wait on. It
/// is not killed: it moves to the background and the result says so, which is
/// how the model learns `background` exists without being told twice.
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
         behind a longer timeout. A command still running after the timeout is not \
         stopped: it moves to the background by itself, and when it ends you are \
         given a file with its whole output. In plan mode only commands that change \
         nothing run (listing, reading, searching, read-only git)."
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
                "description": "How long to wait for the command, in milliseconds. \
                                Default 120000, ceiling 600000. A command still \
                                running after this is not stopped: it moves to the \
                                background and you are told so. Ignored for \
                                commands started with background.",
            },
            "background": {
                "type": "boolean",
                "description": "Start the command and return at once. Read what it \
                                has printed since with BashOutput. Stop it with \
                                BashKill, restart it with BashRestart.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["command"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Offered in Plan too, because reading the project is most of what a plan
    /// needs from a shell (`git status`, `rg`). The core cannot tell `cargo
    /// tree` from `rm -rf` by name, so each call is judged in
    /// [`Tool::plan_check`] and refused there when it may write (§7.1).
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    /// Plan lets through the commands the classifier can vouch for, and only
    /// those (§7.1). The refusal names the way out, because the model will read
    /// it and try again.
    fn plan_check(&self, ws: &Workspace, input: &Value) -> Result<(), String> {
        let Some(command) = command_of(input) else {
            return Ok(());
        };
        let cwd = working_dir(ws, input).unwrap_or_else(|_| ws.root().to_path_buf());
        if readonly::is_read_only_in(command, &search_dirs(ws, &cwd)) {
            return Ok(());
        }
        Err(format!(
            "В плане можно только читать: «{}» может что-то изменить. Чтение, \
             поиск и листинг выполняются и здесь; запуск команды — в режиме «Ручной» \
             или «Авто».",
            head(command)
        ))
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

        // §8: a commit or push is put to the human in every mode, whatever the
        // command is wrapped in. Checked first, so a read-only shortcut can
        // never skip it.
        if let Some(invariant) = invariant::of_command(command) {
            return Some(
                PermissionNeed::new(PermissionKind::RunCommand, self.name(), head(command))
                    .detail(command)
                    .note(cwd.to_string_lossy())
                    .command(command)
                    .invariant(invariant),
            );
        }

        // A command that changes nothing runs without a question (§7.1, §8).
        if readonly::is_read_only_in(command, &search_dirs(ws, &cwd)) {
            return None;
        }

        let mut need = PermissionNeed::new(PermissionKind::RunCommand, self.name(), head(command))
            .detail(command)
            .note(cwd.to_string_lossy())
            .command(command);
        // What a rollback would not bring back: the command may write into any
        // additional root, and no line of it says which (`unsaved::command_reach`).
        if let Some(reach) = unsaved::command_reach(ws) {
            need = need.unsaved(reach);
        }
        Some(need)
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
pub(super) fn head(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn timeout_of(input: &Value) -> Duration {
    let ms = input["timeout_ms"]
        .as_u64()
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1, MAX_TIMEOUT_MS);
    Duration::from_millis(ms)
}

/// The directories a program name may be resolved in before `PATH` (§8): the
/// working directory and every root. A program that sits in one of them shadows
/// the one `PATH` would have run.
fn search_dirs<'a>(ws: &'a Workspace, cwd: &'a Path) -> Vec<&'a Path> {
    let mut dirs = vec![cwd];
    dirs.extend(ws.roots());
    dirs
}

/// Resolves `cwd` and keeps it inside a root: the primary one, or any additional
/// one. A relative `cwd` is taken from the primary root. The command itself can
/// walk out with `cd ..`; what the tool can enforce, it enforces (§8).
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
    /// Still running when the wait ran out. That ends the wait, not the
    /// command: it is handed to the background.
    Overran,
    Cancelled,
}

async fn foreground(
    ctx: &ToolCtx,
    command: &str,
    cwd: &Path,
    limit: Duration,
) -> Result<ToolOutput, ToolError> {
    let spawned = shell::spawn(ctx.shell, command, cwd)
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
            _ = &mut deadline => break Ending::Overran,
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

    // Before the tails are flushed: a half-written line belongs to the
    // collector, which will finish it rather than split it in two.
    if matches!(ending, Ending::Overran) {
        let waited = Waited {
            command,
            cwd,
            limit,
            pid,
        };
        return Ok(to_background(ctx, waited, child, output, ring, bytes, lines).await);
    }

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

    if matches!(ending, Ending::Cancelled) {
        if let Some(pid) = pid {
            let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
        }
        // Reaped even though the answer is discarded: a child nobody waits on
        // stays in the table as a zombie.
        let _ = child.wait().await;
    }

    let produced = ring.next_index();
    let (text, truncated) = clamp(&ring.lines(), MODEL_OUTPUT_MAX_BYTES);

    if matches!(ending, Ending::Cancelled) {
        return Err(ToolError::Cancelled);
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
    async fn a_command_that_overruns_moves_to_the_background_instead_of_dying() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = Bash
            .run(&ctx, json!({ "command": sleeps(), "timeout_ms": 300 }))
            .await
            .unwrap();

        assert!(out.text.contains("NOT stopped"), "{}", out.text);
        assert!(out.text.contains("BashOutput"), "{}", out.text);

        let panel = ctx.processes.summaries();
        assert_eq!(panel.len(), 1);
        assert_eq!(panel[0].state, ProcessState::Running);

        ctx.processes.stop(panel[0].id);
    }

    #[tokio::test]
    async fn a_command_that_moved_leaves_its_whole_output_in_a_file_when_it_ends() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        // Prints before the wait runs out and after it: both belong in the
        // file, in order, and BashOutput must not hand `one` over a second
        // time — the result of the call already did.
        let command = if cfg!(windows) {
            "echo one; Start-Sleep -Milliseconds 1500; echo two"
        } else {
            "echo one; sleep 1.5; echo two"
        };
        let out = Bash
            .run(&ctx, json!({ "command": command, "timeout_ms": 700 }))
            .await
            .unwrap();
        assert!(out.text.contains("NOT stopped"), "{}", out.text);
        // Under a loaded machine the shell may start after the wait ran out,
        // and `one` then arrives through the collector instead.
        let given = out.text.contains("one");

        let id = ctx.processes.summaries()[0].id;
        assert!(ctx.processes.wait_ended(id, Duration::from_secs(15)).await);

        let notices = ctx.processes.take_notices();
        assert_eq!(notices.len(), 1);
        let file = notices[0].file.clone().expect("no output file");
        let text = std::fs::read_to_string(&file).unwrap();
        let one = text.find("one").expect("`one` missing from the file");
        let two = text.find("two").expect("`two` missing from the file");
        assert!(one < two, "{text}");

        let read = ctx.processes.read(id, 10_000).unwrap();
        assert!(
            !given || !read.text.contains("one"),
            "read again: {}",
            read.text
        );
        assert!(read.text.contains("two"), "{}", read.text);
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

    #[test]
    fn a_read_only_command_asks_nothing() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        for command in ["git status", "ls -la", "rg TODO src"] {
            let asked = Bash.permission(&ws, &json!({ "command": command }));
            assert!(asked.is_none(), "{command}");
        }
    }

    #[test]
    fn a_push_is_an_invariant_even_when_it_looks_read_only() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let need = Bash
            .permission(&ws, &json!({ "command": "echo ok && git push" }))
            .unwrap();
        assert_eq!(need.invariant, Some(crate::permission::Invariant::GitPush));
        assert!(need.scopes(ws.root()).is_empty());
    }

    #[test]
    fn plan_lets_read_only_commands_through_and_names_the_way_out_for_the_rest() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let read = json!({ "command": "git log -3" });
        assert!(Bash.plan_check(&ws, &read).is_ok());

        let refusal = Bash
            .plan_check(&ws, &json!({ "command": "npm ci" }))
            .unwrap_err();
        assert!(refusal.contains("В плане"), "{refusal}");
        assert!(refusal.contains("Ручной"), "{refusal}");
    }

    #[test]
    fn bash_is_offered_in_plan_and_agent() {
        assert_eq!(Bash.intents(), &[Intent::Plan, Intent::Agent]);
    }
}

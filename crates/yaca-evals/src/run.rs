//! One task, one session, one verdict.
//!
//! The session is the same one `yaca` opens, driven through the public
//! surface (`spawn`, `Command`, `AgentEvent`) so an eval exercises exactly what
//! the interface runs. What differs from a person at the keyboard is spelled
//! out where it happens: the answers nobody is there to give.
//!
//! Invariants: bypass lets through everything the trust matrix would ask
//! about, but not what §8 keeps behind an explicit request (a `git commit` or
//! `git push` nobody asked for). Those still arrive as permission requests,
//! and this runner answers them with a refusal, the same answer a person
//! would be forced to give by walking away. An eval can therefore never commit
//! or push, whatever a prompt says.

use std::path::Path;
use std::time::{Duration, Instant};

use tokio::sync::broadcast::{error::RecvError, Receiver};
use yaca_core::event::AgentEvent;
use yaca_core::question::QuestionReply;
use yaca_core::{Command, Mode, PermissionAnswer, SessionConfig, SessionHandle};

use crate::check;
use crate::observe::Observed;
use crate::provider::Plan;
use crate::result::TaskResult;
use crate::task::{Task, TaskMode};
use crate::workdir::{copy_tree, create_unique, WorkDir};

/// Sent to the model with the refusal. Without it the agent tries the same
/// commit again; with it, the agent is told that nobody will say yes.
const DENY_REASON: &str =
    "This is an unattended evaluation run: nobody is available to approve this. \
     Do not retry; carry on without it.";

/// After a timeout, how long the core gets to acknowledge the cancel before the
/// run gives up on the session. Cancelling a tool that is mid-command can take
/// a moment to settle.
const CANCEL_GRACE: Duration = Duration::from_secs(60);

pub struct RunOptions<'a> {
    /// Parent directory for the run's trees.
    pub work_root: &'a Path,
    /// Leave the tree on disk after the run, to look at what the agent did.
    pub keep_workdirs: bool,
}

/// A finished run: its result, and every event it produced as one JSON line.
pub struct RunRecord {
    pub result: TaskResult,
    pub events: Vec<String>,
}

pub async fn run_task(task: &Task, plan: &Plan, opts: &RunOptions<'_>) -> RunRecord {
    let mut result = TaskResult::new(&task.id, &plan.provider_id, &plan.model, mode_name(task.mode));
    let mut observed = Observed::default();

    let tree = match prepare_tree(task, opts) {
        Ok(tree) => tree,
        Err(e) => {
            // Without a tree there is nothing to run or check. The run still
            // produces a result, so the summary shows the task and why.
            result.failure = Some(format!("cannot prepare the tree: {e:#}"));
            result.judge();
            return RunRecord { result, events: Vec::new() };
        }
    };

    let started = Instant::now();
    let stopped = drive(task, plan, tree.path(), &mut observed, &mut result).await;
    result.wall_ms = started.elapsed().as_millis() as u64;

    // The check runs after the session has ended and before the tree goes:
    // it judges what the agent left on disk, not what it said it did.
    result.checks = check::run_check(tree.path(), &task.expect, task.check.as_ref()).await;

    result.stop_reason = match stopped {
        Stopped::NeverStarted => "setupFailed".into(),
        Stopped::TimedOut => "timeout".into(),
        Stopped::Ended => observed.ending.clone().unwrap_or_else(|| "closed".into()),
    };
    result.turns = observed.turns;
    result.tool_calls_total = observed.total_tool_calls();
    result.tool_calls = observed.tool_calls.clone();
    result.tokens = observed.tokens;
    result.cost_usd = observed.cost_usd;
    result.permissions_denied = observed.permissions_denied;
    result.questions_dismissed = observed.questions_dismissed;
    result.errors = observed.errors.clone();
    result.judge();

    RunRecord { result, events: observed.log }
}

fn prepare_tree(task: &Task, opts: &RunOptions<'_>) -> anyhow::Result<WorkDir> {
    let path = create_unique(opts.work_root, &task.id)?;
    let tree = WorkDir::new(path, opts.keep_workdirs);
    copy_tree(&task.repo, tree.path())?;
    Ok(tree)
}

/// Runs the session until the turn ends, the timeout hits, or the session
/// goes away. Errors go into `result` and `observed`; nothing here is fatal to
/// the run as a whole.
async fn drive(
    task: &Task,
    plan: &Plan,
    root: &Path,
    observed: &mut Observed,
    result: &mut TaskResult,
) -> Stopped {
    let mut config = SessionConfig::new(root);
    config.provider = plan.provider_id.clone();
    config.model = plan.model.clone();
    config.models = plan.models.clone();
    config.mode = session_mode(task.mode);
    config.journal = None;

    let handle = match yaca_core::spawn(config, plan.provider.clone()) {
        Ok(handle) => handle,
        Err(e) => {
            result.failure = Some(format!("cannot start the session: {e}"));
            return Stopped::NeverStarted;
        }
    };
    let mut events = handle.subscribe();

    // Bypass is the human's choice, made by starting this run. It is switched
    // on before the message, so the first write is not already asked about.
    if handle.send(Command::SetBypass { on: true }).await.is_err()
        || handle
            .send(Command::SendMessage { text: task.prompt.clone(), attachments: Vec::new() })
            .await
            .is_err()
    {
        observed.errors.push("the session stopped before the message was sent".into());
        return Stopped::NeverStarted;
    }

    let mut timed_out = false;
    match tokio::time::timeout(task.timeout, pump(&handle, &mut events, observed)).await {
        Ok(Ok(())) => {}
        Ok(Err(why)) => observed.errors.push(why),
        Err(_) => {
            timed_out = true;
            // Cancel, then let the core write its own ending: a cancelled turn
            // still has tool calls to close and a summary to produce.
            let _ = handle.send(Command::Cancel).await;
            match tokio::time::timeout(CANCEL_GRACE, pump(&handle, &mut events, observed)).await {
                Ok(Ok(())) => {}
                Ok(Err(why)) => observed.errors.push(why),
                Err(_) => observed.errors.push(format!(
                    "the turn did not stop within {}s of the cancel",
                    CANCEL_GRACE.as_secs()
                )),
            }
        }
    }

    // A background process the agent started would otherwise outlive the tree
    // it runs in, and keep a port or a file handle that the next run needs.
    for id in observed.running_processes.clone() {
        let _ = handle.send(Command::StopProcess { id }).await;
    }
    if timed_out {
        Stopped::TimedOut
    } else {
        Stopped::Ended
    }
}

/// How the session's life ended, as far as the driver can tell.
enum Stopped {
    /// The session never opened, or it stopped before the message reached it.
    NeverStarted,
    /// The turn did not finish within the task's timeout and was cancelled.
    TimedOut,
    /// The turn ended on its own, or the stream closed; `observed.ending` says which.
    Ended,
}

/// Reads events until the turn is over, answering the questions a headless run
/// cannot be asked. Stops at `TurnFinished` or `TurnCancelled`.
async fn pump(
    handle: &SessionHandle,
    events: &mut Receiver<AgentEvent>,
    observed: &mut Observed,
) -> Result<(), String> {
    loop {
        match events.recv().await {
            Ok(event) => {
                observed.observe(&event);
                answer(handle, &event, observed).await?;
                if matches!(event, AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }) {
                    return Ok(());
                }
            }
            // Lagging loses events, which makes the counts a lower bound. Say
            // so in the result rather than letting the numbers look exact.
            Err(RecvError::Lagged(n)) => {
                observed.errors.push(format!("the event stream lagged; {n} events were lost"));
            }
            Err(RecvError::Closed) => {
                return Err("the event stream closed before the turn ended".into());
            }
        }
    }
}

async fn answer(handle: &SessionHandle, event: &AgentEvent, observed: &mut Observed) -> Result<(), String> {
    let sent = |what: &str| format!("the session stopped while {what} was being answered");
    match event {
        AgentEvent::PermissionRequested { request } => {
            observed.permissions_denied += 1;
            handle
                .send(Command::AnswerPermission {
                    request_id: request.id.clone(),
                    answer: PermissionAnswer::Deny { reason: Some(DENY_REASON.into()) },
                })
                .await
                .map_err(|_| sent("a permission"))?;
        }
        // Dismissed rather than left open: an open question holds the tool
        // that asked, and the turn would wait forever for a person who is not
        // there. The model is told there is no answer and carries on.
        AgentEvent::QuestionAsked { request } => {
            observed.questions_dismissed += 1;
            handle
                .send(Command::AnswerQuestion {
                    request_id: request.id.clone(),
                    answer: QuestionReply::Dismissed,
                })
                .await
                .map_err(|_| sent("a question"))?;
        }
        _ => {}
    }
    Ok(())
}

fn mode_name(mode: TaskMode) -> &'static str {
    match mode {
        TaskMode::Agent => "agent",
        TaskMode::Plan => "plan",
    }
}

/// `agent` opens in the window's default mode, which is what a person gets.
/// Bypass, not the mode, is what makes the run unattended.
fn session_mode(mode: TaskMode) -> Mode {
    match mode {
        TaskMode::Agent => Mode::Manual,
        TaskMode::Plan => Mode::Plan,
    }
}

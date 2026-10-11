//! A subagent: a conversation of its own, held inside a turn by the `Task` tool.
//!
//! The point is the context. Finding where something lives takes twenty reads
//! and comes down to three lines; reworking a module is a hundred steps the
//! agent needs none of once it is done. Held in the agent's own conversation,
//! all of that stays in the window for every request after. A subagent does
//! the work in a history that is thrown away and hands back a report.
//!
//! What it may do is what the agent may do. Its calls go through the same
//! [`Caller`] — the same question to the human under the same mode (§5.7,
//! §8), the same checkpoint before a command (§5.8), the same row with its
//! diff — so nothing reaches the working tree by a second road with rules of
//! its own. What it does not share is the registry of read files: what a
//! subagent read went into its conversation, and the agent has to read a file
//! itself before editing it.
//!
//! It is not a session. No journal of its own, no line in the session list,
//! no compaction: its history is worth nothing once the report is written.
//!
//! The feed draws a subagent's calls as rows under the `Task` row
//! (`parent` on [`AgentEvent::ToolCallStarted`]). `docs/ui.md` leaves "where
//! do subagents go" open; rows it has to be, because an edit a subagent made
//! is an edit to the human's files and owes them the same diff.
//!
//! [`AgentEvent::ToolCallStarted`]: crate::event::AgentEvent::ToolCallStarted

use std::path::Path;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::error::{ProviderError, ToolError};
use crate::id::{CallId, TurnId};
use crate::instructions::Instructions;
use crate::intent::Intent;
use crate::message::{strip_thinking, ContentBlock, Message};
use crate::process::shell::Shell;
use crate::provider::ProviderRequest;
use crate::tool::ToolRegistry;
use crate::usage::{Pricing, TokenCounts};

use super::accumulator::{Accumulator, Completion};
use super::calls::Caller;
use super::prompt;

/// How many requests a subagent may make before it has to report.
///
/// The agent has no such limit, but the agent has a human reading its every
/// message. A subagent stuck going round the same files would run until the
/// window or the money ran out, and a task that honestly needs more than this
/// was too wide to hand over in one piece.
pub const MAX_STEPS: usize = 100;

/// The ceiling on one answer, as for the agent: a subagent writes files too.
const MAX_OUTPUT_TOKENS: u32 = 64_000;

/// How much of a report reaches the agent. The reason to send a subagent was
/// to keep the agent's window clear, and a report that pastes the files it
/// read undoes that.
const REPORT_MAX_BYTES: usize = 40_000;

const OUT_OF_STEPS: &str = "You have used all the steps you had. Do not call any more \
     tools: write your final report now from what you have done and found, and say \
     plainly what is left undone or unchecked.";

/// What subagents spent, waiting for the session to count it.
///
/// The usage meter belongs to the session's worker alone, and the calls of a
/// batch — several subagents among them — run side by side. They write here,
/// and the worker counts it once the batch is back.
#[derive(Debug, Default)]
pub struct Ledger(Mutex<Vec<(TokenCounts, Pricing)>>);

impl Ledger {
    pub fn add(&self, counts: TokenCounts, pricing: Pricing) {
        self.0.lock().unwrap().push((counts, pricing));
    }

    pub fn take(&self) -> Vec<(TokenCounts, Pricing)> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// What a `Task` call starts a subagent with.
#[derive(Clone)]
pub struct Subagents {
    /// The agent that is sending it.
    sender: Caller,
    turn: TurnId,
    /// The `Task` row: the subagent's own rows are drawn under it.
    row: CallId,
}

impl Subagents {
    pub(super) fn new(sender: Caller, turn: TurnId, row: CallId) -> Self {
        Subagents { sender, turn, row }
    }

    /// Runs a subagent on `task` and returns its report.
    pub(crate) async fn run(
        &self,
        task: String,
        cancel: &CancellationToken,
    ) -> Result<String, ToolError> {
        let shared = &self.sender.shared;
        let me = Caller {
            registry: Arc::new(ToolRegistry::delegated()),
            ws: Arc::new(self.sender.ws.fork()),
            under: Some(self.row.clone()),
            ..self.sender.clone()
        };

        let mut history = vec![Message::user_text(task)];
        let mut bound_to: Option<Intent> = None;

        for step in 1..=MAX_STEPS + 1 {
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }

            // Read before every request, like the agent's: the mode the human
            // switched to and the model they picked apply from the next step.
            let state = shared.state.lock().unwrap().clone();
            let model = me.model();
            let intent = state.mode.intent();
            // The reasoning so far was produced under the other mode's prompt
            // and tools, and a model that binds it to that prefix rejects the
            // history once the prefix moves (`binding`).
            if bound_to.is_some_and(|was| was != intent) {
                strip_thinking(&mut history);
            }
            bound_to = Some(intent);

            let request = ProviderRequest {
                model: model.id.clone(),
                system: system_prompt(
                    me.ws.root(),
                    intent,
                    me.instructions.as_deref(),
                    shared.shell,
                ),
                messages: shared.shown_to(&model, &history),
                tools: me.registry.schemas(intent),
                max_tokens: model.max_output.min(MAX_OUTPUT_TOKENS),
                effort: model.supports_effort.then_some(state.effort),
                thinking: model.supports_thinking,
            };
            let done = self.complete(&me, request, cancel).await?;
            shared.ledger.add(done.usage, model.pricing);

            let said = text_of(&done.blocks);
            if !done.blocks.is_empty() {
                history.push(Message::assistant(done.blocks));
            }
            if done.calls.is_empty() {
                return report(said);
            }
            if step > MAX_STEPS {
                return Err(ToolError::failed_with(
                    format!("подагент не уложился в {MAX_STEPS} шагов"),
                    "The subagent used all its steps without writing a report. What it \
                     changed is in the working tree: look before going on. Give the \
                     rest to a subagent as a narrower task, or do it yourself.",
                ));
            }

            let mut results = me.run_batch(&self.turn, &done.calls, cancel).await;
            if step == MAX_STEPS {
                // In the same message as the results: a second user message
                // in a row is one the provider may refuse.
                results.push(ContentBlock::Text {
                    text: OUT_OF_STEPS.to_string(),
                });
            }
            history.push(Message::tool_results(results));
        }

        unreachable!("the last step either reports or fails")
    }

    /// One answer from the model, with nothing of it shown: the subagent's
    /// words are for the agent, and the feed has the rows of what it did.
    async fn complete(
        &self,
        me: &Caller,
        request: ProviderRequest,
        cancel: &CancellationToken,
    ) -> Result<Completion, ToolError> {
        let failed = |err: ProviderError| ToolError::failed(format!("подагент: {err}"));

        let opened = me.shared.open(me.provider.as_ref(), request, cancel);
        let mut stream = match opened.await {
            Ok(stream) => stream,
            Err(_) if cancel.is_cancelled() => return Err(ToolError::Cancelled),
            Err(err) => return Err(failed(err)),
        };

        let mut acc = Accumulator::new(self.turn.clone());
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(ToolError::Cancelled),
                item = stream.next() => match item {
                    None => break,
                    // The feed events it makes are the model's text and
                    // reasoning; neither is addressed to the human.
                    Some(Ok(event)) => drop(acc.push(event)),
                    Some(Err(err)) => return Err(failed(err)),
                },
            }
        }
        Ok(acc.finish())
    }
}

fn text_of(blocks: &[ContentBlock]) -> String {
    let parts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect();
    parts.join("\n\n")
}

fn report(mut text: String) -> Result<String, ToolError> {
    if text.is_empty() {
        return Err(ToolError::failed("подагент ничего не ответил"));
    }
    if text.len() > REPORT_MAX_BYTES {
        let cut = (0..=REPORT_MAX_BYTES)
            .rev()
            .find(|i| text.is_char_boundary(*i))
            .unwrap_or(0);
        text.truncate(cut);
        text.push_str(&format!(
            "\n[отчёт обрезан: показаны первые {REPORT_MAX_BYTES} байт]"
        ));
    }
    Ok(text)
}

const INTRO: &str = "You are a subagent of Yaca, a coding agent working inside an \
     editor. The agent has handed you one task and reads only your final message. \
     The human it works with sees every tool call you make as it happens, together \
     with its result, but none of your messages.\n\n";

const MODE_PLAN: &str = "<mode>\nMode: plan. You are here to investigate: tools that \
     change files or run commands are not available. If the task asks for a change, \
     work out what it would take and report that instead of the change.\n</mode>\n\n";

const MODE_AGENT: &str = "<mode>\nMode: agent. Do the task to the end and verify the \
     result, rather than reporting what could be done.\n</mode>\n\n";

const SCOPE: &str = "Do what the task says, at the scope it sets. You cannot ask a \
     question, so where the task can be read two ways, take the reading that changes \
     less and say in the report which one you took. Other subagents may be working in \
     this project at the same time: leave alone files the task does not give you. \
     Anything else you notice on the way belongs in the report, not in this \
     change.\n\n";

const REPORT: &str = "<report>\nYour final message is the whole result of your work, \
     and the agent acts on it without having seen what you saw. Open with the \
     outcome: the answer to what was asked, or what you changed. Name every file you \
     changed, and point at code as `path:line` so the agent can open the exact place. \
     Quote only the lines the report rests on, and leave out the story of how you got \
     there. Report results as they are: a failing test with its output, a step you \
     skipped as skipped, and what you could not find or did not check as plainly as \
     what you did.\n</report>\n\n";

/// The subagent's system prompt.
///
/// Built from the agent's own parts where the two are in the same position —
/// how to write code, what a refused permission means, how a row is captioned
/// — and from its own where they are not: the reader of its final message is
/// a model, there is nobody to ask, and it has no task list to keep.
fn system_prompt(
    project_root: &Path,
    intent: Intent,
    instructions: Option<&Instructions>,
    shell: Shell,
) -> String {
    let agent = intent == Intent::Agent;
    let mut s = String::from(INTRO);

    s.push_str("<environment>\n");
    s.push_str(&format!(
        "Project root: {}. Write paths relative to it.\n",
        prompt::shown(project_root)
    ));
    if agent {
        s.push_str(prompt::shell_fact(shell));
    }
    s.push_str("</environment>\n\n");

    s.push_str(if agent { MODE_AGENT } else { MODE_PLAN });

    s.push_str("<how_to_work>\n");
    s.push_str(prompt::GROUNDING);
    s.push_str(SCOPE);
    if agent {
        s.push_str(prompt::CODE);
    }
    s.push_str(prompt::BATCHING);
    s.push_str("</how_to_work>\n\n");

    s.push_str("<permissions>\n");
    s.push_str(prompt::PERMISSION_NETWORK);
    if agent {
        s.push_str(prompt::PERMISSION_AGENT);
    }
    s.push_str(prompt::PERMISSION_OUTCOME);
    if agent {
        s.push_str(prompt::JUDGMENT);
    }
    s.push_str("</permissions>\n\n");

    if agent {
        s.push_str(prompt::EDITING);
    }
    s.push_str(REPORT);
    s.push_str(prompt::CAPTIONS);

    if let Some(instructions) = instructions {
        s.push('\n');
        s.push_str(&prompt::project_section(instructions));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_the_text_and_nothing_else_of_the_answer() {
        let blocks = vec![
            ContentBlock::Thinking {
                thinking: "hm".into(),
                signature: None,
            },
            ContentBlock::Text {
                text: "first\n".into(),
            },
            ContentBlock::Text {
                text: "second".into(),
            },
        ];
        assert_eq!(text_of(&blocks), "first\n\nsecond");
    }

    #[test]
    fn an_empty_report_is_a_failure_not_an_empty_result() {
        // The agent would read an empty result as "nothing found".
        assert!(matches!(report(String::new()), Err(ToolError::Failed { .. })));
    }

    #[test]
    fn a_long_report_is_cut_on_a_character_and_says_so() {
        let cut = report("я".repeat(REPORT_MAX_BYTES)).unwrap();
        assert!(cut.len() < REPORT_MAX_BYTES + 100);
        assert!(cut.ends_with("байт]"));
        assert_eq!(report("short".into()).unwrap(), "short");
    }

    fn plain(intent: Intent) -> String {
        system_prompt(Path::new("/p"), intent, None, Shell::default())
    }

    #[test]
    fn the_prompt_is_stable_and_carries_the_project_rules() {
        let rules = Instructions {
            path: "AGENTS.md".into(),
            text: "Use tabs.".into(),
            truncated: false,
        };
        // The prefix of every request the subagent makes; a clock in here
        // would void the cache between its steps.
        assert_eq!(plain(Intent::Agent), plain(Intent::Agent));
        let with = system_prompt(Path::new("/p"), Intent::Agent, Some(&rules), Shell::default());
        assert!(with.contains("Use tabs."));
        assert!(with.trim_end().ends_with("</project_instructions>"));
        assert!(!plain(Intent::Agent).contains("project_instructions"));
    }

    #[test]
    fn writing_is_only_explained_where_it_can_happen() {
        // The mode binds a subagent as it binds the agent (§7.1), and telling
        // it how edits work in plan would be telling it they do.
        assert!(plain(Intent::Plan).contains("Mode: plan"));
        assert!(!plain(Intent::Plan).contains("working tree"));
        assert!(!plain(Intent::Plan).contains(Shell::default().name()));
        assert!(plain(Intent::Agent).contains("Mode: agent"));
        assert!(plain(Intent::Agent).contains("working tree"));
        assert!(plain(Intent::Agent).contains(Shell::default().name()));
    }

    #[test]
    fn it_is_not_told_about_what_only_the_agent_has() {
        // Neither is among its tools, so naming them would send it looking.
        for intent in [Intent::Plan, Intent::Agent] {
            let p = plain(intent);
            assert!(!p.contains("TodoWrite"), "{intent:?}");
            assert!(!p.contains("ExitPlanMode"), "{intent:?}");
        }
    }

    #[test]
    fn tags_are_balanced_in_every_mode() {
        for intent in [Intent::Plan, Intent::Agent] {
            let p = plain(intent);
            for tag in [
                "environment",
                "mode",
                "how_to_work",
                "permissions",
                "report",
                "action_captions",
            ] {
                let open = p.matches(&format!("<{tag}>")).count();
                let close = p.matches(&format!("</{tag}>")).count();
                assert_eq!((open, close), (1, 1), "{intent:?} <{tag}>");
            }
        }
    }
}

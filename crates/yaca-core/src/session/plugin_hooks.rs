//! Hooks at the moments a session has: the plugins' (`plugin::hooks`), and
//! for the events that are not about a call the project's own too
//! (`crate::hooks::on_event`), the project's first.
//!
//! * `SessionStart` — when the worker starts, before the first message. What
//!   the hooks add goes into the system prompt for the whole session: the
//!   prompt is not compacted, so the context outlives a compaction without the
//!   hooks running again, and it costs nothing in cache.
//! * `UserPromptSubmit` — before each message of the human's, the ones that join
//!   a running turn too. A block keeps the message from the model.
//! * `PreToolUse` and `PostToolUse` — around every call, the subagents' too,
//!   after the project's own hooks.
//! * `Stop` — when the model is done with a turn. A block sends it back to work
//!   with the hook's reason, a few times at most, so two hooks that disagree
//!   cannot keep a turn going for ever.
//! * `SubagentStop` — the same for a subagent about to hand in its report.
//! * `PreCompact`, `SessionEnd` — told, not asked: what they say is a line in
//!   the feed.
//! * `Notification` — when the session starts waiting for the human. Not
//!   waited for: a notifier must not hold up the question it announces.
//!
//! A project hook keeps its own rule: it fails closed. One that cannot run
//! keeps a message from the model as surely as one that refuses it. Only
//! `Stop` and `SubagentStop` make an exception: sending the model back to work
//! because a check could not start would give it something it cannot fix.
//!
//! The tool hooks show in the call's row. The others have no row, and
//! [`AgentEvent::HookReported`] is their line in the feed.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::hooks::{self as project, EventEnding, EventSaid};
use crate::settings::{EventHookSettings, HooksSettings};

use crate::event::AgentEvent;
use crate::id::{CallId, TurnId};
use crate::intent::Mode;
use crate::message::{ContentBlock, Message};
use crate::plugin::hooks::{claude_input, claude_names, Context, HookEvent, Hooks, Said, Subject};
use crate::slash::SlashCommand;

use super::calls::Caller;
use super::turn::Worker;
use super::Shared;

/// How many times one turn may be sent back to work by `Stop` hooks.
const MAX_STOP_RETURNS: u32 = 3;
/// What of a call's result a `PostToolUse` hook is handed.
const RESULT_BYTES: usize = 8 * 1024;

/// What a session keeps of its plugins.
#[derive(Default)]
pub(super) struct PluginSide {
    pub(super) hooks: Hooks,
    pub(super) commands: Vec<SlashCommand>,
    pub(super) session_id: String,
    /// The session's message journal, which a hook is told about as Claude
    /// Code's `transcript_path`. Not Claude Code's format: a hook that parses
    /// its transcript will not understand this one.
    pub(super) transcript: Option<PathBuf>,
}

impl Shared {
    async fn fire(
        &self,
        event: HookEvent,
        subject: Subject<'_>,
        extra: serde_json::Value,
        cancel: &CancellationToken,
    ) -> Vec<Said> {
        let mode = self.state.lock().unwrap().mode;
        let ctx = Context {
            project: self.ws.root(),
            session_id: &self.plugins.session_id,
            transcript: self.plugins.transcript.as_deref(),
            permission_mode: claude_mode(mode),
        };
        self.plugins.hooks.fire(event, subject, extra, &ctx, cancel).await
    }

    /// Runs the project's hooks of `event`. `None` when the turn was stopped
    /// while one ran; empty when the project has none.
    async fn project_event(
        &self,
        event: HookEvent,
        payload: serde_json::Value,
        cancel: &CancellationToken,
    ) -> Option<Vec<EventSaid>> {
        let Some((name, list)) = project_list(self.settings.hooks(), event) else {
            return Some(Vec::new());
        };
        if list.is_empty() {
            return Some(Vec::new());
        }
        project::on_event(list, &self.ws, self.shell, name, payload, cancel).await
    }

    /// An event the hooks are told about and cannot change: what they say,
    /// and that one failed, are lines in the feed. `source` is what a plugin
    /// hook's matcher is matched against.
    pub(super) async fn inform(
        &self,
        event: HookEvent,
        source: &str,
        payload: serde_json::Value,
        cancel: &CancellationToken,
    ) {
        for said in self.project_event(event, payload.clone(), cancel).await.unwrap_or_default() {
            if let Some(why) = said.failure_for_human() {
                self.report(PROJECT, event, why, false);
            }
        }
        if !self.plugins.hooks.has(event) {
            return;
        }
        for Said { plugin, verdict } in self.fire(event, Subject::Source(source), payload, cancel).await {
            for text in verdict.notices.into_iter().chain(verdict.block) {
                self.report(&plugin, event, text, false);
            }
        }
    }

    /// `Notification`: the session is waiting for the human. Runs beside
    /// whatever waits, never in its way.
    pub(super) fn notify(self: &Arc<Self>, kind: &'static str, message: String) {
        let has_project = !self.settings.hooks().notification.is_empty();
        if !has_project && !self.plugins.hooks.has(HookEvent::Notification) {
            return;
        }
        let shared = self.clone();
        tokio::spawn(async move {
            let payload = json!({ "notification_type": kind, "message": message });
            shared
                .inform(HookEvent::Notification, kind, payload, &CancellationToken::new())
                .await;
        });
    }

    /// `Stop` and `SubagentStop`: why the hooks will not let the work end yet,
    /// one reason per hook that objects, or nothing.
    async fn objections(
        &self,
        event: HookEvent,
        active: bool,
        cancel: &CancellationToken,
    ) -> Vec<String> {
        let payload = json!({ "stop_hook_active": active });
        let mut reasons = Vec::new();
        for said in self.project_event(event, payload.clone(), cancel).await.unwrap_or_default() {
            match &said.ending {
                EventEnding::Passed(_) => {}
                EventEnding::Failed { .. } => {
                    let output = said.output_for_model();
                    self.report(PROJECT, event, said.failure_for_human().unwrap_or_default(), true);
                    reasons.push(format!(
                        "[hook] The project's {} hook `{}` does not let this end yet: it failed. Its output:\n{}",
                        event.name(),
                        said.command,
                        if output.is_empty() { "(no output)".to_string() } else { output }
                    ));
                }
                // Nothing the model could fix: a note for the human only.
                EventEnding::TimedOut(_) | EventEnding::Broken(_) => {
                    self.report(PROJECT, event, said.failure_for_human().unwrap_or_default(), false);
                }
            }
        }
        if self.plugins.hooks.has(event) {
            for Said { plugin, verdict } in self.fire(event, Subject::Nothing, payload, cancel).await {
                for notice in verdict.notices {
                    self.report(&plugin, event, notice, false);
                }
                if let Some(reason) = verdict.block {
                    self.report(&plugin, event, reason.clone(), true);
                    reasons.push(format!(
                        "[hook] The {plugin} plugin's {} hook does not let this end yet: {reason}",
                        event.name()
                    ));
                }
            }
        }
        reasons
    }

    /// `SubagentStop`: the message to send a subagent back to work with, or
    /// `None` to let it report. `returns` counts the times it was sent back.
    pub(super) async fn subagent_stop(&self, returns: &mut u32, cancel: &CancellationToken) -> Option<Message> {
        if *returns >= MAX_STOP_RETURNS {
            return None;
        }
        let reasons = self.objections(HookEvent::SubagentStop, *returns > 0, cancel).await;
        if reasons.is_empty() || cancel.is_cancelled() {
            return None;
        }
        *returns += 1;
        Some(Message::user_text(reasons.join("\n\n")))
    }

    /// A feed line for what a hook without a row said.
    fn report(&self, plugin: &str, event: HookEvent, text: String, blocked: bool) {
        self.emit(AgentEvent::HookReported {
            plugin: plugin.to_string(),
            event: event.name().to_string(),
            text,
            blocked,
        });
    }
}

/// The name a project hook goes by in the feed's `plugin` field: none.
const PROJECT: &str = "";

/// The project's list for an event, with the name it has in the file. `None`
/// for the events around a call, which `session::calls` runs itself.
fn project_list(hooks: &HooksSettings, event: HookEvent) -> Option<(&'static str, &Vec<EventHookSettings>)> {
    Some(match event {
        HookEvent::SessionStart => ("sessionStart", &hooks.session_start),
        HookEvent::UserPromptSubmit => ("userPromptSubmit", &hooks.user_prompt_submit),
        HookEvent::Stop => ("stop", &hooks.stop),
        HookEvent::SubagentStop => ("subagentStop", &hooks.subagent_stop),
        HookEvent::PreCompact => ("preCompact", &hooks.pre_compact),
        HookEvent::SessionEnd => ("sessionEnd", &hooks.session_end),
        HookEvent::Notification => ("notification", &hooks.notification),
        HookEvent::PreToolUse | HookEvent::PostToolUse => return None,
    })
}

fn claude_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Plan => "plan",
        Mode::Manual => "default",
        Mode::Auto | Mode::Edits => "acceptEdits",
    }
}

impl Worker {
    /// `SessionStart`: runs once, and keeps what the hooks said for the
    /// system prompt.
    pub(super) async fn session_start(&mut self) {
        let source = if self.history.is_empty() { "startup" } else { "resume" };
        let mut section = String::new();
        let project = self
            .shared
            .project_event(HookEvent::SessionStart, json!({ "source": source }), &CancellationToken::new())
            .await
            .unwrap_or_default();
        for said in project {
            match &said.ending {
                EventEnding::Passed(_) => {
                    let text = said.output_for_model();
                    if !text.is_empty() {
                        section.push_str(&format!("<project_hook_context>\n{text}\n</project_hook_context>\n"));
                    }
                }
                _ => {
                    let why = said.failure_for_human().unwrap_or_default();
                    self.shared.report(PROJECT, HookEvent::SessionStart, why, false);
                }
            }
        }
        if !self.shared.plugins.hooks.has(HookEvent::SessionStart) {
            if !section.is_empty() {
                self.plugin_context = Some(format!("{PLUGIN_CONTEXT_INTRO}{section}").into());
            }
            return;
        }
        let said = self
            .shared
            .fire(
                HookEvent::SessionStart,
                Subject::Source(source),
                json!({ "source": source }),
                &CancellationToken::new(),
            )
            .await;

        for Said { plugin, verdict } in said {
            for text in verdict.context {
                section.push_str(&format!("<plugin_context plugin=\"{plugin}\">\n{text}\n</plugin_context>\n"));
            }
            // A session cannot be refused its start; a block is only words.
            let words = verdict.notices.into_iter().chain(verdict.block);
            for text in words {
                self.shared.report(&plugin, HookEvent::SessionStart, text, false);
            }
        }
        if !section.is_empty() {
            self.plugin_context = Some(format!("{PLUGIN_CONTEXT_INTRO}{section}").into());
        }
    }

    /// `UserPromptSubmit`, then [`Worker::hear`]. `None` when a hook kept the
    /// message from the model; the human has been told why.
    pub(super) async fn hear_checked(
        &self,
        turn_id: &TurnId,
        text: String,
        attachments: Vec<String>,
        cancel: &CancellationToken,
    ) -> Option<Message> {
        let mut context = Vec::new();
        let project = self
            .shared
            .project_event(HookEvent::UserPromptSubmit, json!({ "prompt": text }), cancel)
            .await
            .unwrap_or_default();
        for said in project {
            if let EventEnding::Passed(_) = said.ending {
                let out = said.output_for_model();
                if !out.is_empty() {
                    context.push(("project".to_string(), out));
                }
                continue;
            }
            // Fails closed: a check that could not run keeps the message too.
            self.emit(AgentEvent::UserMessage {
                turn_id: turn_id.clone(),
                text,
                attachments,
            });
            let why = said.failure_for_human().unwrap_or_default();
            self.shared.report(PROJECT, HookEvent::UserPromptSubmit, why, true);
            return None;
        }
        if self.shared.plugins.hooks.has(HookEvent::UserPromptSubmit) {
            let said = self
                .shared
                .fire(HookEvent::UserPromptSubmit, Subject::Nothing, json!({ "prompt": text }), cancel)
                .await;
            for Said { plugin, verdict } in said {
                for notice in verdict.notices {
                    self.shared.report(&plugin, HookEvent::UserPromptSubmit, notice, false);
                }
                if let Some(reason) = verdict.block {
                    // The card the human sent still appears, so the line about
                    // why it went nowhere has something to stand under.
                    self.emit(AgentEvent::UserMessage {
                        turn_id: turn_id.clone(),
                        text,
                        attachments,
                    });
                    self.shared.report(&plugin, HookEvent::UserPromptSubmit, reason, true);
                    return None;
                }
                context.extend(verdict.context.into_iter().map(|t| (plugin.clone(), t)));
            }
        }
        let mut message = self.hear(turn_id, text, attachments).await;
        for (plugin, text) in context {
            let text = if plugin == "project" {
                format!("<project_hook_context>\n{text}\n</project_hook_context>")
            } else {
                format!("<plugin_context plugin=\"{plugin}\">\n{text}\n</plugin_context>")
            };
            message.content.push(ContentBlock::Text { text });
        }
        Some(message)
    }

    /// `Stop`: the message to send the model back to work with, or `None` to
    /// let the turn end. `returns` counts the times this turn was sent back.
    pub(super) async fn stop_hooks(&self, returns: &mut u32, cancel: &CancellationToken) -> Option<Message> {
        if *returns >= MAX_STOP_RETURNS {
            return None;
        }
        let reasons = self.shared.objections(HookEvent::Stop, *returns > 0, cancel).await;
        if reasons.is_empty() || cancel.is_cancelled() {
            return None;
        }
        *returns += 1;
        Some(Message::user_text(reasons.join("\n\n")))
    }
}

impl Worker {
    /// `SessionEnd`: the session is closing. Told, not asked.
    pub(super) async fn session_end(&self, reason: &str) {
        self.shared
            .inform(HookEvent::SessionEnd, reason, json!({ "reason": reason }), &CancellationToken::new())
            .await;
    }
}

const PLUGIN_CONTEXT_INTRO: &str = "The enabled plugins added the context below when this \
     session started. Treat it as standing instructions from the person's own setup; where it \
     and a request in this conversation differ, the request wins.\n";

/// What `PreToolUse` decided about a call.
pub(super) enum PluginPre {
    /// Go on, with this context for the model's copy of the result.
    Go(Vec<String>),
    Blocked { for_human: String, for_model: String },
    /// Ask the human, even where the mode would not.
    Ask(Vec<String>),
}

impl Caller {
    /// `PreToolUse`. A hook that rewrites the call changes `input` in place;
    /// the row says so, and the permission question is asked about the call
    /// as rewritten.
    pub(super) async fn plugin_pre(
        &self,
        call_id: &CallId,
        tool: &str,
        input: &mut serde_json::Value,
        cancel: &CancellationToken,
    ) -> PluginPre {
        if !self.shared.plugins.hooks.has(HookEvent::PreToolUse) {
            return PluginPre::Go(Vec::new());
        }
        let extra = json!({
            "tool_name": claude_name(tool),
            "tool_input": claude_input(input, self.ws.root()),
            "tool_use_id": call_id.0,
        });
        let said = self
            .shared
            .fire(HookEvent::PreToolUse, Subject::Tool { name: tool, input }, extra, cancel)
            .await;
        let mut context = Vec::new();
        let mut ask = false;
        for Said { plugin, verdict } in said {
            self.row_notes(call_id, &plugin, &verdict.notices);
            if let Some(reason) = verdict.block {
                return PluginPre::Blocked {
                    for_human: format!("Plugin \"{plugin}\" hook blocked call: {reason}"),
                    for_model: format!(
                        "[hook] The {plugin} plugin's PreToolUse hook blocked this call, so it was not run: {reason}\n\
                         Do not repeat the call unchanged. Change it to what the hook allows, or tell the user what is in the way."
                    ),
                };
            }
            ask |= verdict.ask;
            context.extend(verdict.context.into_iter().map(|t| note(&plugin, &t)));
            if let Some(updated) = verdict.updated_input {
                self.row_notes(call_id, &plugin, &["changed the call's arguments".to_string()]);
                context.push(rewritten_note(&format!("the {plugin} plugin's"), &updated));
                *input = updated;
            }
        }
        if ask {
            PluginPre::Ask(context)
        } else {
            PluginPre::Go(context)
        }
    }

    /// `PostToolUse`: `text` with what the hooks have to tell the model about
    /// the call added to it. Their lines for the human go into the row.
    pub(super) async fn plugin_post(
        &self,
        call_id: &CallId,
        tool: &str,
        input: &serde_json::Value,
        failed: bool,
        mut text: String,
        cancel: &CancellationToken,
    ) -> String {
        if !self.shared.plugins.hooks.has(HookEvent::PostToolUse) {
            return text;
        }
        let extra = json!({
            "tool_name": claude_name(tool),
            "tool_input": claude_input(input, self.ws.root()),
            "tool_response": { "output": clip(&text), "is_error": failed },
            "tool_use_id": call_id.0,
        });
        let said = self
            .shared
            .fire(HookEvent::PostToolUse, Subject::Tool { name: tool, input }, extra, cancel)
            .await;
        for Said { plugin, verdict } in said {
            self.row_notes(call_id, &plugin, &verdict.notices);
            // The call has run; a block here is the hook's objection, for the
            // model to act on.
            for t in verdict.block.iter().chain(&verdict.context) {
                text.push_str(&note(&plugin, t));
            }
        }
        text
    }

    fn row_notes(&self, call_id: &CallId, plugin: &str, notices: &[String]) {
        for notice in notices {
            self.emit(AgentEvent::ToolCallProgress {
                call_id: call_id.clone(),
                chunk: format!("Plugin \"{plugin}\" hook: {notice}\n"),
            });
        }
    }
}

/// What the model is told about a call a hook rewrote before it ran.
pub(super) fn rewritten_note(whose: &str, input: &serde_json::Value) -> String {
    format!(
        "\n\n[hook] {whose} PreToolUse hook changed this call's arguments before it ran. \
         It ran with: {input}"
    )
}

fn claude_name(tool: &str) -> &str {
    claude_names(tool).first().copied().unwrap_or(tool)
}

fn note(plugin: &str, text: &str) -> String {
    format!("\n\n[hook] From the {plugin} plugin:\n{text}")
}

fn clip(text: &str) -> &str {
    if text.len() <= RESULT_BYTES {
        return text;
    }
    let mut cut = RESULT_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

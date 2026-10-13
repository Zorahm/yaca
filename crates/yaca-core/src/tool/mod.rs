//! Тулы и реестр.
//!
//! The mechanics of §5.4.1 live here: who writes the caption of an action. Tools
//! come in two kinds, and the split is drawn in the call's schema, not in the prompt.
//!
//! * [`Narration::Interface`] — the caption is derived from the fact: a file
//!   created, changed, deleted, or failed. The `description` field is absent from
//!   the schema altogether, so the model does not spend tokens on it or try to rewrite the fact.
//! * [`Narration::Model`] — only the model knows the caption: from `rg -n $'\x01' src/`
//!   the interface cannot tell why it was run. The `description` field is required.

pub mod ask_user;
pub mod bash;
mod bash_background;
pub mod bash_control;
pub mod bash_output;
pub mod delete_dir;
pub mod delete_file;
pub mod edit_file;
pub mod exit_plan_mode;
pub mod extract_message;
pub(crate) mod files;
pub(crate) mod net;
pub mod glob;
pub mod grep;
pub mod lsp;
pub mod memory;
pub(crate) mod page;
pub(crate) mod robots;
pub mod read_file;
pub mod read_image;
pub mod search_message;
pub mod send_text;
pub mod set_title;
pub mod skill;
pub mod task;
pub mod todo_write;
pub mod web_fetch;
pub mod web_search;
pub mod write_file;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::change::{FileDiff, FileFact};
use crate::checkpoint::CheckpointReason;
use crate::error::ToolError;
use crate::event::{AgentEvent, EventSink, ToolMeta};
use crate::id::CallId;
use crate::intent::Intent;
use crate::lsp::Lsp;
use crate::permission::PermissionNeed;
use crate::picture::Picture;
use crate::process::shell::Shell;
use crate::process::Processes;
use crate::provider::ToolSchema;
use crate::session::questions::Questions;
use crate::session::subagent::Subagents;
use crate::store::transcript::Transcript;
use crate::workspace::Workspace;

/// Maximum length of a caption, from §5.4.1.
pub const DESCRIPTION_MAX_CHARS: usize = 60;

/// Who writes the caption of an action in the feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Narration {
    Interface,
    Model,
}

/// Where a tool sends its output while it is still running (§15, `ToolCallProgress`).
///
/// `call_id` is built in: the tool physically cannot write into someone else's
/// feed row. The «пользовались» flag decides who sends the text: the feed gets it
/// exactly once — either as it arrives, or whole when the tool has returned.
pub struct Progress {
    call_id: CallId,
    events: EventSink,
    used: AtomicBool,
}

impl Progress {
    pub fn new(call_id: CallId, events: EventSink) -> Self {
        Progress {
            call_id,
            events,
            used: AtomicBool::new(false),
        }
    }

    pub fn push(&self, chunk: impl Into<String>) {
        let chunk: String = chunk.into();
        if chunk.is_empty() {
            return;
        }
        self.used.store(true, Ordering::Relaxed);
        self.events.emit(AgentEvent::ToolCallProgress {
            call_id: self.call_id.clone(),
            chunk,
        });
    }

    /// Whether output went to the feed on the way. The session looks here so as not
    /// to show the same text twice.
    pub fn used(&self) -> bool {
        self.used.load(Ordering::Relaxed)
    }
}

pub struct ToolCtx {
    pub ws: Arc<Workspace>,
    /// Фоновые команды сессии (§6.5).
    pub processes: Processes,
    pub events: EventSink,
    pub progress: Progress,
    pub cancel: CancellationToken,
    /// What `Task` starts a subagent with. `None` for a subagent's own calls,
    /// and so one does not hand its work to another.
    pub agents: Option<Subagents>,
    /// Asks the human a question and waits for the answer (`AskUser`). `None`
    /// only where there is no session behind the call, which is unit tests.
    pub questions: Option<Questions>,
    /// Whether the model that made the call can be shown an image.
    pub vision: bool,
    /// The shell commands run through, as the session settled it.
    pub shell: Shell,
    /// The session's language servers. `None` only where there is no session
    /// behind the call, which is unit tests.
    pub lsp: Option<Lsp>,
    /// Whether to honour `Disallow` from `robots.txt` (§10).
    ///
    /// An application-level setting, not a project one: it is about how Yaca behaves
    /// on the network, and the human decides it for the whole program. It arrives as a field of
    /// [`crate::SessionConfig`] — by the same route as the mode and the effort.
    pub respect_robots: bool,
    /// What was said in this conversation, from the session's journal. `None`
    /// where nothing is written down: the tools that read it are not offered
    /// there (`ToolRegistry::of_session`).
    pub transcript: Option<Transcript>,
    /// The session already has a name the model may not replace — its own, or
    /// one a human typed. `SetTitle` refuses on it. False where there is no
    /// index behind the call.
    pub title_locked: bool,
}

impl ToolCtx {
    pub(crate) fn check_cancelled(&self) -> Result<(), ToolError> {
        if self.cancel.is_cancelled() {
            Err(ToolError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// A change that landed in the working tree. The interface takes the caption of
/// the row (§5.5) and the diff for its expansion (§5.5.1).
#[derive(Debug, Clone)]
pub struct FileChange {
    pub fact: FileFact,
    pub diff: FileDiff,
}

#[derive(Debug)]
pub struct ToolOutput {
    /// What goes to the model as `tool_result`.
    pub text: String,
    /// What the interface shows in the expanded row and in the counters.
    pub meta: ToolMeta,
    /// Filled in by the file tools. The session sends the event from it — it alone
    /// knows the `call_id`, and all events leave from one place.
    pub changed: Option<FileChange>,
    /// Images for the model, each under the name it is told. They reach it
    /// after the results of the batch: see [`ContentBlock::Image`].
    ///
    /// [`ContentBlock::Image`]: crate::message::ContentBlock::Image
    pub pictures: Vec<(String, Picture)>,
    /// The call answered and its work goes on: the session leaves its row
    /// open, and whoever was left working closes it (`session::background`).
    pub detached: bool,
}

impl ToolOutput {
    pub fn new(text: String) -> Self {
        let lines = text.lines().count() as u32;
        ToolOutput {
            text,
            meta: ToolMeta {
                lines,
                ..Default::default()
            },
            changed: None,
            pictures: Vec::new(),
            detached: false,
        }
    }

    pub fn detached(mut self) -> Self {
        self.detached = true;
        self
    }

    pub fn picture(mut self, name: String, picture: Picture) -> Self {
        self.pictures.push((name, picture));
        self
    }

    pub fn changed(mut self, change: FileChange) -> Self {
        self.changed = Some(change);
        self
    }

    pub fn items(mut self, n: u32) -> Self {
        self.meta.items = Some(n);
        self
    }

    pub fn truncated(mut self, yes: bool) -> Self {
        self.meta.truncated = yes;
        self
    }

    /// How many lines went to the feed. Counted from `text` by default, but for
    /// streaming tools the feed and the model see different things: a command gives the human
    /// all of its output, and the model a cut one.
    pub fn lines(mut self, n: u32) -> Self {
        self.meta.lines = n;
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// Not `'static`: a tool that comes from an MCP server is named by the
    /// server, at run time.
    fn name(&self) -> &str;

    /// The tool's description for the model — not the caption of an action.
    fn description(&self) -> &str;

    /// JSON Schema properties without the internal `description`: the registry adds it.
    fn properties(&self) -> Value;

    fn required(&self) -> &[&str];

    /// The whole input schema, for a tool that has one ready-made — an MCP
    /// server describes its tools itself. `None` means the registry builds it
    /// from [`Tool::properties`] and [`Tool::required`], closed to other keys.
    fn input_schema(&self) -> Option<Value> {
        None
    }

    /// The tool has an argument of its own called `description`, so the
    /// caption of §5.4.1 is not taken out of its input before it runs.
    fn keeps_description(&self) -> bool {
        false
    }

    /// The call leaves no trace in the stream: no row, no output, no failure
    /// line. For a tool whose effect reaches the interface another way — as an
    /// event of its own — so that the row would only repeat it. The model still
    /// sees the call and its result, and a refused call is still refused.
    fn silent(&self) -> bool {
        false
    }

    fn narration(&self) -> Narration;

    fn intents(&self) -> &'static [Intent];

    /// Whether a call may run at the same time as its neighbours in one batch (§5.4).
    ///
    /// The model calls tools in a batch precisely so that they run together, and five
    /// reads in a row are five waits instead of one. But running at the same time
    /// is not allowed for all of them.
    ///
    /// `true` is set on a tool that changes nothing and asks about nothing.
    /// Writing tools may not: two edits of one file are a race around
    /// the §8 freshness invariant, where both checks pass but only one survives,
    /// and the «перед командой» checkpoint (§5.8) stops meaning anything if
    /// a second command runs alongside. Asking tools may not either: §5.7 draws one
    /// block with a `2 из 3` counter, and two questions at once have nowhere to sit.
    fn concurrent(&self) -> bool {
        false
    }

    /// The first line of the expanded action row (§5.5.1): the full path,
    /// full command, full URL.
    fn target(&self, _input: &Value) -> Option<String> {
        None
    }

    /// The permission this call needs (§8).
    ///
    /// `None` — there is nothing to ask about the action in any trust mode.
    /// Worked out before the tool runs: the question must be settled before
    /// anything happens. Malformed arguments give `None` — the tool itself will
    /// say more clearly about them.
    ///
    /// Takes the workspace, not the whole [`ToolCtx`]: the question is asked before the call, and
    /// building the feed, processes and cancel token for it would mean building
    /// a context that nobody uses.
    fn permission(&self, _ws: &Workspace, _input: &Value) -> Option<PermissionNeed> {
        None
    }

    /// Whether this particular call may run in Plan (§7.1).
    ///
    /// A tool offered in Plan is still asked per call: Bash is offered there
    /// only for the commands that change nothing. `Err` carries the sentence the
    /// model reads back, in the language of the conversation.
    fn plan_check(&self, _ws: &Workspace, _input: &Value) -> Result<(), String> {
        Ok(())
    }

    /// The snapshot this call needs taken before it runs (§5.8).
    ///
    /// `None` for file edits: the point taken before the message (§5.8) already
    /// covers everything the turn writes, and that is the granularity `Esc Esc`
    /// rolls back to. A divider per edited file would make the feed a list of
    /// snapshots instead of a conversation.
    ///
    /// Asked after the permission gate and answered before the tool starts, for
    /// the same reason §5.7 is: what cannot be undone must be dealt with while
    /// there is still nothing to undo.
    fn checkpoint(&self, _input: &Value) -> Option<CheckpointReason> {
        None
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError>;
}

/// Text the system prompt puts into the description of the `description` field.
const DESCRIPTION_FIELD_DOC: &str = "Single line, up to 60 characters, no trailing period: \
what was done or discovered, in past tense first person, in the conversation's language. \
Focus on the substance, not the tool: 'Found existing clipboard usage' \
rather than 'Ran rg clipboard src/'. No tool names and no command flags.";

/// The tools a subagent is not given: see [`ToolRegistry::delegated`].
pub(crate) const AGENT_ONLY: [&str; 8] = [
    "Task",
    "TodoWrite",
    "ExitPlanMode",
    "AskUser",
    "SetTitle",
    "SendText",
    "SearchMessage",
    "ExtractMessage",
];

#[derive(Clone)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        ToolRegistry {
            tools: tools.into_iter().map(|t| (t.name().to_string(), t)).collect(),
        }
    }

    /// The same registry with `more` added: the tools of the session's MCP
    /// servers on top of the core's own. A name already taken stays with the
    /// tool that has it — nothing a server sends may replace a built-in tool.
    pub fn with(mut self, more: Vec<Arc<dyn Tool>>) -> Self {
        for tool in more {
            self.tools.entry(tool.name().to_string()).or_insert(tool);
        }
        self
    }

    /// The core's full set.
    pub fn standard() -> Self {
        Self::new(vec![
            Arc::new(read_file::ReadFile),
            Arc::new(read_image::ReadImage),
            Arc::new(glob::Glob),
            Arc::new(grep::Grep::new()),
            Arc::new(web_fetch::WebFetch),
            Arc::new(web_search::WebSearch::default()),
            Arc::new(todo_write::TodoWrite),
            Arc::new(set_title::SetTitle),
            Arc::new(send_text::SendText),
            Arc::new(exit_plan_mode::ExitPlanMode),
            Arc::new(ask_user::AskUser),
            Arc::new(write_file::WriteFile),
            Arc::new(memory::Memory),
            Arc::new(skill::Skill),
            Arc::new(edit_file::EditFile),
            Arc::new(delete_file::DeleteFile),
            Arc::new(delete_dir::DeleteDir),
            Arc::new(delete_file::MoveFile),
            Arc::new(bash::Bash),
            Arc::new(bash_output::BashOutput),
            Arc::new(bash_control::BashKill),
            Arc::new(bash_control::BashRestart),
            Arc::new(task::Task),
        ])
    }

    /// The tools of a session: the core's set, and with a journal the ones
    /// that read it. A session that writes nothing down has no journal to
    /// read, and a tool that could only fail is not offered.
    pub fn of_session(journaled: bool) -> Self {
        let all = Self::standard();
        if !journaled {
            return all;
        }
        all.with(vec![
            Arc::new(search_message::SearchMessage),
            Arc::new(extract_message::ExtractMessage),
        ])
    }

    /// What a subagent works with: everything the agent has, less what
    /// belongs to the agent alone. `Task`, because the agent is the one who
    /// splits the work; the task list and the way out of plan mode, because
    /// §5.6 and §7.1 draw one of each and it is the agent's; `SetTitle`, because a conversation has one title and the
    /// agent is the one who knows what it is about; `AskUser`, because
    /// the agent sees the whole task and is the one who puts choices to the
    /// human, not a worker deep inside it; `SendText`, because what is put in
    /// front of the human is part of the agent's answer to them;
    /// `SearchMessage` and `ExtractMessage`, because the conversation they
    /// read is the agent's.
    ///
    /// Taken from the agent's registry rather than built anew, so a subagent
    /// has the same MCP tools the agent has.
    pub fn delegated(&self) -> Self {
        let mut all = self.clone();
        for own in AGENT_ONLY {
            all.tools.remove(own);
        }
        all
    }

    /// The tool, if it exists at all in this mode.
    ///
    /// The mode is not decoration: §7.1 does not offer writing tools to the model in
    /// «Плане», and a `write_file` it made up must get «неизвестный
    /// инструмент», not a permission request. Asking about an action that does not exist
    /// in this mode is an interface lying about the state (§0).
    pub fn get(&self, name: &str, intent: Intent) -> Option<Arc<dyn Tool>> {
        self.tools
            .get(name)
            .filter(|t| intent.allows(t.intents()))
            .cloned()
    }

    /// Every tool name the registry knows, in its order, whatever the mode.
    /// Hook settings are checked against these: a name the registry does not
    /// know would never match a call.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.tools.keys().map(String::as_str)
    }

    /// Schemas for the request to the model.
    ///
    /// The order is deterministic (`BTreeMap`): the schemas go first in the request,
    /// and shuffling the list would reset the prompt cache on every turn.
    pub fn schemas(&self, intent: Intent) -> Vec<ToolSchema> {
        self.tools
            .values()
            .filter(|t| intent.allows(t.intents()))
            .map(|t| {
                let mut schema = t.input_schema().unwrap_or_else(|| {
                    json!({
                        "type": "object",
                        "properties": t.properties(),
                        "required": t.required(),
                        "additionalProperties": false,
                    })
                });

                if t.narration() == Narration::Model {
                    if let Some(props) = schema["properties"].as_object_mut() {
                        props.insert(
                            "description".into(),
                            json!({
                                "type": "string",
                                "maxLength": DESCRIPTION_MAX_CHARS,
                                "description": DESCRIPTION_FIELD_DOC,
                            }),
                        );
                    }
                    // A ready-made schema may have no `required` at all.
                    match schema["required"].as_array_mut() {
                        Some(required) => required.push(json!("description")),
                        None => schema["required"] = json!(["description"]),
                    }
                }

                ToolSchema {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    input_schema: schema,
                }
            })
            .collect()
    }
}

/// Takes the model's caption out of the arguments and brings it to the form of §5.4.1.
///
/// A length violation is cut rather than failing the turn: the caption is decoration for the feed,
/// and losing a tool's work over it is not acceptable.
pub fn take_description(input: &mut Value) -> Option<String> {
    let raw = input
        .as_object_mut()
        .and_then(|o| o.remove("description"))?
        .as_str()?
        .to_string();

    let one_line = raw.split('\n').next().unwrap_or("").trim();
    let trimmed = one_line.trim_end_matches(['.', '。']).trim();
    if trimmed.is_empty() {
        return None;
    }

    let cut: String = if trimmed.chars().count() > DESCRIPTION_MAX_CHARS {
        trimmed
            .chars()
            .take(DESCRIPTION_MAX_CHARS - 1)
            .collect::<String>()
            .trim_end()
            .to_string()
            + "…"
    } else {
        trimmed.to_string()
    };
    Some(cut)
}

/// Shared scaffolding for tool tests: a real directory, a real workspace,
/// a live event bus. There are no mocks here on purpose — a tool that lies about the
/// file system is caught only on the file system.
/// Shared scaffolding for tool tests: a real directory, a real workspace,
/// a live event bus. There are no mocks here on purpose — a tool that lies about the
/// file system is caught only on the file system.
#[cfg(test)]
pub(crate) mod tests_support {
    use std::sync::Arc;

    use tokio::sync::broadcast;

    use super::{Progress, ToolCtx};
    use crate::event::{AgentEvent, EventSink};
    use crate::id::CallId;
    use crate::process::Processes;
    use crate::testutil::TempDir;
    use crate::workspace::Workspace;

    fn build(dir: &TempDir, tx: broadcast::Sender<AgentEvent>) -> (ToolCtx, Arc<Workspace>) {
        build_on(Arc::new(Workspace::open(dir.path()).unwrap()), tx)
    }

    /// A context over a workspace the test has already shaped, e.g. with
    /// additional roots.
    pub fn ctx_on(ws: Workspace) -> (ToolCtx, Arc<Workspace>) {
        let (tx, _rx) = broadcast::channel(64);
        build_on(Arc::new(ws), tx)
    }

    fn build_on(ws: Arc<Workspace>, tx: broadcast::Sender<AgentEvent>) -> (ToolCtx, Arc<Workspace>) {
        let events = EventSink::new(tx);
        let ctx = ToolCtx {
            processes: Processes::new(events.clone()),
            progress: Progress::new(CallId::new(), events.clone()),
            ws: ws.clone(),
            events,
            cancel: tokio_util::sync::CancellationToken::new(),
            agents: None,
            questions: None,
            vision: true,
            shell: crate::process::shell::Shell::default(),
            respect_robots: true,
            title_locked: false,
            transcript: None,
            lsp: None,
        };
        (ctx, ws)
    }

    pub fn ctx_for(dir: &TempDir) -> (ToolCtx, Arc<Workspace>) {
        let (tx, _rx) = broadcast::channel(64);
        build(dir, tx)
    }

    pub fn ctx_with_events(dir: &TempDir) -> (ToolCtx, broadcast::Receiver<AgentEvent>) {
        let (tx, rx) = broadcast::channel(64);
        let (ctx, _ws) = build(dir, tx);
        (ctx, rx)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod roots_tests;

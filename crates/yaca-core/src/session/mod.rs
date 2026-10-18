//! Session: commands in, event stream out.
//!
//! Built from two tasks, and this is not decoration. The dispatcher reads commands
//! and must answer instantly — otherwise `Esc` does not stop the agent while
//! the agent is busy. The worker runs the turn and owns the history alone, so
//! the history is not handed to anyone under a mutex.
//!
//! Cut along the seams of the loop itself: here are the session's layout and the dispatcher, in
//! [`turn`] — the agent's turn from message to stop, in [`calls`] — running
//! tools, in [`asking`] — the permission question before them, in [`checkpoints`] — working-tree snapshots
//! and rollback, in [`compaction`] — history compaction during the work, in [`inbox`] —
//! the worker's queue and the messages that merge into the running turn (§7.4), in
//! [`subagent`] — the conversation that the `Task` tool holds inside a turn, in
//! [`background`] — the subagents the agent does not wait for. Messages
//! not yet merged into a turn live in [`queue`]: cancelling and merging are the same
//! record under one lock.

pub mod accumulator;
mod asking;
mod background;
pub mod binding;
mod calls;
mod changeset;
mod checkpoints;
pub mod compact;
mod compaction;
mod plugin_hooks;
mod environment;
mod guard;
mod inbox;
pub mod log;
mod mentions;
pub mod prompt;
mod queue;
pub mod questions;
pub(crate) mod retry;
pub mod subagent;
mod turn;
mod vision;
mod window;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::change::ChangeSet;
use crate::checkpoint::Checkpoints;
use crate::command::Command;
use crate::error::CoreError;
use crate::event::{AgentEvent, EventSink, SessionState};
use crate::id::{QueuedId, RequestId, SessionId};
use crate::intent::{Effort, Mode};
use crate::lsp::{Lsp, LspServerSettings, LspServerSummary};
use crate::mcp::{Mcp, McpServerSettings, McpServerSummary};
use crate::pending::Pending;
use crate::permission::broker::Broker;
use crate::permission::policy::Policy;
use crate::permission::PermissionAnswer;
use crate::question::{QuestionReply, QuestionRequest};
use crate::process::shell::{Shell, ShellChoice};
use crate::process::Processes;
use crate::provider::registry::ANTHROPIC_ID;
use crate::provider::Provider;
use crate::settings::Settings;
use crate::store::{Recorder, SessionMeta, SessionStore};
use subagent::Ledger;
use crate::tool::ToolRegistry;
use crate::usage::{catalog, ModelInfo, Pricing, UsageMeter, DEFAULT_MODEL};
use crate::workspace::Workspace;

/// What the worker is asked to do. More than "another message" since §5.11:
/// compaction owns the history, and the history belongs to the worker alone.
pub(crate) enum Job {
    /// A message waiting in [`queue::Queue`]: the job says where to look, and
    /// the queue hands the text over only if it has not been cancelled.
    Message { id: QueuedId },
    /// "Compact" (§5.11).
    Compact,
    /// The human rewrote the summary before continuing (§5.11).
    EditSummary { text: String },
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub root: PathBuf,
    /// Empty when the source of [`SessionConfig::model`] did not name one:
    /// then the model decides, see [`Providers::resolve`].
    ///
    /// [`Providers::resolve`]: crate::provider::registry::Providers::resolve
    pub provider: String,
    pub model: String,
    /// The provider's model this session talks to (§10).
    ///
    /// Not the global catalog: the context window and price are taken from here, while
    /// an arbitrary provider has its own. A model that is not in the list
    /// counts as unknown — free, with a conservative window.
    pub models: Vec<ModelInfo>,
    pub mode: Mode,
    pub effort: Effort,
    /// The chats folder the session is journaled into (`store::chats_dir` in
    /// the applications). `None`: nothing is written down. Application-level,
    /// like `mcp_approvals`: the core does not reach for the human's folders
    /// on its own, so a test journals into a folder of its own.
    pub journal: Option<PathBuf>,
    /// Continue an existing session instead of starting one (§9).
    ///
    /// The conversation comes back from `<id>.messages.jsonl` and the journals
    /// are appended to, so the session keeps its id, its title and its cost.
    /// Needs [`SessionConfig::journal`]: a session with nothing written down
    /// has nothing to come back to.
    pub resume: Option<SessionId>,
    /// Whether to respect `Disallow` from `robots.txt` when loading a page (§10).
    ///
    /// An application setting, so it arrives here rather than being read from
    /// `.yaca/settings.json`: a project does not decide how Yaca behaves on the network.
    /// The default is to respect it; `yaca-cli` gets it without knowing about
    /// window settings.
    pub respect_robots: bool,
    /// The small model the `Decision` tool asks. `None`: the tool is not
    /// offered, because one that could only refuse is a line in every request
    /// for nothing. Application-level, like the providers it is built from.
    pub decision: Option<crate::tool::decision::DecisionSetup>,
    /// Which shell commands run through (§10). The human's choice about the
    /// application, like the flag above, and settled for the whole session
    /// when it opens ([`Shell`]).
    pub shell: ShellChoice,
    /// Work in a git worktree of this chat's own, on its own branch
    /// (`isolation`), so that another chat of the project cannot change the
    /// tree under it or be rolled back by it.
    ///
    /// Off by default: the worktree starts from `HEAD` and leaves out what is
    /// uncommitted. Asking for it in a project that is not a repository, or has
    /// no commit, fails at start rather than falling back to the shared tree.
    /// A resumed chat that already has its worktree gets it whatever this says.
    pub isolate: bool,
    /// Further folders the session may work in, besides [`SessionConfig::root`]
    /// (`yaca --add-dir`). Their files are addressed by absolute path. The
    /// project's own `.yaca`, instructions and checkpoints stay with the root.
    ///
    /// Fixed for the session: a resumed session must be given the same set, or
    /// [`spawn`] refuses it.
    pub additional_roots: Vec<PathBuf>,
    /// The human's own MCP servers, the same for every project
    /// (`mcp::config`). Application-level, so it arrives here like the provider
    /// does: the caller reads `mcp.json` from the configuration directory. The
    /// project's servers are added to these by [`spawn`].
    pub mcp_servers: BTreeMap<String, McpServerSettings>,
    /// The configuration directory, as the place where approvals of servers
    /// from the project's `.mcp.json` are kept between sessions. `None`: an
    /// approval lasts for the session it was given in.
    pub mcp_approvals: Option<PathBuf>,
    /// The human's own language servers (`lsp::config`), on top of the
    /// built-in ones found on `PATH`. Application-level like `mcp_servers`:
    /// a project does not get to declare a program to be run.
    pub lsp_servers: BTreeMap<String, LspServerSettings>,
    /// Whether the built-in servers are looked for on `PATH`. Off, only the
    /// human's own `lsp_servers` exist. Off by default, so that what a session
    /// offers the model does not depend on what is installed on the machine
    /// the core happens to run on; the applications turn it on, like they
    /// fill `mcp_servers`.
    pub lsp_autodetect: bool,
    /// The human's own skill directories (`skills::user_dirs`), scanned after
    /// the project's. Application-level like `mcp_servers`: the applications
    /// fill it, and a test session sees only the project's skills.
    pub user_skills: Vec<PathBuf>,
    /// The human's own agent directories (`agents::user_dirs`), scanned after
    /// the project's. Filled by the applications, like `user_skills`.
    pub user_agents: Vec<PathBuf>,
    /// Where remote MCP servers' OAuth logins are kept: the keychain, or a
    /// folder in tests.
    pub mcp_tokens: crate::mcp::oauth::TokenStore,
    /// The plugins the human enabled (`plugin`), loaded by the application:
    /// their skills, commands, MCP servers and hooks join the session's. Empty
    /// in a test session, like `user_skills`.
    pub plugins: Vec<crate::plugin::Plugin>,
}

impl SessionConfig {
    /// The built-in defaults, with no project consulted.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        SessionConfig {
            root: root.into(),
            provider: ANTHROPIC_ID.to_string(),
            model: DEFAULT_MODEL.to_string(),
            models: catalog(),
            mode: Mode::default(),
            effort: Effort::default(),
            journal: None,
            resume: None,
            respect_robots: true,
            decision: None,
            shell: ShellChoice::default(),
            isolate: false,
            additional_roots: Vec::new(),
            mcp_servers: BTreeMap::new(),
            mcp_approvals: None,
            lsp_servers: BTreeMap::new(),
            lsp_autodetect: false,
            user_skills: Vec::new(),
            user_agents: Vec::new(),
            mcp_tokens: crate::mcp::oauth::TokenStore::default(),
            plugins: Vec::new(),
        }
    }

    /// The defaults the project asked for in `.yaca/settings.json` (§9, §10).
    ///
    /// Separate from [`SessionConfig::new`] and applied before the caller's own
    /// overrides, so that "the settings said Auto" and "this caller asked for
    /// Auto" never have to be told apart after the fact.
    ///
    /// No model is chosen unless the project names one. The built-in default
    /// of [`SessionConfig::new`] belongs to a provider that may never have been
    /// set up; left empty, the pair is settled by the provider list, which
    /// starts a new chat on the first provider that answered (§10).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, CoreError> {
        let mut config = SessionConfig::new(root);
        config.provider.clear();
        config.model.clear();
        let settings = Settings::load(&config.root)?;
        if let Some(mode) = settings.mode {
            config.mode = mode;
        }
        config.choose_model(
            settings.provider.unwrap_or_default(),
            settings.model.unwrap_or_default(),
        );
        Ok(config)
    }

    /// Takes a model together with the provider it came with.
    ///
    /// The two are one choice and are replaced together. A model taken from
    /// one place with the provider left over from another — the built-in
    /// `anthropic` under a session's local model — is a pair nobody chose.
    /// Either half may be empty and stays empty; the provider list fills it in
    /// later ([`Providers::resolve`]). A source that says neither changes
    /// nothing.
    ///
    /// [`Providers::resolve`]: crate::provider::registry::Providers::resolve
    pub fn choose_model(&mut self, provider: String, model: String) {
        if provider.is_empty() && model.is_empty() {
            return;
        }
        self.provider = provider;
        self.model = model;
    }

    /// Continue a recorded session (§9), on the model it was talking to.
    ///
    /// Over the project's settings and under whatever the caller picks next:
    /// the conversation was held with that model, and the project default may
    /// have changed since. A session recorded before sessions wrote their
    /// provider down brings only the model, and [`SessionConfig::choose_model`]
    /// lets the model decide.
    ///
    /// Looks in [`SessionConfig::journal`], which is set first.
    pub fn resume_session(&mut self, id: SessionId) {
        let meta = self
            .journal
            .as_deref()
            .and_then(|chats| SessionStore::open(chats, &self.root).meta(&id));
        if let Some(meta) = meta {
            self.choose_model(meta.provider, meta.model);
        }
        self.resume = Some(id);
    }
}

/// A late subscriber, standing at a point in a session's stream. See
/// [`SessionHandle::attach`].
pub struct Attach {
    /// Every event from the stand on.
    pub events: broadcast::Receiver<AgentEvent>,
    /// Everything before it.
    pub replay: Replay,
}

/// What came before the stand, to be read with [`SessionHandle::replay`]. Kept
/// apart from the receiver because reading it is blocking work that belongs on
/// another thread, while the receiver stays where the events are awaited.
#[derive(Debug, Clone, Copy)]
pub struct Replay {
    /// Where the journal ended at the stand. `None` — the session is not
    /// written down, so there is nothing before it to read.
    journal: Option<u64>,
}

pub struct SessionHandle {
    id: SessionId,
    commands: mpsc::Sender<Command>,
    events: broadcast::Sender<AgentEvent>,
    shared: Arc<Shared>,
}

impl SessionHandle {
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.events.subscribe()
    }

    /// The current state without waiting for an event: a subscriber arriving later
    /// than the start would otherwise not know which mode the session is in.
    pub fn state(&self) -> SessionState {
        self.shared.state.lock().unwrap().clone()
    }

    /// This session's line in the panel (§9). `None` — the session is not
    /// being written down, so it has no line.
    pub fn meta(&self) -> Option<SessionMeta> {
        self.shared.recorder.as_ref().map(|r| r.meta())
    }

    /// §9: rename, for a session that is open. The index of a running session
    /// is this session's to write, so the name goes in here and not past it
    /// into the file (`SessionStore::rename`), where the next turn would
    /// write the old one back.
    pub fn rename(&self, title: &str) -> Result<(), CoreError> {
        let recorder = self.shared.recorder.as_ref().ok_or_else(|| {
            CoreError::Config(format!("session {} is not recorded", self.id()))
        })?;
        recorder.rename(title)?;
        Ok(())
    }

    /// The MCP servers as they are now, for the same late subscriber
    /// [`SessionHandle::state`] is for: servers start connecting when the
    /// session does, before anyone listens.
    pub fn mcp(&self) -> Vec<McpServerSummary> {
        self.shared.mcp.snapshot()
    }

    /// Stands a new subscriber at this point in the stream, for a window that
    /// lost its feed while the session kept running (a reload).
    ///
    /// The channel carries everything from here on. Everything before is
    /// [`SessionHandle::replay`] of the returned [`Replay`], and the two do not
    /// overlap: the journal is read up to the length it had at this point.
    pub fn attach(&self) -> Attach {
        let (events, journal) = self
            .shared
            .events
            .attach(|| self.shared.recorder.as_ref().map(|r| r.journal_len()));
        Attach {
            events,
            replay: Replay { journal },
        }
    }

    /// The events a window has to fold to be where the session was at the stand.
    ///
    /// The journal as written, then the state that is never written down —
    /// mode and model, MCP and language servers, the background commands, the
    /// queue, the checkpoint list. The journal is read from disk and the rest
    /// is asked of the live session, so call this off the async runtime.
    ///
    /// The live part is taken after the stand, not at it. Each of those
    /// events is a whole list, and any change between the two reaches the
    /// subscriber as a later whole list, so the last word is the right one
    /// whichever way the race went.
    pub fn replay(&self, at: &Replay) -> Vec<AgentEvent> {
        let mut events = match (&self.shared.recorder, at.journal) {
            (Some(recorder), Some(len)) => recorder.events_to(len),
            _ => Vec::new(),
        };
        events.push(AgentEvent::StateChanged { state: self.state() });
        events.push(AgentEvent::McpUpdated { servers: self.mcp() });
        // A server's form still waiting: the journal does not hold it, and
        // the window must be able to answer it again.
        for request in self.shared.mcp.open_elicitations() {
            events.push(AgentEvent::ElicitationRequested { request });
        }
        events.push(AgentEvent::LspUpdated { servers: self.lsp() });
        events.push(AgentEvent::ProcessesUpdated {
            processes: self.shared.processes.summaries(),
        });
        events.push(AgentEvent::QueueUpdated {
            queued: self.shared.queue.waiting(),
        });
        events.push(AgentEvent::CheckpointsUpdated {
            checkpoints: self.shared.checkpoints.list(),
        });
        events
    }

    /// The language servers of this session and the state each is in.
    pub fn lsp(&self) -> Vec<LspServerSummary> {
        self.shared.lsp.snapshot()
    }

    pub async fn send(&self, command: Command) -> Result<(), CoreError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| CoreError::Cancelled)
    }
}

struct Shared {
    state: Mutex<SessionState>,
    /// Token of the current turn. Replaced on every turn.
    cancel: Mutex<CancellationToken>,
    events: EventSink,
    /// Background commands (§6.5). Also shared: "Stop" must work
    /// while the agent is busy with something else.
    processes: Processes,
    /// Messages sent during a turn and not yet taken in (§7.4). Shared, because
    /// a cancel comes from the dispatcher while the worker is mid-turn.
    queue: queue::Queue,
    /// The shadow repository (§5.8). Shared for the same reason: a rollback is
    /// a command from the panel, and the panel is not waiting for the turn.
    checkpoints: Checkpoints,
    /// The hunks of this session that are still pending (§6.3). Shared so a
    /// decision from the panel and a write from the agent meet at one set.
    changes: ChangeSet,
    /// Who asks for permission and remembers the rules issued (§5.7).
    broker: Broker,
    /// `AskUser` questions on screen. Shared with the dispatcher, like permissions:
    /// the answer arrives as a command, while the tool waiting for it runs in the worker. The question itself sits
    /// next to the channel, so the answer can be checked before delivery.
    questions: Pending<RequestId, QuestionRequest, QuestionReply>,
    /// The provider's model for this session — the source of the context window and prices (§10).
    models: Vec<ModelInfo>,
    ws: Arc<Workspace>,
    /// The worktree this session works in, when it is isolated.
    isolation: Option<crate::isolation::Isolation>,
    /// The three files of this session (§9). `None` — nothing is written down,
    /// which is what a one-shot CLI run wants.
    recorder: Option<Arc<Recorder>>,
    /// `.yaca/settings.json` as it was when the session opened. Read for what
    /// the core decides on its own — which model summarizes history (§5.11).
    settings: Settings,
    /// What subagents spent and the worker has not counted yet.
    ledger: Arc<Ledger>,
    /// Subagents the running turn sent in the background (`background`).
    background: background::Background,
    /// Held by a call that may not run beside another (`calls`).
    lane: tokio::sync::Mutex<()>,
    /// Models that turned an image down in this session (`vision`).
    blind: Mutex<std::collections::HashSet<String>>,
    /// Calls of tools that are `silent`: their events are dropped in `emit`.
    /// Call ids are unique, so entries are never reused and not worth removing.
    silent: Mutex<std::collections::HashSet<String>>,
    /// Whether to respect `robots.txt` (§10). From [`SessionConfig`], not from the
    /// project file: an application setting.
    respect_robots: bool,
    /// What `Decision` asks, when there is something to ask.
    decision: Option<crate::tool::decision::DecisionSetup>,
    /// The shell of this session's commands, named in its system prompt.
    shell: Shell,
    /// The MCP servers of this session (`mcp`). Shared: reconnecting is a
    /// command from the panel, and a tool call reaches its server through here.
    mcp: Mcp,
    /// The language servers of this session (`lsp`). Started by the first
    /// call that needs one.
    lsp: Lsp,
    /// Where this session's skills are found (`skills`). Scanned again by
    /// every `Skill` call and `/name`; the prompt's catalog is read once.
    skills: Arc<crate::skills::SkillRoots>,
    /// The named subagents, read when the session opened (`agents`).
    agents: Arc<Vec<crate::agents::AgentDef>>,
    /// The plugins' commands and hooks (`plugin_hooks`).
    plugins: plugin_hooks::PluginSide,
}

impl Shared {
    /// What is known about the model: from the provider's list, not from the global
    /// catalog. An unfamiliar model counts as free, with a conservative window —
    /// §8 promises to work with any model, including ones Yaca has not
    /// heard of.
    fn model_info(&self, id: &str) -> ModelInfo {
        self.models
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .unwrap_or_else(|| crate::usage::unknown_model(id))
    }

    /// The price of one request. A routed request is priced by the model that
    /// answered, which the configured (virtual) model knows nothing of. The
    /// lookup is by id and provider together: the same id can be listed under
    /// two providers at different prices. `fallback` is for an unrouted
    /// request, or a routed model that is not in the list.
    ///
    /// One rule for the agent, its subagents and compaction: priced apart, a
    /// routed request would be billed at the virtual model's price in two of
    /// the three places and the meter would disagree with itself.
    fn pricing_of(&self, routed: Option<&(String, String)>, fallback: Pricing) -> Pricing {
        routed
            .and_then(|(provider, model)| {
                self.models
                    .iter()
                    .find(|m| m.id == *model && m.provider_id == *provider)
            })
            .map_or(fallback, |m| m.pricing)
    }

    /// The tools of this session: the core's set, `more` on top, and `Decision`
    /// when there is a model to ask. One place for both the first registry and
    /// the one rebuilt per turn, or a turn would quietly lose the tool.
    fn tools(&self, mut more: Vec<Arc<dyn crate::tool::Tool>>) -> ToolRegistry {
        if let Some(setup) = &self.decision {
            more.push(Arc::new(crate::tool::decision::Decision::new(setup.clone())));
        }
        ToolRegistry::of_session(self.recorder.is_some()).with(more)
    }

    fn emit(&self, event: AgentEvent) {
        // Dropped before the journal as well: a reopened session must not
        // draw a row the live one never had.
        if let Some(call) = event.call_id() {
            if self.silent.lock().unwrap().contains(&call.0) {
                return;
            }
        }
        // Written down and sent as one step (`EventSink::emit_after`), or a
        // window attaching to a running session could see an event in both its
        // journal and its channel, or in neither.
        self.events.emit_after(event, |event| {
            if let Some(recorder) = &self.recorder {
                recorder.event(event);
            }
        });
    }

    /// The sink a tool sends its own events through. They go by the journal
    /// like everything [`Shared::emit`] sends: the index takes the title from
    /// `TitleSet`, and a reopened session draws its tasks and shown text from
    /// what was written down.
    fn tool_events(&self) -> EventSink {
        match &self.recorder {
            Some(recorder) => {
                let recorder = recorder.clone();
                self.events.journaled(move |event| recorder.event(event))
            }
            None => self.events.clone(),
        }
    }

    fn emit_state(&self) {
        let state = self.state.lock().unwrap().clone();
        self.emit(AgentEvent::StateChanged { state });
    }

    /// The §8 trust matrix as currently configured.
    fn policy(&self) -> Policy {
        let state = self.state.lock().unwrap();
        Policy {
            trust: state.mode.trust(),
            bypass: state.bypass,
        }
    }

}

/// A resumed session comes back with the folders it was started with, and no
/// others. The conversation was held with those roots; a different set is
/// refused here rather than quietly used.
///
/// A journal that has no sidecar (`store::rebuild`) does not record the roots,
/// so it reads as having none: see the README.
fn check_resumed_roots(
    store: &SessionStore,
    id: &SessionId,
    ws: &Workspace,
) -> Result<(), CoreError> {
    let recorded = store.meta(id).map(|meta| meta.additional_roots).unwrap_or_default();
    let given = ws.additional_text();
    if recorded == given {
        return Ok(());
    }
    let list = |roots: &[String]| {
        if roots.is_empty() {
            "none".to_string()
        } else {
            roots.join(", ")
        }
    };
    Err(CoreError::Config(format!(
        "session {id} was opened with additional roots: {}; requested: {}. Roots cannot be changed during a session",
        list(&recorded),
        list(&given)
    )))
}

/// Starts the session and returns a handle to it.
pub fn spawn(
    config: SessionConfig,
    provider: Arc<dyn Provider>,
) -> Result<SessionHandle, CoreError> {
    // Settings first: the workspace needs the project's snapshot exclusions
    // before it is shared, so that the permission questions see them (§8).
    let root = Workspace::open(&config.root)?;
    let settings = Settings::load(root.root())?;
    let root = root.with_shadow_exclude(&settings.shadow.exclude);
    let store = config
        .journal
        .as_deref()
        .map(|chats| SessionStore::open(chats, root.home()));
    if let Some(store) = &store {
        store.sweep_drafts();
    }

    // Resuming keeps the id: §9 shows one session in the panel, not a chain of
    // fragments, and cost and title are already attached to it.
    let (id, history, prefix) = match &config.resume {
        Some(id) => {
            let store = store.as_ref().ok_or_else(|| {
                CoreError::Config(format!(
                    "cannot resume session {id} without journal: chats folder is not specified"
                ))
            })?;
            let records = store.records(id);
            let prefix = crate::store::history::prefix(&records);
            (id.clone(), crate::store::history::messages(records), prefix)
        }
        None => (SessionId::new(), Vec::new(), None),
    };

    // After the id, which names the worktree. The tools and the snapshots
    // work in it from here on; the journals stay keyed to the project.
    let isolation = crate::isolation::open(root.home(), &id, config.isolate)?;
    // The additional roots are checked against the tree the session works in,
    // so after `in_tree`: a folder inside the worktree is redundant there.
    let ws = Arc::new(
        match &isolation {
            Some(isolation) => root.in_tree(&isolation.path)?,
            None => root,
        }
        .with_roots(&config.additional_roots)?,
    );
    if let (Some(resumed), Some(store)) = (&config.resume, &store) {
        check_resumed_roots(store, resumed, &ws)?;
    }

    let (event_tx, _) = broadcast::channel(4_096);
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (work_tx, work_rx) = mpsc::channel(64);

    let recorder = match (&config.journal, &store) {
        (Some(chats), Some(store)) => {
            let mut meta = store
                .meta(&id)
                .unwrap_or_else(|| SessionMeta::new(id.clone(), config.provider.clone(), config.model.clone()));
            meta.additional_roots = ws.additional_text();
            crate::store::location::claim(chats, ws.home())
                .ok()
                .and_then(|dir| Recorder::open(&dir, meta).ok())
                .map(Arc::new)
        }
        _ => None,
    };

    // Cost and tokens carry over, or a resumed session would report a day of
    // work as free (§7.1).
    let usage = recorder
        .as_ref()
        .map(|r| r.meta())
        .filter(|_| config.resume.is_some())
        .map(|m| UsageMeter::resumed(m.tokens, m.cost_usd))
        .unwrap_or_default();

    let events = EventSink::new(event_tx.clone());
    // The project's folder, not the worktree: `.mcp.json` is the project's,
    // and an approval is given to the project. The servers themselves start in
    // the tree the session works in.
    let mcp = {
        let project_file = crate::mcp::config::load_project_file(ws.home())?;
        let mut specs =
            crate::mcp::config::merge(&config.mcp_servers, &project_file, &settings.mcp_servers);
        specs.extend(crate::plugin::mcp_specs(&config.plugins));
        let mut roots = vec![ws.root().to_path_buf()];
        roots.extend(ws.additional_text().into_iter().map(PathBuf::from));
        Mcp::start(
            specs,
            ws.home(),
            ws.root(),
            roots,
            events.clone(),
            config.mcp_approvals.as_deref(),
            config.mcp_tokens.clone(),
        )
    };
    let lsp = Lsp::new(
        crate::lsp::config::resolve(&config.lsp_servers, |program| {
            config.lsp_autodetect && crate::lsp::config::on_path(program)
        }),
        ws.root(),
        events.clone(),
    );
    let broker = Broker::new(ws.root());
    // Standing rules from the project (§5.7, §10). Neither "Always allow" nor
    // "Always deny" writes here — a rule granted in a hurry does not outlive
    // the hurry — so whatever is in the file was put there on purpose, through
    // the settings window.
    for rule in &settings.permissions.rules {
        broker.grant(rule.clone());
    }
    for deny in &settings.permissions.denies {
        broker.refuse(deny.scope.clone(), deny.reason.clone());
    }

    let shared = Arc::new(Shared {
        processes: Processes::new(events.clone()),
        queue: queue::Queue::new(events.clone()),
        // Hard, not optional. §5.8 is not a nicety, and a session that cannot
        // snapshot is a session where applying edits and running commands are
        // both irreversible — better said now than discovered later.
        changes: ChangeSet::new(),
        // Snapshots cover the primary root only (`Workspace::root`). What an
        // additional root holds is never saved, and the questions say so
        // (`permission::unsaved`).
        //
        // The list of points is kept next to the journals, so a reload of the
        // window, a reopened chat and a restart all find the points the
        // session had. A run that is not written down keeps it in memory.
        checkpoints: Checkpoints::open_in(
            ws.root(),
            ws.home(),
            &settings.shadow.exclude,
            config.journal.as_deref().map(|chats| {
                crate::checkpoint::ledger::path(
                    &crate::store::location::project_dir(chats, ws.home()),
                    &id,
                )
            }),
        )
        .map_err(|e| {
            CoreError::Config(format!(
                "cannot open shadow repository in {}: {e}",
                ws.root().display()
            ))
        })?,
        broker,
        questions: Pending::default(),
        respect_robots: config.respect_robots,
        decision: config.decision.clone(),
        shell: Shell::resolve(config.shell),
        mcp,
        lsp,
        skills: Arc::new(
            crate::skills::SkillRoots::new(ws.home(), config.user_skills.clone())
                .with_plugins(&config.plugins),
        ),
        agents: Arc::new(
            crate::agents::AgentRoots::new(ws.home(), config.user_agents.clone())
                .with_plugins(&config.plugins)
                .scan(),
        ),
        plugins: plugin_hooks::PluginSide {
            hooks: crate::plugin::hooks::Hooks::new(&config.plugins),
            commands: crate::plugin::commands(&config.plugins),
            session_id: id.to_string(),
            transcript: recorder.as_ref().map(|r| r.messages_file()),
        },
        models: config.models.clone(),
        ws: ws.clone(),
        isolation: isolation.clone(),
        recorder,
        settings,
        ledger: Arc::default(),
        background: Default::default(),
        lane: tokio::sync::Mutex::new(()),
        blind: Mutex::default(),
        silent: Mutex::default(),
        state: Mutex::new(SessionState {
            // The project, not the worktree: the window files the chat under
            // the project it was opened in.
            project_root: ws.home().to_string_lossy().to_string(),
            provider: config.provider.clone(),
            model: config.model.clone(),
            mode: config.mode,
            effort: config.effort,
            busy: false,
            bypass: false,
            isolation,
        }),
        cancel: Mutex::new(CancellationToken::new()),
        events,
    });
    // Once per session start, off the start path: a walk of `.yaca/shadow` is
    // not something the first prompt should wait for (§5.8).
    shared.checkpoints.sweep_in_background();

    // A server's form is a wait for the human like a question: the
    // `Notification` hooks hear of it. Weak, or the hub would keep the
    // session it belongs to alive.
    let weak = Arc::downgrade(&shared);
    shared.mcp.on_waiting(Arc::new(move |server: &str| {
        if let Some(shared) = weak.upgrade() {
            shared.notify("elicitation", format!("The MCP server {server} needs your input"));
        }
    }));

    tokio::spawn(dispatch(cmd_rx, work_tx, shared.clone()));
    tokio::spawn(
        turn::Worker {
            instructions: crate::instructions::load(&ws).map(Arc::new),
            memory: crate::tool::memory::store::load(&ws).map(Arc::new),
            skills: shared.skills.scan().listing().map(Arc::new),
            agents_section: crate::agents::section(&shared.agents).map(Arc::from),
            plugin_context: None,
            ws,
            provider,
            registry: Arc::new(shared.tools(Vec::new())),
            mcp_seen: None,
            mcp_notes: None,
            // A resumed session opens compacted if that is how it was left,
            // which is why the summary marker is looked for rather than
            // assumed absent.
            compacted: compact::is_summary(history.first()),
            // Nothing measured yet: a reopened history is estimated whole.
            measured: 0,
            history,
            prefix,
            usage,
            shared: shared.clone(),
        }
        .run(work_rx),
    );

    Ok(SessionHandle {
        id,
        commands: cmd_tx,
        events: event_tx,
        shared,
    })
}

/// The dispatcher. Nothing long-running may happen here.
async fn dispatch(mut rx: mpsc::Receiver<Command>, work: mpsc::Sender<Job>, shared: Arc<Shared>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::SendMessage { text, attachments } => {
                // A message sent while work is running goes into the queue (§7.4). The chip
                // is shown only when the worker is busy: an idle one
                // takes the message at once, and the chip would flash a lie about waiting.
                let busy = shared.state.lock().unwrap().busy;
                let id = shared.queue.push(text, attachments, busy);
                if work.send(Job::Message { id }).await.is_err() {
                    break;
                }
            }

            // Both belong to the worker: it owns the history, and §5.11 is
            // entirely about the history. Queued behind the current turn —
            // only a message joins a running one (§7.4) — because compacting
            // mid-turn would cut the conversation out from under the model
            // answering it.
            Command::Compact => {
                if work.send(Job::Compact).await.is_err() {
                    break;
                }
            }
            Command::EditSummary { text } => {
                if work.send(Job::EditSummary { text }).await.is_err() {
                    break;
                }
            }
            Command::Cancel => {
                shared.cancel.lock().unwrap().cancel();
                // A dangling question with no turn is a question about nothing.
                shared.broker.abandon_all();
                shared.questions.abandon_all();
            }
            Command::SetMode { mode } => {
                shared.state.lock().unwrap().mode = mode;
                shared.emit_state();
            }
            Command::SetModel { model } => {
                shared.state.lock().unwrap().model = model;
                shared.emit_state();
            }
            Command::SetEffort { effort } => {
                shared.state.lock().unwrap().effort = effort;
                shared.emit_state();
            }

            Command::PreviewCheckpoint { id } => checkpoints::preview(&shared, id).await,
            Command::RestoreCheckpoint { id } => checkpoints::restore(&shared, id).await,

            Command::AnswerPermission { request_id, answer } => {
                // The rule is issued here, not in the worker: even if
                // the waiter has already gone, "allow always" must remain.
                match &answer {
                    PermissionAnswer::AllowAlways { scope } => shared.broker.grant(scope.clone()),
                    PermissionAnswer::DenyAlways { scope, reason } => {
                        shared.broker.refuse(scope.clone(), reason.clone())
                    }
                    PermissionAnswer::Allow | PermissionAnswer::Deny { .. } => {}
                }
                shared.broker.answer(&request_id, answer);
            }

            Command::AnswerQuestion { request_id, answer } => {
                // An answer the question cannot take is refused here, not
                // delivered: the tool would otherwise hand the model a choice
                // nobody made. The question stays open for another try.
                let request = shared.questions.meta(&request_id);
                let refusal = request.as_ref().and_then(|r| r.refuses(&answer));
                match refusal {
                    Some(why) => shared.emit(AgentEvent::Error {
                        code: "invalid_answer".into(),
                        message: format!("Answer rejected: {why}."),
                        retryable: false,
                    }),
                    None => {
                        // Radio: a written answer replaces the pick, here, so the
                        // model, the feed and the journal all hear the same one.
                        let answer = match &request {
                            Some(request) => request.settle(answer),
                            None => answer,
                        };
                        shared.questions.answer(&request_id, answer);
                    }
                }
            }

            Command::SetBypass { on } => {
                shared.state.lock().unwrap().bypass = on;
                shared.emit_state();
            }

            // Both return at once: connecting happens in a task of its own.
            Command::ReconnectMcpServer { server } => shared.mcp.reconnect(&server),
            Command::RestartLspServer { server } => shared.lsp.restart(&server),
            Command::LoginMcpServer { server } => shared.mcp.login(&server),
            Command::LogoutMcpServer { server } => shared.mcp.logout(&server),
            Command::AnswerElicitation { request_id, reply } => {
                if let Err(why) = shared.mcp.answer_elicitation(&request_id, reply) {
                    shared.emit(AgentEvent::Error {
                        code: "invalid_answer".into(),
                        message: format!("Answer rejected: {why}."),
                        retryable: false,
                    });
                }
            }
            Command::ApproveMcpServer { server } => shared.mcp.approve(&server),

            Command::WatchProcess { id } => shared.processes.watch(id),
            Command::ReadFileView { path } => changeset::read_view(&shared, path).await,
            // Only ever removes an entry that is still waiting; see `queue`.
            Command::CancelQueued { id } => shared.queue.cancel(&id),
            // Spawned inside: the walk must not hold up the dispatcher.
            Command::SearchFiles { query } => mentions::answer(&shared, query),
            Command::StopProcess { id } => {
                // Killing the process tree means running `taskkill`, which is
                // blocking work; the dispatcher has to stay responsive.
                // We do not wait for a reply: the panel updates when the process dies.
                let processes = shared.processes.clone();
                tokio::task::spawn_blocking(move || processes.stop(id));
            }
        }
    }
}

#[cfg(test)]
mod tests;

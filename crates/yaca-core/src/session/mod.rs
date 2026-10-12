//! Сессия: команды внутрь, поток событий наружу.
//!
//! Устроена двумя задачами, и это не украшательство. Диспетчер читает команды
//! и обязан отвечать мгновенно — иначе `Esc` не останавливает агента, пока
//! агент занят. Работник крутит турн и владеет историей единолично, поэтому
//! история никому не отдана под мьютекс.
//!
//! Разрезана по швам самого цикла: здесь устройство сессии и диспетчер, в
//! [`turn`] — ход агента от сообщения до остановки, в [`calls`] — выполнение
//! тулов, в [`asking`] — вопрос о разрешении перед ним, в [`checkpoints`] — снимки рабочего дерева
//! и откат, в [`compaction`] — сжатие истории по ходу работы, в [`inbox`] —
//! очередь работника и сообщения, вливающиеся в идущий ход (§7.4), в
//! [`subagent`] — разговор, который тул `Task` ведёт внутри хода. Сообщения,
//! ещё не влитые в ход, живут в [`queue`]: отмена и влитие — одна и та же
//! запись под одним замком.

pub mod accumulator;
mod asking;
pub mod binding;
mod calls;
mod changeset;
mod checkpoints;
pub mod compact;
mod compaction;
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
use crate::usage::{catalog, ModelInfo, UsageMeter, DEFAULT_MODEL};
use crate::workspace::Workspace;

/// What the worker is asked to do. More than "another message" since §5.11:
/// compaction owns the history, and the history belongs to the worker alone.
pub(crate) enum Job {
    /// A message waiting in [`queue::Queue`]: the job says where to look, and
    /// the queue hands the text over only if it has not been cancelled.
    Message { id: QueuedId },
    /// «Сжать» (§5.11).
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
    /// Модели провайдера, с которым говорит эта сессия (§10).
    ///
    /// Не глобальный каталог: окно контекста и цена берутся отсюда, а у
    /// произвольного провайдера они свои. Модель, которой в списке нет,
    /// считается неизвестной — бесплатной и с консервативным окном.
    pub models: Vec<ModelInfo>,
    pub mode: Mode,
    pub effort: Effort,
    /// Писать журнал в `.yaca/sessions`. CLI обычно не нужно.
    pub journal: bool,
    /// Continue an existing session instead of starting one (§9).
    ///
    /// The conversation comes back from `<id>.messages.jsonl` and the journals
    /// are appended to, so the session keeps its id, its title and its cost.
    /// Needs [`SessionConfig::journal`]: a session with nothing written down
    /// has nothing to come back to.
    pub resume: Option<SessionId>,
    /// Соблюдать ли `Disallow` из `robots.txt` при загрузке страницы (§10).
    ///
    /// Настройка приложения, и потому приезжает сюда, а не читается из
    /// `.yaca/settings.json`: проект не решает, как Yaca ведёт себя в сети.
    /// Умолчание — соблюдать; `yaca-cli` его и получает, ничего не зная о
    /// настройках окна.
    pub respect_robots: bool,
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
            journal: true,
            resume: None,
            respect_robots: true,
            shell: ShellChoice::default(),
            isolate: false,
            additional_roots: Vec::new(),
            mcp_servers: BTreeMap::new(),
            mcp_approvals: None,
            lsp_servers: BTreeMap::new(),
            lsp_autodetect: false,
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
    pub fn resume_session(&mut self, id: SessionId) {
        if let Some(meta) = SessionStore::open(&self.root).meta(&id) {
            self.choose_model(meta.provider, meta.model);
        }
        self.resume = Some(id);
    }
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

    /// Текущее состояние без ожидания события: подписчик, пришедший позже
    /// старта, иначе не узнал бы, в каком режиме сессия.
    pub fn state(&self) -> SessionState {
        self.shared.state.lock().unwrap().clone()
    }

    /// This session's line in the panel (§9). `None` — the session is not
    /// being written down, so it has no line.
    pub fn meta(&self) -> Option<SessionMeta> {
        self.shared.recorder.as_ref().map(|r| r.meta())
    }

    /// The MCP servers as they are now, for the same late subscriber
    /// [`SessionHandle::state`] is for: servers start connecting when the
    /// session does, before anyone listens.
    pub fn mcp(&self) -> Vec<McpServerSummary> {
        self.shared.mcp.snapshot()
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
    /// Токен текущего турна. Заменяется на каждом турне.
    cancel: Mutex<CancellationToken>,
    events: EventSink,
    /// Фоновые команды (§6.5). Тоже общие: «Остановить» обязано работать,
    /// пока агент занят чем-то другим.
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
    /// Кто спрашивает разрешения и помнит выданные правила (§5.7).
    broker: Broker,
    /// Вопросы `AskUser` на экране. Общие с диспетчером, как и разрешения:
    /// ответ приходит командой, а ждёт его тул в работнике. Сам вопрос лежит
    /// рядом с каналом, чтобы ответ проверить до доставки.
    questions: Pending<RequestId, QuestionRequest, QuestionReply>,
    /// Модели провайдера этой сессии — источник окна контекста и цен (§10).
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
    /// Held by a call that may not run beside another (`calls`).
    lane: tokio::sync::Mutex<()>,
    /// Models that turned an image down in this session (`vision`).
    blind: Mutex<std::collections::HashSet<String>>,
    /// Соблюдать ли `robots.txt` (§10). Из [`SessionConfig`], а не из файла
    /// проекта: настройка приложения.
    respect_robots: bool,
    /// The shell of this session's commands, named in its system prompt.
    shell: Shell,
    /// The MCP servers of this session (`mcp`). Shared: reconnecting is a
    /// command from the panel, and a tool call reaches its server through here.
    mcp: Mcp,
    /// The language servers of this session (`lsp`). Started by the first
    /// call that needs one.
    lsp: Lsp,
}

impl Shared {
    /// Что известно о модели: из списка провайдера, а не из глобального
    /// каталога. Незнакомая считается бесплатной и с консервативным окном —
    /// §8 обещает работу с любыми моделями, включая те, о которых Yaca не
    /// слышал.
    fn model_info(&self, id: &str) -> ModelInfo {
        self.models
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .unwrap_or_else(|| crate::usage::unknown_model(id))
    }

    fn emit(&self, event: AgentEvent) {
        if let Some(recorder) = &self.recorder {
            recorder.event(&event);
        }
        self.events.emit(event);
    }

    fn emit_state(&self) {
        let state = self.state.lock().unwrap().clone();
        self.emit(AgentEvent::StateChanged { state });
    }

    /// Матрица доверия §8 в том виде, в каком она сейчас настроена.
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
            "ни одного".to_string()
        } else {
            roots.join(", ")
        }
    };
    Err(CoreError::Config(format!(
        "сессия {id} открыта с дополнительными корнями: {}; заданы: {}. Корни не меняются по ходу сессии",
        list(&recorded),
        list(&given)
    )))
}

/// Поднимает сессию и возвращает ручку к ней.
pub fn spawn(
    config: SessionConfig,
    provider: Arc<dyn Provider>,
) -> Result<SessionHandle, CoreError> {
    // Settings first: the workspace needs the project's snapshot exclusions
    // before it is shared, so that the permission questions see them (§8).
    let root = Workspace::open(&config.root)?;
    let settings = Settings::load(root.root())?;
    let root = root.with_shadow_exclude(&settings.shadow.exclude);
    let store = SessionStore::open(root.home());

    // Resuming keeps the id: §9 shows one session in the panel, not a chain of
    // fragments, and cost and title are already attached to it.
    let (id, history, prefix) = match &config.resume {
        Some(id) => {
            let records = store.records(id);
            let prefix = crate::store::history::prefix(&records);
            (id.clone(), crate::store::history::messages(records), prefix)
        }
        None => (SessionId::new(), Vec::new(), None),
    };

    // After the id, which names the worktree. The tools and the snapshots
    // work in it from here on; the project's folder keeps the journals.
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
    if let Some(resumed) = &config.resume {
        check_resumed_roots(&store, resumed, &ws)?;
    }

    let (event_tx, _) = broadcast::channel(4_096);
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (work_tx, work_rx) = mpsc::channel(64);

    let recorder = if config.journal {
        let mut meta = store
            .meta(&id)
            .unwrap_or_else(|| SessionMeta::new(id.clone(), config.provider.clone(), config.model.clone()));
        meta.additional_roots = ws.additional_text();
        Recorder::open(ws.home(), meta).ok().map(Arc::new)
    } else {
        None
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
        let specs =
            crate::mcp::config::merge(&config.mcp_servers, &project_file, &settings.mcp_servers);
        let mut roots = vec![ws.root().to_path_buf()];
        roots.extend(ws.additional_text().into_iter().map(PathBuf::from));
        Mcp::start(
            specs,
            ws.home(),
            ws.root(),
            roots,
            events.clone(),
            config.mcp_approvals.as_deref(),
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
        checkpoints: Checkpoints::open_in(ws.root(), ws.home(), &settings.shadow.exclude).map_err(|e| {
            CoreError::Config(format!(
                "не открыть теневой репозиторий в {}: {e}",
                ws.root().display()
            ))
        })?,
        broker,
        questions: Pending::default(),
        respect_robots: config.respect_robots,
        shell: Shell::resolve(config.shell),
        mcp,
        lsp,
        models: config.models.clone(),
        ws: ws.clone(),
        isolation: isolation.clone(),
        recorder,
        settings,
        ledger: Arc::default(),
        lane: tokio::sync::Mutex::new(()),
        blind: Mutex::default(),
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

    tokio::spawn(dispatch(cmd_rx, work_tx, shared.clone()));
    tokio::spawn(
        turn::Worker {
            instructions: crate::instructions::load(&ws).map(Arc::new),
            memory: crate::tool::memory::store::load(&ws).map(Arc::new),
            skills: crate::tool::skill::listing(&ws).map(Arc::new),
            ws,
            provider,
            registry: Arc::new(ToolRegistry::standard()),
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

/// Диспетчер. Ничего долгого здесь быть не должно.
async fn dispatch(mut rx: mpsc::Receiver<Command>, work: mpsc::Sender<Job>, shared: Arc<Shared>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::SendMessage { text, attachments } => {
                // Отправленное во время работы встаёт в очередь (§7.4). Чип
                // показываем только когда работник занят: простаивающий
                // возьмёт сообщение сразу, и чип мигнул бы ложью про ожидание.
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
                // Висящий вопрос без турна — вопрос ни о чём.
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
                // Правило выдаётся здесь, а не в работнике: даже если
                // ожидающий уже ушёл, «разрешать всегда» должно остаться.
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
                let refusal = shared
                    .questions
                    .meta(&request_id)
                    .and_then(|request| request.refuses(&answer));
                match refusal {
                    Some(why) => shared.emit(AgentEvent::Error {
                        code: "invalid_answer".into(),
                        message: format!("Ответ не принят: {why}."),
                        retryable: false,
                    }),
                    None => {
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
            Command::ApproveMcpServer { server } => shared.mcp.approve(&server),

            Command::WatchProcess { id } => shared.processes.watch(id),
            Command::ReadFileView { path } => changeset::read_view(&shared, path).await,
            // Only ever removes an entry that is still waiting; see `queue`.
            Command::CancelQueued { id } => shared.queue.cancel(&id),
            // Spawned inside: the walk must not hold up the dispatcher.
            Command::SearchFiles { query } => mentions::answer(&shared, query),
            Command::StopProcess { id } => {
                // Убийство дерева процессов — запуск `taskkill`, то есть
                // блокирующая работа; диспетчер обязан остаться отзывчивым.
                // Ответа не ждём: панель обновится, когда процесс умрёт.
                let processes = shared.processes.clone();
                tokio::task::spawn_blocking(move || processes.stop(id));
            }
        }
    }
}

#[cfg(test)]
mod tests;

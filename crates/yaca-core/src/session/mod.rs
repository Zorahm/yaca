//! Сессия: команды внутрь, поток событий наружу.
//!
//! Устроена двумя задачами, и это не украшательство. Диспетчер читает команды
//! и обязан отвечать мгновенно — иначе `Esc` не останавливает агента, пока
//! агент занят. Работник крутит турн и владеет историей единолично, поэтому
//! история никому не отдана под мьютекс.
//!
//! Разрезана по швам самого цикла: здесь устройство сессии и диспетчер, в
//! [`turn`] — ход агента от сообщения до остановки, в [`calls`] — выполнение
//! тулов вместе с разрешениями, в [`checkpoints`] — снимки рабочего дерева
//! и откат, в [`compaction`] — сжатие истории по ходу работы, в [`inbox`] —
//! очередь работника и сообщения, вливающиеся в идущий ход (§7.4).

pub mod accumulator;
pub mod binding;
mod calls;
mod checkpoints;
pub mod compact;
mod compaction;
mod inbox;
pub mod log;
pub mod prompt;
mod turn;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use crate::checkpoint::Checkpoints;
use crate::command::Command;
use crate::error::CoreError;
use crate::event::{AgentEvent, EventSink, SessionState};
use crate::id::SessionId;
use crate::intent::{Effort, Mode};
use crate::permission::broker::Broker;
use crate::permission::policy::Policy;
use crate::permission::PermissionAnswer;
use crate::process::Processes;
use crate::provider::registry::ANTHROPIC_ID;
use crate::provider::Provider;
use crate::settings::Settings;
use crate::store::{Recorder, SessionMeta, SessionStore};
use crate::tool::ToolRegistry;
use crate::usage::{catalog, ModelInfo, UsageMeter, DEFAULT_MODEL};
use crate::workspace::Workspace;

/// What the worker is asked to do. More than "another message" since §5.11:
/// compaction owns the history, and the history belongs to the worker alone.
pub(crate) enum Job {
    Message {
        text: String,
        attachments: Vec<String>,
    },
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
    /// The shadow repository (§5.8). Shared for the same reason: a rollback is
    /// a command from the panel, and the panel is not waiting for the turn.
    checkpoints: Checkpoints,
    /// Кто спрашивает разрешения и помнит выданные правила (§5.7).
    broker: Broker,
    /// Модели провайдера этой сессии — источник окна контекста и цен (§10).
    models: Vec<ModelInfo>,
    ws: Arc<Workspace>,
    /// The three files of this session (§9). `None` — nothing is written down,
    /// which is what a one-shot CLI run wants.
    recorder: Option<Arc<Recorder>>,
    /// `.yaca/settings.json` as it was when the session opened. Read for what
    /// the core decides on its own — which model summarizes history (§5.11).
    settings: Settings,
    /// Соблюдать ли `robots.txt` (§10). Из [`SessionConfig`], а не из файла
    /// проекта: настройка приложения.
    respect_robots: bool,
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

/// Поднимает сессию и возвращает ручку к ней.
pub fn spawn(
    config: SessionConfig,
    provider: Arc<dyn Provider>,
) -> Result<SessionHandle, CoreError> {
    let ws = Arc::new(Workspace::open(&config.root)?);
    let settings = Settings::load(ws.root())?;
    let store = SessionStore::open(ws.root());

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

    let (event_tx, _) = broadcast::channel(4_096);
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (work_tx, work_rx) = mpsc::channel(64);

    let recorder = if config.journal {
        let meta = store
            .meta(&id)
            .unwrap_or_else(|| SessionMeta::new(id.clone(), config.provider.clone(), config.model.clone()));
        Recorder::open(ws.root(), meta).ok().map(Arc::new)
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
        // Hard, not optional. §5.8 is not a nicety, and a session that cannot
        // snapshot is a session where applying edits and running commands are
        // both irreversible — better said now than discovered later.
        checkpoints: Checkpoints::open(ws.root(), &settings.shadow.exclude).map_err(|e| {
            CoreError::Config(format!(
                "не открыть теневой репозиторий в {}: {e}",
                ws.root().display()
            ))
        })?,
        broker,
        respect_robots: config.respect_robots,
        models: config.models.clone(),
        ws: ws.clone(),
        recorder,
        settings,
        state: Mutex::new(SessionState {
            project_root: ws.root().to_string_lossy().to_string(),
            provider: config.provider.clone(),
            model: config.model.clone(),
            mode: config.mode,
            effort: config.effort,
            busy: false,
            bypass: false,
        }),
        cancel: Mutex::new(CancellationToken::new()),
        events,
    });

    tokio::spawn(dispatch(cmd_rx, work_tx, shared.clone()));
    tokio::spawn(
        turn::Worker {
            instructions: crate::instructions::load(&ws),
            ws,
            provider,
            registry: ToolRegistry::standard(),
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
                // Отправленное во время работы встаёт в очередь (§7.4).
                if work.send(Job::Message { text, attachments }).await.is_err() {
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

            Command::SetBypass { on } => {
                shared.state.lock().unwrap().bypass = on;
                shared.emit_state();
            }

            Command::WatchProcess { id } => shared.processes.watch(id),
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

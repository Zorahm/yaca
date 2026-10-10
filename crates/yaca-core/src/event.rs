//! Поток `AgentEvent` — единственное, что ядро отдаёт наружу (§15).
//!
//! Интерфейс — чистая функция от этого потока, поэтому здесь не должно быть
//! ничего, что нельзя отрисовать, и не должно не хватать ничего, что §5 рисует.
//!
//! Сверх списка §15 добавлены пять событий; каждое закрывает дыру, где §5
//! требует картинку, а §15 не даёт данных:
//!
//! * [`AgentEvent::Error`] и [`AgentEvent::RetryScheduled`] — §5.10 рисует блок
//!   ошибки с кодом и строку `Повтор через 4 с · попытка 2 из 5`.
//! * [`AgentEvent::ToolCallRejected`] — §5.9: сработавший инвариант красится
//!   `--vermilion-ink`, не `--rust`, и это не то же самое, что упавший тул.
//! * [`AgentEvent::TodosUpdated`] — §5.6: список задач обновляется на месте,
//!   из результата `todo_write` это не вывести.
//! * [`AgentEvent::PlanProposed`] — §7.1: выход из `Plan` через `exit_plan_mode`
//!   показывает план и ждёт, пока человек выберет режим.
//!
//! У [`AgentEvent::ToolCallDenied`] появилось поле `stopped`: §5.5
//! различает отказ в разрешении и остановку работающего вызова, а §15 даёт на
//! оба один исход.
//!
//! Со срезом команд добавились три события панели процессов (§6.5), которой в
//! §15 нет вовсе:
//!
//! * [`AgentEvent::ProcessesUpdated`] — список фоновых команд целиком: панель
//!   есть функция от последнего такого события, а не сумма предыдущих.
//! * [`AgentEvent::ProcessOutputReset`] — «выбор показывает вывод»: весь
//!   кольцевой буфер разом, когда человек выбрал строку.
//! * [`AgentEvent::ProcessOutput`] — живой хвост выбранной строки.
//!
//! The checkpoint slice adds three more on top of §15's `CheckpointCreated`:
//!
//! * [`AgentEvent::CheckpointsUpdated`] — the whole list of points. §1.1 gives
//!   checkpoints a panel, and a panel assembled from twenty separate
//!   `CheckpointCreated` events is a second copy of the truth.
//! * [`AgentEvent::CheckpointPreviewed`] — §5.8 requires a rollback to show the
//!   files it will touch and be confirmed against them. That list exists only
//!   in the core, and §15 has no event carrying it.
//! * [`AgentEvent::CheckpointRestored`] — the `Откачено к 14:32 · 3 файла` line
//!   of §5.5. `CheckpointCreated` marks the point; nothing in §15 marks the
//!   return to it.
//!
//! The storage slice adds one more beside §15's `ContextCompacted`:
//!
//! * [`AgentEvent::SummaryEdited`] — §5.11 lets the human rewrite the summary
//!   before continuing, and a session reopened from its journal has to show the
//!   text they left, not the one the model first wrote.
//!
//! Subagents add no event, and one field: `parent` on
//! [`AgentEvent::ToolCallStarted`]. A subagent's calls are rows like any
//! other — the same diff, the same question to the human — and the field says
//! which `Task` row they are drawn under. §15 has no place for it because the
//! spec leaves "where do subagents go" as an open question.
//!
//! And the interface slice adds [`AgentEvent::UserMessage`]. §5.1 draws what
//! the human said, and §9 reopens a session by replaying this journal: without
//! it a reopened session is the agent talking to itself. The text is written
//! down twice — here for the feed, and in the message log for the model — but
//! the two carry different things, and the feed журнал is the one that has to
//! be replayable on its own.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::change::{FileDiff, FileFact};
use crate::checkpoint::{CheckpointChange, CheckpointSummary};
use crate::id::{BlockId, CallId, CheckpointId, RequestId, TurnId};
use crate::intent::{Effort, Mode};
use crate::permission::PermissionRequest;
use crate::process::{ProcessId, ProcessSummary};
use crate::usage::UsageSnapshot;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Карточка сообщения человека (§5.1).
    ///
    /// Приходит от ядра, а не рисуется интерфейсом по факту отправки: иначе
    /// открытая заново сессия показывала бы ответы без вопросов.
    #[serde(rename_all = "camelCase")]
    UserMessage {
        turn_id: TurnId,
        text: String,
        /// Файлы, приложенные через `@` (§7.2) — чипы над текстом.
        attachments: Vec<String>,
    },

    /// Кольцо состояния вращается, кнопка отправки → `■`.
    #[serde(rename_all = "camelCase")]
    TurnStarted { turn_id: TurnId, at: TsMillis },

    /// Дописывает в текущий текстовый блок.
    #[serde(rename_all = "camelCase")]
    TextDelta {
        turn_id: TurnId,
        block_id: BlockId,
        text: String,
    },

    /// Дописывает в свёрнутый блок рассуждения (§5.3).
    #[serde(rename_all = "camelCase")]
    ThinkingDelta {
        turn_id: TurnId,
        block_id: BlockId,
        text: String,
    },

    /// Строка в группе действий с точкой `●`, подпись в настоящем времени.
    #[serde(rename_all = "camelCase")]
    ToolCallStarted {
        turn_id: TurnId,
        call_id: CallId,
        /// Имя тула. В ленте не показывается (§16.6), нужно для иконки и отладки.
        tool: String,
        /// Подпись, которую написала модель (§5.4.1). `None` — тул из тех,
        /// что подписывает интерфейс, либо модель не заполнила поле.
        description: Option<String>,
        /// Первая строка раскрытой строки (§5.5.1): полный путь или полная команда.
        target: Option<String>,
        /// Аргументы вызова. Нужны для §5.7 (подробность запроса разрешения).
        #[ts(type = "unknown")]
        input: serde_json::Value,
        /// The `Task` row this call belongs under: it was made by a subagent,
        /// not by the agent. `None` for the agent's own calls, and absent
        /// from journals written before subagents existed.
        #[serde(default)]
        parent: Option<CallId>,
    },

    /// Дописывает вывод в раскрытую строку.
    #[serde(rename_all = "camelCase")]
    ToolCallProgress { call_id: CallId, chunk: String },

    /// Подпись в прошедшем времени, счётчики, шапка-итог пересобирается.
    #[serde(rename_all = "camelCase")]
    ToolCallFinished { call_id: CallId, meta: ToolMeta },

    /// Форма неудачи вместо описания модели, глагол цветом `--rust`.
    #[serde(rename_all = "camelCase")]
    ToolCallFailed { call_id: CallId, message: String },

    /// Инвариант отказал (§5.9). Глагол цветом `--vermilion-ink`: защита сработала,
    /// человеку делать нечего.
    #[serde(rename_all = "camelCase")]
    ToolCallRejected { call_id: CallId, reason: String },

    /// `Отклонено вами` (§5.5). Третий род исхода: не сбой тула и не
    /// сработавший инвариант, а решение человека.
    #[serde(rename_all = "camelCase")]
    ToolCallDenied {
        call_id: CallId,
        reason: Option<String>,
        /// Вызов уже работал, и человек его остановил, — а не отказал в
        /// разрешении до запуска. Род исхода тот же, решение человека, но
        /// предложение другое: «Остановлено вами» против «Отклонено вами».
        /// Поля в §15 нет; лента без него написала бы про остановленную
        /// команду, что человек её отклонил, чего не было.
        stopped: bool,
    },

    /// Инлайновый блок запроса разрешения (§5.7). Кольцо пульсирует,
    /// окно поднимается, при свёрнутом окне уходит уведомление.
    #[serde(rename_all = "camelCase")]
    PermissionRequested { request: PermissionRequest },

    /// Блок можно убирать: ответ получен либо турн отменён.
    #[serde(rename_all = "camelCase")]
    PermissionResolved { request_id: RequestId },

    /// Список задач обновляется на месте, новый блок не создаётся (§5.6).
    #[serde(rename_all = "camelCase")]
    TodosUpdated { items: Vec<TodoItem> },

    /// `exit_plan_mode`: показать план и ждать, пока человек выберет режим (§7.1).
    #[serde(rename_all = "camelCase")]
    PlanProposed { plan: String },

    /// Файл изменён в рабочем дереве: строка действия получает счётчики, а
    /// раскрытие — дифф (§5.5.1).
    #[serde(rename_all = "camelCase")]
    FileChanged {
        call_id: CallId,
        fact: FileFact,
        diff: FileDiff,
    },

    /// Панель процессов целиком (§6.5).
    ///
    /// Панель — функция от последнего такого события, а не сумма предыдущих:
    /// состояние живёт в ядре, которое одно знает, жив ли ещё процесс.
    #[serde(rename_all = "camelCase")]
    ProcessesUpdated { processes: Vec<ProcessSummary> },

    /// Человек выбрал строку в панели — показать её вывод целиком (§6.5).
    #[serde(rename_all = "camelCase")]
    ProcessOutputReset {
        id: ProcessId,
        text: String,
        /// Сколько строк вытеснено из буфера. Панель говорит об этом прямо:
        /// молча потерянный вывод неотличим от вывода, которого не было.
        #[ts(type = "number")]
        dropped: u64,
    },

    /// Живой хвост выбранного процесса.
    ///
    /// Приходит только для выбранной строки. Вывод остальных лежит в буфере:
    /// болтливый `npm run dev`, на который никто не смотрит, вытеснил бы
    /// настоящие события ленты из широковещательного канала, а отставший
    /// подписчик их теряет.
    #[serde(rename_all = "camelCase")]
    ProcessOutput { id: ProcessId, chunk: String },

    /// A divider line in the feed (§5.8): `── ● 14:32 · перед применением…`.
    ///
    /// Only sent when the point is new. A snapshot of a tree that has not moved
    /// since the previous checkpoint is not one, and a divider offering to roll
    /// back to the state the feed is already in would be an interface lying
    /// about state (§0).
    #[serde(rename_all = "camelCase")]
    CheckpointCreated { checkpoint: CheckpointSummary },

    /// The checkpoint panel in full (§1.1).
    ///
    /// Like the change bar, the panel is a function of the last such event and
    /// not the sum of the previous ones: the list is where a rollback adds a
    /// point of its own, and a list kept in the interface would miss it.
    #[serde(rename_all = "camelCase")]
    CheckpointsUpdated { checkpoints: Vec<CheckpointSummary> },

    /// The files a rollback would touch (§5.8). Shown for confirmation; nothing
    /// has happened on disk yet.
    #[serde(rename_all = "camelCase")]
    CheckpointPreviewed {
        id: CheckpointId,
        files: Vec<CheckpointChange>,
    },

    /// `Откачено к 14:32 · 3 файла` (§5.5). `at` is the time of the point that
    /// was returned to, which is the time the line names.
    #[serde(rename_all = "camelCase")]
    CheckpointRestored {
        id: CheckpointId,
        at: TsMillis,
        files: u32,
    },

    /// Разделитель `история сжата · освобождено 84k токенов ▾` (§5.11).
    ///
    /// `freed` is an estimate, and the field is one number rather than two
    /// because §5.11 shows one. The exact figure belongs to the provider and
    /// arrives a response later, by which time the divider has been on screen
    /// for a minute.
    #[serde(rename_all = "camelCase")]
    ContextCompacted { summary: String, freed: u32 },

    /// The human rewrote the summary before continuing (§5.11).
    #[serde(rename_all = "camelCase")]
    SummaryEdited { summary: String },

    /// Заполнение кольца состояния.
    #[serde(rename_all = "camelCase")]
    UsageUpdated { usage: UsageSnapshot },

    /// Кольцо гаснет, группа действий сворачивается в итог.
    #[serde(rename_all = "camelCase")]
    TurnFinished {
        turn_id: TurnId,
        stop_reason: StopReason,
    },

    /// Строка `Остановлено`, незавершённые тулы в `отменено`.
    #[serde(rename_all = "camelCase")]
    TurnCancelled { turn_id: TurnId },

    /// Блок ошибки с кодом, «Повторить» и «Сменить модель» (§5.10).
    #[serde(rename_all = "camelCase")]
    Error {
        code: String,
        message: String,
        retryable: bool,
    },

    /// `Повтор через 4 с · попытка 2 из 5` (§5.10).
    #[serde(rename_all = "camelCase")]
    RetryScheduled {
        attempt: u32,
        of: u32,
        delay_ms: u32,
        reason: String,
    },

    /// Отражение `SetMode` и смены модели: нижний ряд композера всегда
    /// показывает актуальный режим (§7.1, §8).
    #[serde(rename_all = "camelCase")]
    StateChanged { state: SessionState },
}

impl AgentEvent {
    /// Тег события — тот же, что уезжает в JSON, и то, по чему интерфейс
    /// различает ветки. Живёт здесь, а не в тестах: список из двадцати с
    /// лишним веток, поддерживаемый в двух местах, расходится на первом же
    /// добавленном событии.
    pub fn kind(&self) -> &'static str {
        match self {
            AgentEvent::UserMessage { .. } => "userMessage",
            AgentEvent::TurnStarted { .. } => "turnStarted",
            AgentEvent::TextDelta { .. } => "textDelta",
            AgentEvent::ThinkingDelta { .. } => "thinkingDelta",
            AgentEvent::ToolCallStarted { .. } => "toolCallStarted",
            AgentEvent::ToolCallProgress { .. } => "toolCallProgress",
            AgentEvent::ToolCallFinished { .. } => "toolCallFinished",
            AgentEvent::ToolCallFailed { .. } => "toolCallFailed",
            AgentEvent::ToolCallRejected { .. } => "toolCallRejected",
            AgentEvent::ToolCallDenied { .. } => "toolCallDenied",
            AgentEvent::PermissionRequested { .. } => "permissionRequested",
            AgentEvent::PermissionResolved { .. } => "permissionResolved",
            AgentEvent::TodosUpdated { .. } => "todosUpdated",
            AgentEvent::PlanProposed { .. } => "planProposed",
            AgentEvent::FileChanged { .. } => "fileChanged",
            AgentEvent::ProcessesUpdated { .. } => "processesUpdated",
            AgentEvent::ProcessOutputReset { .. } => "processOutputReset",
            AgentEvent::ProcessOutput { .. } => "processOutput",
            AgentEvent::CheckpointCreated { .. } => "checkpointCreated",
            AgentEvent::CheckpointsUpdated { .. } => "checkpointsUpdated",
            AgentEvent::CheckpointPreviewed { .. } => "checkpointPreviewed",
            AgentEvent::CheckpointRestored { .. } => "checkpointRestored",
            AgentEvent::ContextCompacted { .. } => "contextCompacted",
            AgentEvent::SummaryEdited { .. } => "summaryEdited",
            AgentEvent::UsageUpdated { .. } => "usageUpdated",
            AgentEvent::TurnFinished { .. } => "turnFinished",
            AgentEvent::TurnCancelled { .. } => "turnCancelled",
            AgentEvent::Error { .. } => "error",
            AgentEvent::RetryScheduled { .. } => "retryScheduled",
            AgentEvent::StateChanged { .. } => "stateChanged",
        }
    }
}

/// Куда ядро складывает события. Тонкая обёртка поверх широковещательного
/// канала: подписчиков может не быть вовсе — CLI гоняет ядро без них.
#[derive(Clone)]
pub struct EventSink(tokio::sync::broadcast::Sender<AgentEvent>);

impl EventSink {
    pub fn new(tx: tokio::sync::broadcast::Sender<AgentEvent>) -> Self {
        EventSink(tx)
    }

    pub fn emit(&self, event: AgentEvent) {
        let _ = self.0.send(event);
    }
}

/// Миллисекунды эпохи. В Rust `u64`, в TS обычное `number`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TsMillis(#[ts(type = "number")] pub u64);

impl TsMillis {
    pub fn now() -> Self {
        TsMillis(crate::id::now_ms())
    }
}

/// Счётчики завершённого тула. Файловые дельты (`+124 −0`) едут отдельно,
/// в `FileChanged`: их считает дифф, а не тул.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ToolMeta {
    /// Сколько строк вывода ушло в ленту.
    pub lines: u32,
    /// Совпадений — для `grep`, файлов — для `glob`.
    pub items: Option<u32>,
    /// Вывод обрезан, полный доступен по кнопке (§5.5.1).
    pub truncated: bool,
    /// Сколько заняло, миллисекунды. Мета справа в строке.
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TodoStatus {
    /// `○` цветом `--ink-muted`.
    Pending,
    /// `●` цветом `--vermilion-ink` с пульсом.
    InProgress,
    /// `✓` цветом `--moss`, текст `line-through`.
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    /// Модель отказалась по соображениям безопасности.
    Refusal,
    /// Серверный тул поставил турн на паузу; цикл продолжает сам.
    PauseTurn,
    /// Ход оборвался ошибкой. Сверх набора провайдера: кольцо состояния
    /// гаснет по `TurnFinished`, поэтому событие обязано прийти и здесь.
    Failed,
}

/// Снимок настроек сессии для нижнего ряда композера.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub project_root: String,
    #[serde(default)]
    pub provider: String,
    pub model: String,
    pub mode: Mode,
    pub effort: Effort,
    pub busy: bool,
    /// Обход разрешений (§8). Не пятый режим, а флаг поверх текущего: чип
    /// режима и кольцо становятся `--rust`.
    pub bypass: bool,
}

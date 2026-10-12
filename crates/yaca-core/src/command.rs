//! Команды внутрь ядра — обратная сторона §15.
//!
//! Держим только то, что ядро умеет исполнить: объявлять команду, которая
//! молча ничего не делает, — ровно тот случай, когда интерфейс врёт про
//! состояние.
//!
//! Two of these are beyond §15's list.
//!
//! [`Command::PreviewCheckpoint`] — §5.8 requires a rollback to show the files
//! it will touch and be confirmed against them, so there has to be a way to ask
//! what a rollback would do without doing it.
//!
//! [`Command::AnswerQuestion`] — the answer to an `AskUser` question. §15 has
//! no command for it because §5.7 has no question beyond permissions; this is
//! the same road the permission answer takes, with its own reply shape.
//!
//! [`Command::EditSummary`] — §5.11 lets the human rewrite the summary before
//! the conversation continues, and what they write has to reach the history the
//! model is given.
//!
//! The file panel adds one more, for §6.4: [`Command::ReadFileView`] (the whole
//! file, for the panel). §15 has none: it draws the diff of a call. There is no
//! command that decides a hunk — the one way back is a checkpoint.
//!
//! Two more came with the queue and the mention search, both beyond §15's list:
//! [`Command::CancelQueued`] (§7.4, the chip's `отменить`) and
//! [`Command::SearchFiles`] (§7.2, the `@` menu). Their answers are
//! `QueueUpdated` and `FilesFound`, which are in the header of `event.rs`.
//!
//! MCP adds two, both beyond §15's list: [`Command::ReconnectMcpServer`] and
//! [`Command::ApproveMcpServer`]. Their answer is `McpUpdated`.
//!
//! What is deliberately *not* here is the session panel of §9: listing,
//! renaming, duplicating, exporting and deleting sessions. This channel talks
//! to the session that is running, and last week's work is not something the
//! current turn has an opinion about. Those live on
//! [`SessionStore`](crate::store::SessionStore).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CheckpointId, QueuedId, RequestId};
use crate::intent::{Effort, Mode};
use crate::permission::PermissionAnswer;
use crate::process::ProcessId;
use crate::question::QuestionReply;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    #[serde(rename_all = "camelCase")]
    SendMessage {
        text: String,
        /// Файлы, приложенные через `@` (§7.2). Регистрируются как прочитанные:
        /// человек сам их приложил, заставлять агента перечитывать — глупость.
        #[serde(default)]
        attachments: Vec<String>,
    },

    /// `Esc`. Незавершённые тулы получают `отменено`.
    Cancel,

    /// Переключатель режима в нижнем ряду композера (§7.1, §8).
    #[serde(rename_all = "camelCase")]
    SetMode { mode: Mode },

    #[serde(rename_all = "camelCase")]
    SetModel { model: String },

    #[serde(rename_all = "camelCase")]
    SetEffort { effort: Effort },

    /// What rolling back to this point would touch (§5.8). Answers with
    /// `CheckpointPreviewed`; nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    PreviewCheckpoint { id: CheckpointId },

    /// `Откатить сюда` (§5.8), once the human has confirmed the list.
    #[serde(rename_all = "camelCase")]
    RestoreCheckpoint { id: CheckpointId },

    /// «Сжать» (§5.11, §6.1.1). The core also does this on its own once the
    /// context is nearly full; the button is for doing it sooner.
    Compact,

    /// §5.11: the summary can be rewritten before the conversation continues.
    #[serde(rename_all = "camelCase")]
    EditSummary { text: String },

    /// Ответ на активный запрос разрешения (§5.7).
    #[serde(rename_all = "camelCase")]
    AnswerPermission {
        request_id: RequestId,
        answer: PermissionAnswer,
    },

    /// Ответ на вопрос `AskUser` с вариантами. Не §15: §5.7 даёт ответ только
    /// на разрешение. Отказ от ответа — тот же вариант `Dismissed`, а не
    /// отдельная команда, чтобы у вопроса был один путь закрытия.
    #[serde(rename_all = "camelCase")]
    AnswerQuestion {
        request_id: RequestId,
        answer: QuestionReply,
    },

    /// Обход разрешений. Включение подтверждает человек, флаг не
    /// переживает перезапуск и сбрасывается при смене проекта (§8).
    #[serde(rename_all = "camelCase")]
    SetBypass { on: bool },

    /// Выбор строки в панели процессов (§6.5). `None` — выбор снят.
    ///
    /// Вывод течёт наружу только у выбранной строки. Болтливый `npm run dev`,
    /// на который никто не смотрит, вытеснил бы настоящие события ленты из
    /// широковещательного канала, а отставший подписчик их теряет.
    #[serde(rename_all = "camelCase")]
    WatchProcess { id: Option<ProcessId> },

    /// «Остановить» в панели процессов (§6.5). Бьёт по всему дереву, которое
    /// команда за собой развела.
    #[serde(rename_all = "camelCase")]
    StopProcess { id: ProcessId },

    /// The whole file for the panel, with its unchanged lines (§6.4). Answers
    /// with `FileViewLoaded`; nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    ReadFileView { path: String },

    /// Take a message out of the queue before the turn takes it in (§7.4). A
    /// message the turn has already taken in, or one that was never queued, is
    /// left alone: the queue as the core reports it is the answer either way.
    #[serde(rename_all = "camelCase")]
    CancelQueued { id: QueuedId },

    /// `@` (§7.2): the project's files matching `query`. Answers with
    /// `FilesFound`; the walk runs off the dispatcher and nothing on disk moves.
    #[serde(rename_all = "camelCase")]
    SearchFiles { query: String },

    /// Drop the connection to an MCP server, if there is one, and connect
    /// again: the way back for a server that failed or went away. A name the
    /// session does not have is ignored.
    #[serde(rename_all = "camelCase")]
    ReconnectMcpServer { server: String },

    /// The human's "yes" to a server declared in the project's `.mcp.json`,
    /// which is not started until they give it (`mcp::config`). Remembered for
    /// the project until the declaration changes.
    #[serde(rename_all = "camelCase")]
    ApproveMcpServer { server: String },
}

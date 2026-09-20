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
//! [`Command::EditSummary`] — §5.11 lets the human rewrite the summary before
//! the conversation continues, and what they write has to reach the history the
//! model is given.
//!
//! What is deliberately *not* here is the session panel of §9: listing,
//! renaming, duplicating, exporting and deleting sessions. This channel talks
//! to the session that is running, and last week's work is not something the
//! current turn has an opinion about. Those live on
//! [`SessionStore`](crate::store::SessionStore).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CheckpointId, RequestId};
use crate::intent::{Effort, Mode};
use crate::permission::PermissionAnswer;
use crate::process::ProcessId;

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
}

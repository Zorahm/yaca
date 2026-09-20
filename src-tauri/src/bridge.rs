//! Мост между потоком ядра и вебвью.
//!
//! §12 требует применять текстовые дельты пакетами по кадру, а не по событию.
//! Причина не в отрисовке, а в IPC: каждый переход в вебвью стоит денег,
//! а модель выдаёт десятки дельт в секунду.
//!
//! Мост на каждый открытый чат, а канал по-прежнему один: пачка едет с
//! идентификатором сессии, и окно раскладывает её по лентам само. Канал на
//! чат означал бы подписку и отписку на каждое открытие — лишний повод
//! потерять события того, кто работает в фоне.

use std::time::Duration;

use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

/// Кадр при 60 fps.
const FRAME: Duration = Duration::from_millis(16);

/// Пачка событий одного чата (§12).
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventFrame {
    pub session: SessionId,
    pub events: Vec<AgentEvent>,
}

pub async fn pump(
    app: AppHandle,
    mut events: Receiver<AgentEvent>,
    stop: CancellationToken,
    channel: &'static str,
    session: SessionId,
) {
    let mut batch: Vec<AgentEvent> = Vec::new();
    let mut ticker = tokio::time::interval(FRAME);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = stop.cancelled() => break,

            _ = ticker.tick() => {
                if !batch.is_empty() {
                    let events = coalesce(std::mem::take(&mut batch));
                    let _ = app.emit(channel, EventFrame { session: session.clone(), events });
                }
            }

            received = events.recv() => match received {
                Ok(event) => batch.push(event),
                // Ядро обогнало вебвью. Пропуск дельт заметен, но лучше, чем
                // отставание, которое уже не наверстать.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
        }
    }

    if !batch.is_empty() {
        let events = coalesce(batch);
        let _ = app.emit(channel, EventFrame { session, events });
    }
}

/// Склеивает подряд идущие дельты одного блока. Всё остальное проходит как есть:
/// порядок событий — это и есть контракт, и перетасовывать его нельзя.
fn coalesce(batch: Vec<AgentEvent>) -> Vec<AgentEvent> {
    let mut out: Vec<AgentEvent> = Vec::with_capacity(batch.len());

    for event in batch {
        let merged = match (out.last_mut(), &event) {
            (
                Some(AgentEvent::TextDelta {
                    block_id: prev,
                    text: acc,
                    ..
                }),
                AgentEvent::TextDelta { block_id, text, .. },
            ) if prev == block_id => {
                acc.push_str(text);
                true
            }
            (
                Some(AgentEvent::ThinkingDelta {
                    block_id: prev,
                    text: acc,
                    ..
                }),
                AgentEvent::ThinkingDelta { block_id, text, .. },
            ) if prev == block_id => {
                acc.push_str(text);
                true
            }
            (
                Some(AgentEvent::ToolCallProgress {
                    call_id: prev,
                    chunk: acc,
                }),
                AgentEvent::ToolCallProgress { call_id, chunk },
            ) if prev == call_id => {
                acc.push_str(chunk);
                true
            }
            _ => false,
        };

        if !merged {
            out.push(event);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaca_core::event::{StopReason, TsMillis};

    fn delta(block: &str, text: &str) -> AgentEvent {
        AgentEvent::TextDelta {
            turn_id: "t".into(),
            block_id: block.into(),
            text: text.into(),
        }
    }

    #[test]
    fn дельты_одного_блока_склеиваются() {
        let out = coalesce(vec![delta("b1", "при"), delta("b1", "вет")]);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], AgentEvent::TextDelta { text, .. } if text == "привет"));
    }

    #[test]
    fn разные_блоки_не_смешиваются() {
        let out = coalesce(vec![delta("b1", "раз"), delta("b2", "два")]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn порядок_остальных_событий_сохраняется() {
        let out = coalesce(vec![
            AgentEvent::TurnStarted {
                turn_id: "t".into(),
                at: TsMillis(1),
            },
            delta("b1", "а"),
            delta("b1", "б"),
            AgentEvent::TurnFinished {
                turn_id: "t".into(),
                stop_reason: StopReason::EndTurn,
            },
        ]);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], AgentEvent::TurnStarted { .. }));
        assert!(matches!(out[2], AgentEvent::TurnFinished { .. }));
    }

    #[test]
    fn дельта_после_чужого_события_не_прилипает_к_прошлой() {
        let out = coalesce(vec![
            delta("b1", "а"),
            AgentEvent::UsageUpdated {
                usage: Default::default(),
            },
            delta("b1", "б"),
        ]);
        assert_eq!(out.len(), 3);
    }
}

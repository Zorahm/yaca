//! Bridge between the core's event stream and the webview.
//!
//! §12 requires applying text deltas in batches per frame, not per event.
//! The reason is not rendering but IPC: every crossing into the webview costs money,
//! and the model emits dozens of deltas per second.
//!
//! There is one bridge per open chat, but the channel is still one: a batch travels with
//! the session identifier, and the window sorts it into the feeds itself. A channel per
//! chat would mean subscribing and unsubscribing on every open — an extra chance to
//! lose the events of the one working in the background.
//!
//! A bridge also has an epoch. The window can lose its feed while the session runs on (a
//! reload) and then attaches again, which puts a new bridge in place of the old one. A
//! frame of the old one can still be on its way to the window when the new one's snapshot
//! arrives, and folding it would draw its events twice; the epoch is what tells them apart.

use std::time::Duration;

use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;

/// Frame at 60 fps.
const FRAME: Duration = Duration::from_millis(16);

/// Batch of events of one chat (§12).
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EventFrame {
    pub session: SessionId,
    /// Which bridge of the chat sent this. The window folds only the frames of the epoch
    /// its feed was built at.
    pub epoch: u32,
    pub events: Vec<AgentEvent>,
}

pub async fn pump(
    app: AppHandle,
    mut events: Receiver<AgentEvent>,
    stop: CancellationToken,
    channel: &'static str,
    session: SessionId,
    epoch: u32,
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
                    let _ = app.emit(channel, EventFrame { session: session.clone(), epoch, events });
                }
            }

            received = events.recv() => match received {
                Ok(event) => batch.push(event),
                // The core outran the webview. Skipping deltas is noticeable, but it is better than
                // a lag that can no longer be made up.
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            },
        }
    }

    if !batch.is_empty() {
        let events = coalesce(batch);
        let _ = app.emit(channel, EventFrame { session, epoch, events });
    }
}

/// Merges consecutive deltas of one block. Everything else passes through as is:
/// the order of events is the contract itself, and it must not be shuffled.
pub(crate) fn coalesce(batch: Vec<AgentEvent>) -> Vec<AgentEvent> {
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
    fn deltas_of_one_block_coalesce() {
        let out = coalesce(vec![delta("b1", "hel"), delta("b1", "lo")]);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], AgentEvent::TextDelta { text, .. } if text == "hello"));
    }

    #[test]
    fn different_blocks_do_not_mix() {
        let out = coalesce(vec![delta("b1", "one"), delta("b2", "two")]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn order_of_other_events_is_kept() {
        let out = coalesce(vec![
            AgentEvent::TurnStarted {
                turn_id: "t".into(),
                at: TsMillis(1),
            },
            delta("b1", "a"),
            delta("b1", "b"),
            AgentEvent::TurnFinished {
                turn_id: "t".into(),
                stop_reason: StopReason::EndTurn,
                at: TsMillis(2),
            },
        ]);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], AgentEvent::TurnStarted { .. }));
        assert!(matches!(out[2], AgentEvent::TurnFinished { .. }));
    }

    #[test]
    fn delta_after_another_event_does_not_join_the_earlier_one() {
        let out = coalesce(vec![
            delta("b1", "a"),
            AgentEvent::UsageUpdated {
                usage: Default::default(),
            },
            delta("b1", "b"),
        ]);
        assert_eq!(out.len(), 3);
    }
}

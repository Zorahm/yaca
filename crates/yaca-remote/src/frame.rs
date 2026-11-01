//! Batching the event stream into frames (§12).
//!
//! A client that draws the feed must not be woken per event: the model emits dozens of
//! deltas a second, and every wake-up costs more than the text it carries. So the
//! events of one chat are collected and sent once per tick of a clock, with consecutive
//! deltas of one block merged into one.
//!
//! The desktop window and the remote server send a chat this way, and both use this
//! module, so a phone sees exactly the stream the window sees.

use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;

/// What to do when the receiver lagged behind the sender and events were lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lag {
    /// Go on with what comes next. The desktop takes this: a lost delta is a glitch on
    /// screen, while a lag that never catches up is worse.
    Skip,
    /// Stop without flushing: the stream now has a hole, and the remote client will
    /// attach again for a whole feed.
    Stop,
}

/// Why [`pump`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpEnd {
    /// The stop token was cancelled. The pending batch was flushed.
    Cancelled,
    /// Every sender is gone. The pending batch was flushed.
    Closed,
    /// Events were lost and the policy was [`Lag::Stop`]. Nothing was flushed.
    Lagged,
}

/// Collects events from `events` and hands them to `emit` in batches, one per `period`.
///
/// `emit` is never called with an empty batch. On cancel or when the channel closes,
/// the events still pending are flushed before the function returns, so a chat that
/// stops does not lose its last deltas.
pub async fn pump<F>(
    mut events: Receiver<AgentEvent>,
    stop: CancellationToken,
    period: Duration,
    lag: Lag,
    mut emit: F,
) -> PumpEnd
where
    F: FnMut(Vec<AgentEvent>) + Send,
{
    let mut batch: Vec<AgentEvent> = Vec::new();
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let end = loop {
        tokio::select! {
            _ = stop.cancelled() => break PumpEnd::Cancelled,

            _ = ticker.tick() => {
                if !batch.is_empty() {
                    emit(coalesce(std::mem::take(&mut batch)));
                }
            }

            received = events.recv() => match received {
                Ok(event) => batch.push(event),
                Err(RecvError::Lagged(_)) => match lag {
                    Lag::Skip => continue,
                    // Flushing here would hand out the events around the hole as if they
                    // were the whole story; the caller decides what a hole means.
                    Lag::Stop => return PumpEnd::Lagged,
                },
                Err(RecvError::Closed) => break PumpEnd::Closed,
            },
        }
    };

    if !batch.is_empty() {
        emit(coalesce(batch));
    }
    end
}

/// Merges consecutive deltas of one block. Everything else passes through as is:
/// the order of events is the contract itself, and it must not be shuffled.
pub fn coalesce(batch: Vec<AgentEvent>) -> Vec<AgentEvent> {
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
    use tokio::sync::{broadcast, mpsc};
    use tokio::time::timeout;
    use yaca_core::event::{StopReason, TsMillis};

    /// Used where the test must control every flush itself. The first tick of an
    /// interval fires at once and the rest never come within the test, so the only
    /// emission is the one the pump makes on its own exit.
    const LONG: Duration = Duration::from_secs(3600);
    /// Short enough that the real clock fires within the test, long enough that the
    /// events of one burst are in the batch before the first tick after them.
    const SHORT: Duration = Duration::from_millis(5);

    fn delta(block: &str, text: &str) -> AgentEvent {
        AgentEvent::TextDelta {
            turn_id: "t".into(),
            block_id: block.into(),
            text: text.into(),
        }
    }

    fn collector() -> (
        impl FnMut(Vec<AgentEvent>) + Send,
        mpsc::UnboundedReceiver<Vec<AgentEvent>>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            move |batch| {
                let _ = tx.send(batch);
            },
            rx,
        )
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<Vec<AgentEvent>>) -> Vec<Vec<AgentEvent>> {
        let mut out = Vec::new();
        while let Ok(batch) = rx.try_recv() {
            out.push(batch);
        }
        assert!(out.iter().all(|batch| !batch.is_empty()));
        out
    }

    fn texts(batches: &[Vec<AgentEvent>]) -> Vec<String> {
        batches
            .iter()
            .flatten()
            .filter_map(|event| match event {
                AgentEvent::TextDelta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
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

    #[tokio::test]
    async fn a_tick_emits_the_batch_coalesced() {
        let (tx, rx) = broadcast::channel(16);
        // All sent before the pump is first polled, so they are one burst for it.
        tx.send(delta("b1", "hel")).unwrap();
        tx.send(delta("b1", "lo")).unwrap();
        let (emit, mut emitted) = collector();
        let stop = CancellationToken::new();
        let task = tokio::spawn(pump(rx, stop.clone(), SHORT, Lag::Stop, emit));

        let batch = timeout(Duration::from_secs(5), emitted.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(batch.len(), 1);
        assert!(matches!(&batch[0], AgentEvent::TextDelta { text, .. } if text == "hello"));

        stop.cancel();
        assert_eq!(task.await.unwrap(), PumpEnd::Cancelled);
    }

    #[tokio::test]
    async fn cancel_flushes_the_pending_batch() {
        let (tx, rx) = broadcast::channel(16);
        let (emit, mut emitted) = collector();
        let stop = CancellationToken::new();
        let task = tokio::spawn(pump(rx, stop.clone(), LONG, Lag::Stop, emit));

        tx.send(delta("b1", "a")).unwrap();
        tx.send(delta("b1", "b")).unwrap();
        // The pump only runs when the test yields; this lets it take both events into
        // its batch before the cancel, which is what the flush has to deliver.
        tokio::time::sleep(Duration::from_millis(20)).await;
        stop.cancel();

        assert_eq!(task.await.unwrap(), PumpEnd::Cancelled);
        assert_eq!(texts(&drain(&mut emitted)), vec!["ab".to_string()]);
    }

    #[tokio::test]
    async fn closed_channel_flushes_and_returns_closed() {
        let (tx, rx) = broadcast::channel(16);
        tx.send(delta("b1", "a")).unwrap();
        tx.send(delta("b1", "b")).unwrap();
        drop(tx);
        let (emit, mut emitted) = collector();

        let end = pump(rx, CancellationToken::new(), LONG, Lag::Stop, emit).await;

        assert_eq!(end, PumpEnd::Closed);
        assert_eq!(texts(&drain(&mut emitted)), vec!["ab".to_string()]);
    }

    #[tokio::test]
    async fn overrun_with_stop_returns_lagged_without_flushing() {
        let (tx, rx) = broadcast::channel(1);
        // Capacity 1: the second send overwrites the first before the pump polls.
        tx.send(delta("b1", "a")).unwrap();
        tx.send(delta("b1", "b")).unwrap();
        let (emit, mut emitted) = collector();

        let end = pump(rx, CancellationToken::new(), LONG, Lag::Stop, emit).await;

        assert_eq!(end, PumpEnd::Lagged);
        assert!(drain(&mut emitted).is_empty());
    }

    #[tokio::test]
    async fn overrun_with_skip_goes_on_with_what_comes_next() {
        let (tx, rx) = broadcast::channel(1);
        tx.send(delta("b1", "lost")).unwrap();
        tx.send(delta("b2", "kept")).unwrap();
        drop(tx);
        let (emit, mut emitted) = collector();

        let end = pump(rx, CancellationToken::new(), LONG, Lag::Skip, emit).await;

        assert_eq!(end, PumpEnd::Closed);
        assert_eq!(texts(&drain(&mut emitted)), vec!["kept".to_string()]);
    }

    #[tokio::test]
    async fn ticks_keep_emitting_while_the_stream_runs() {
        let (tx, rx) = broadcast::channel(16);
        let (emit, mut emitted) = collector();
        let stop = CancellationToken::new();
        let task = tokio::spawn(pump(rx, stop.clone(), SHORT, Lag::Skip, emit));

        tx.send(delta("b1", "first")).unwrap();
        let first = timeout(Duration::from_secs(5), emitted.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(texts(&[first]), vec!["first".to_string()]);

        tx.send(delta("b1", "second")).unwrap();
        let second = timeout(Duration::from_secs(5), emitted.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(texts(&[second]), vec!["second".to_string()]);

        stop.cancel();
        assert_eq!(task.await.unwrap(), PumpEnd::Cancelled);
    }
}

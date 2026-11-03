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
//!
//! The batching itself (the clock, the merge of deltas) lives in `yaca_remote::frame`, which
//! the remote server uses as well; this module only sends each batch to the window.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast::Receiver;
use tokio_util::sync::CancellationToken;
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_remote::frame::Lag;

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
    events: Receiver<AgentEvent>,
    stop: CancellationToken,
    channel: &'static str,
    session: SessionId,
    epoch: u32,
) {
    // The window goes on past a lag. A lost delta is a glitch, and a lag that never catches
    // up is worse; the remote server stops instead, because its client starts over anyway.
    yaca_remote::frame::pump(events, stop, FRAME, Lag::Skip, move |events| {
        let _ = app.emit(
            channel,
            EventFrame {
                session: session.clone(),
                epoch,
                events,
            },
        );
    })
    .await;
}

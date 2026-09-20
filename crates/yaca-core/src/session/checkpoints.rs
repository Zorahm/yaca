//! Checkpoints from the session's side: taking one, and going back to one.
//!
//! Everything here runs on a blocking task. A snapshot walks the project and a
//! rollback rewrites it, and the dispatcher has to stay answerable meanwhile —
//! `Esc` must work while a rollback is running, the same as it must while the
//! change set is being applied.

use std::sync::Arc;

use crate::checkpoint::CheckpointReason;
use crate::event::AgentEvent;
use crate::id::CheckpointId;

use super::Shared;

/// Takes a snapshot and puts a divider in the feed.
///
/// Silent when the tree is byte for byte what the previous point holds: there
/// is nothing there to come back to that is not already there.
///
/// A failed snapshot does not stop what it was guarding. The feed says the
/// snapshot failed and then shows no divider, so what the human sees is what is
/// true — there is no point to roll back to. Refusing to run the command
/// instead would trade a missing safety net for a dead agent.
pub(super) async fn create(shared: &Arc<Shared>, reason: CheckpointReason) {
    let checkpoints = shared.checkpoints.clone();
    let outcome = tokio::task::spawn_blocking(move || checkpoints.create(reason)).await;

    match outcome {
        Ok(Ok(Some(checkpoint))) => {
            shared.emit(AgentEvent::CheckpointCreated { checkpoint });
            emit_list(shared);
        }
        Ok(Ok(None)) => {}
        Ok(Err(err)) => failed(shared, format!("could not take a snapshot: {err}")),
        Err(_) => failed(shared, "the snapshot task did not finish".into()),
    }
}

/// §5.8: the rollback shows the files it will touch and waits to be confirmed.
pub(super) async fn preview(shared: &Arc<Shared>, id: CheckpointId) {
    let checkpoints = shared.checkpoints.clone();
    let asked = id.clone();
    let outcome = tokio::task::spawn_blocking(move || checkpoints.preview(&asked)).await;

    match outcome {
        Ok(Ok(Some(files))) => shared.emit(AgentEvent::CheckpointPreviewed { id, files }),
        // The point is gone, so the panel showing it is stale. Saying nothing
        // would leave a confirmation dialog waiting for an answer that never
        // comes.
        Ok(Ok(None)) => failed(shared, format!("no such checkpoint: {id}")),
        Ok(Err(err)) => failed(shared, format!("could not read the checkpoint: {err}")),
        Err(_) => failed(shared, "the checkpoint task did not finish".into()),
    }
}

pub(super) async fn restore(shared: &Arc<Shared>, id: CheckpointId) {
    let checkpoints = shared.checkpoints.clone();
    let asked = id.clone();
    let outcome = tokio::task::spawn_blocking(move || checkpoints.restore(&asked)).await;

    let restored = match outcome {
        Ok(Ok(Some(restored))) => restored,
        Ok(Ok(None)) => return failed(shared, format!("no such checkpoint: {id}")),
        Ok(Err(err)) => return failed(shared, format!("could not roll back: {err}")),
        Err(_) => return failed(shared, "the rollback task did not finish".into()),
    };

    // Same rule as §11: a file that moved under the agent has to be read again.
    // The agent's history still describes the tree as it was a moment ago, and
    // the freshness invariant is what keeps it from editing on that memory.
    for path in &restored.touched {
        if let Ok(abs) = shared.ws.resolve_new(path) {
            shared.ws.forget(&abs);
        }
    }

    if !restored.failed.is_empty() {
        failed(
            shared,
            format!(
                "the rollback could not reach {} file(s): {}",
                restored.failed.len(),
                restored.failed.join(", ")
            ),
        );
    }

    shared.emit(AgentEvent::CheckpointRestored {
        id,
        at: restored.checkpoint.at,
        files: restored.touched.len() as u32,
    });
    // A rollback records the tree it replaced, so the list has grown.
    emit_list(shared);
}

/// The checkpoint panel in full (§1.1). Sent by the core rather than assembled
/// in the interface, for the same reason the change bar is.
pub(super) fn emit_list(shared: &Arc<Shared>) {
    shared.emit(AgentEvent::CheckpointsUpdated {
        checkpoints: shared.checkpoints.list(),
    });
}

fn failed(shared: &Arc<Shared>, message: String) {
    shared.emit(AgentEvent::Error {
        code: "checkpoint_failed".into(),
        message,
        // Nothing here is fixed by asking again: the store is unwritable, or
        // the project is too large to walk.
        retryable: false,
    });
}

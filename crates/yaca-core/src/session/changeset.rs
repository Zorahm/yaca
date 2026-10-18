//! The session's side of the file panel (§6.4): remembering what each file
//! held before the session's first write, and answering the panel with the
//! whole file.
//!
//! Everything touching the disk runs on a blocking task, the same as the
//! checkpoints do: the dispatcher has to stay answerable while a file is read.

use std::path::PathBuf;
use std::sync::Arc;

use crate::change::view::view;
use crate::change::FileView;
use crate::event::AgentEvent;
use crate::tool::files::read_text;

use super::Shared;

/// The writers whose content the set keeps. `Move` has no content to review —
/// the file's bytes do not change — so it is not tracked, and deleting a
/// directory is not a content change either.
pub(super) fn keeps(tool: &str) -> bool {
    matches!(tool, "Write" | "Edit" | "NotebookEdit" | "Delete")
}

/// A writer's target as it was before the write.
pub(super) struct Before {
    abs: PathBuf,
    path: String,
    /// `None` — the file did not exist.
    text: Option<String>,
}

/// Captures the file a writer is about to change. A file that cannot be read
/// as text is not tracked: the writer would refuse it too.
pub(super) async fn before(shared: &Arc<Shared>, target: String) -> Option<Before> {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || {
        let abs = shared.ws.resolve_new(&target).ok()?;
        let text = read_text(&abs).ok()?;
        Some(Before {
            path: shared.ws.display(&abs),
            abs,
            text,
        })
    })
    .await
    .ok()
    .flatten()
}

/// Records what the writer left on disk.
pub(super) async fn after(shared: &Arc<Shared>, before: Before) {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || match read_text(&before.abs) {
        Ok(now) => {
            shared
                .changes
                .record(before.path, before.abs, before.text, now);
        }
        // The write happened and cannot be read back: the panel would be a lie
        // about what changed, so the feed says so instead of going quiet.
        Err(err) => failed(
            &shared,
            "change_untracked",
            format!("failed to reread {} after edit: {err}", before.path),
        ),
    })
    .await
    .ok();
}

/// The full view of one file for the panel (§6.4).
pub(super) async fn read_view(shared: &Arc<Shared>, path: String) {
    let shared = shared.clone();
    tokio::task::spawn_blocking(move || emit_view(&shared, &path))
        .await
        .ok();
}

fn emit_view(shared: &Shared, path: &str) {
    shared.emit(AgentEvent::FileViewLoaded {
        path: path.to_owned(),
        view: shared.changes.view(path).or_else(|| on_disk(shared, path)),
    });
}

/// The view of a file the set does not know under this name: one the agent
/// only read, or a changed one named another way.
///
/// The path comes from an action row, and a row of `Read` carries what the
/// model wrote — absolute, or relative with a `./`. So it is resolved the way
/// a tool resolves it, bounds included, and the set is asked again by the name
/// it keeps. `None` when there is nothing to show: no such file, a directory,
/// a binary, or a path outside the workspace.
fn on_disk(shared: &Shared, path: &str) -> Option<FileView> {
    let abs = shared.ws.resolve_existing(path).ok()?;
    if let Some(changed) = shared.changes.view(&shared.ws.display(&abs)) {
        return Some(changed);
    }
    let text = read_text(&abs).ok()??;
    Some(view(&text, &text, None))
}

fn failed(shared: &Shared, code: &str, message: String) {
    shared.emit(AgentEvent::Error {
        code: code.into(),
        message,
        // Asking again changes nothing: the file cannot be read back, and the
        // human has to look at it.
        retryable: false,
    });
}

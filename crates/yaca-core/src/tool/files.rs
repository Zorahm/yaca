//! Shared plumbing for file tools: writing to the tree and the invariants (§8).
//!
//! The invariants live here, not in each tool, because "switched off by nothing"
//! is a property of one place, not of four identical checks, one of which
//! someone eventually forgets.

use std::path::{Path, PathBuf};

use crate::change::{ChangeKind, FileDiff, FileFact};
use crate::error::ToolError;
use crate::permission::{invariant, unsaved, PermissionKind, PermissionNeed};
use crate::workspace::{ReadRecord, Workspace};

use super::ToolCtx;

/// Where the search starts: where it was pointed, and without a path in every root,
/// the primary one first. `grep`, `glob` and `@`-search walk the same list.
pub(crate) fn starts(ws: &Workspace, given: Option<PathBuf>) -> Vec<PathBuf> {
    match given {
        Some(path) => vec![path],
        None => ws.roots().into_iter().map(Path::to_path_buf).collect(),
    }
}

/// The question about editing file `abs` (§5.7). One for `Write` and `Edit`.
///
/// An edit in an additional root is marked as unsaved: a rollback will not bring
/// it back, and the human must learn that before it happens.
pub(crate) fn edit_need(ws: &Workspace, tool: &str, raw: &str, abs: PathBuf) -> PermissionNeed {
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw.to_string());
    let mut need = PermissionNeed::new(PermissionKind::EditFile, tool, name)
        .detail(ws.display(&abs));
    if let Some(lost) = unsaved::outside_checkpoint(ws, [abs.as_path()]) {
        need = need.unsaved(lost);
    }
    // Git's configuration is asked about in every mode (§8): the write
    // itself is the step a later `git status` would run. The root that holds
    // the file decides what counts as the project's `.git`.
    match invariant::of_path(ws.root_for(&abs), &abs) {
        Some(found) => need.invariant(found).path(abs),
        None => need.path(abs),
    }
}

/// Tree walk for reading tools: `grep` and `glob` walk the same way.
///
/// `require_git(false)` is not decoration here. By default `ignore` applies
/// `.gitignore` only inside a git repository, and a project without `.git` is
/// an ordinary case. Silently getting a full walk, the tool goes into `target/`
/// and `node_modules/` and reads tens of thousands of files: the line in the feed
/// hangs for minutes, and the human sees a freeze, not a search.
pub(crate) fn walk(start: &Path) -> ignore::Walk {
    ignore::WalkBuilder::new(start)
        .hidden(true)
        .require_git(false)
        .build()
}

/// The target of an edit, past the checks.
pub(crate) struct Target {
    pub abs: PathBuf,
    pub rel: String,
    /// Contents on disk. `None` — the file does not exist, or (only from
    /// [`look_any`]) it is binary and has no text to show.
    pub current: Option<String>,
    /// Whether anything is at `abs`. Unlike `current`, true for a binary file.
    pub exists: bool,
    /// Fingerprint of the disk: by it §8 catches an edit made outside Yaca.
    pub fingerprint: Option<ReadRecord>,
}

/// Reads a file, checking nothing beyond the project boundaries.
pub(crate) fn look(ctx: &ToolCtx, raw: &str) -> Result<Target, ToolError> {
    look_in(&ctx.ws, raw)
}

/// The same over the workspace, without the rest of the context.
///
/// Separate, because this is the only part of reading that goes to disk, and a
/// `ToolCtx` cannot be carried across the task boundary. A tool that wants to read
/// without occupying the executor moves the call into `spawn_blocking` with a copy
/// of the `Arc`.
pub(crate) fn look_in(ws: &Workspace, raw: &str) -> Result<Target, ToolError> {
    let abs = ws.resolve_new(raw)?;
    let current = read_text(&abs)?;
    Ok(Target {
        rel: ws.display(&abs),
        exists: current.is_some(),
        current,
        fingerprint: ws.fingerprint(&abs),
        abs,
    })
}

/// Like [`look`], for tools that act on a file without editing its text:
/// `Delete` and `Move`. A binary file is a legitimate target for them, so it
/// comes back with `exists` set and no `current` instead of a refusal.
pub(crate) fn look_any(ctx: &ToolCtx, raw: &str) -> Result<Target, ToolError> {
    let abs = ctx.ws.resolve_new(raw)?;
    // An unreadable file (binary, a directory, no access) still exists; the
    // operation itself reports the real error if it cannot proceed.
    let current = read_text(&abs).ok().flatten();
    Ok(Target {
        rel: ctx.ws.display(&abs),
        exists: std::fs::symlink_metadata(&abs).is_ok(),
        current,
        fingerprint: ctx.ws.fingerprint(&abs),
        abs,
    })
}

/// Writes the contents to the working tree and returns what the interface
/// will show as a line and a diff.
///
/// The fingerprint is updated here too: a write changes the mtime, and without
/// this the agent could not fix a file twice in a row — the second edit would hit
/// its own first one as "file changed after reading".
pub(crate) fn write(
    ctx: &ToolCtx,
    target: &Target,
    kind: ChangeKind,
    content: Option<&str>,
) -> Result<(FileFact, FileDiff), ToolError> {
    match content {
        Some(text) => {
            if let Some(parent) = target.abs.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ToolError::failed(e.to_string()))?;
            }
            std::fs::write(&target.abs, text).map_err(|e| ToolError::failed(e.to_string()))?;
            ctx.ws.mark_read(&target.abs);
        }
        None => {
            std::fs::remove_file(&target.abs).map_err(|e| ToolError::failed(e.to_string()))?;
            ctx.ws.forget(&target.abs);
        }
    }

    Ok(FileFact::of(
        target.rel.clone(),
        kind,
        target.current.as_deref(),
        content,
    ))
}

/// The same plus the invariants of an edit: the file must have been read and must
/// not have changed since the read (§8).
pub(crate) fn look_editable(ctx: &ToolCtx, raw: &str) -> Result<Target, ToolError> {
    let target = look(ctx, raw)?;
    ensure_editable(ctx, &target)?;
    Ok(target)
}

fn ensure_editable(ctx: &ToolCtx, target: &Target) -> Result<(), ToolError> {
    if !ctx.ws.was_read(&target.abs) {
        // §5.9 format: `Edit rejected · src/app.rs was not read in this session`.
        return Err(ToolError::rejected(format!(
            "{} was not read in this session",
            target.rel
        )));
    }

    if ctx.ws.read_record(&target.abs) != target.fingerprint {
        // Forget the fingerprint: until the file is re-read it may not be edited
        // blind, and a retry runs into the very first check.
        ctx.ws.forget(&target.abs);
        return Err(ToolError::rejected(format!(
            "{} changed since it was read — read it again",
            target.rel
        )));
    }

    Ok(())
}

/// Reads a file from disk. Binary and missing are told apart: the first is a
/// refusal, the second a legitimate `None`.
pub(crate) fn read_text(path: &Path) -> Result<Option<String>, ToolError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ToolError::failed(e.to_string())),
    };
    if bytes.iter().take(8_192).any(|b| *b == 0) {
        return Err(ToolError::failed("binary file, nothing to edit"));
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

//! Deleting a directory with everything in it (§5.5, §5.7).
//!
//! A tool of its own rather than a flag on `Delete`. A model that meant a file
//! and named a directory hears so from `Delete`; it does not lose the directory.
//!
//! Some things are never deleted, in any mode and under bypass too, because
//! "roll back" could not bring them back — §5.8 keeps neither in its snapshots:
//! the project itself, and any directory with a `.git` or `.yaca` inside, which
//! is a repository's history or another project's sessions and snapshots. A
//! link is not deleted either: removing the link and removing what it points
//! to are two different requests, and the model cannot tell which one it made.
//!
//! The rest rolls back with the point taken before the message, the same as a
//! deleted file — except what snapshots do not keep at all: the excluded
//! directories (`node_modules`, `target`…) and files over 8 MB. Deleting that
//! is not something a rollback can undo, so it is asked about in every mode but
//! bypass, Auto included (§8), and the question says what will not come back.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::unsaved;
use crate::permission::{invariant, FileCount, PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

/// How far the question counts before it says "more" (§5.7). The question
/// waits for the count, and the count walks the disk.
const COUNT_FOR_QUESTION: u32 = 10_000;

/// Names whose loss a rollback cannot undo.
const KEPT: [&str; 2] = [".git", crate::project::DIR];

pub struct DeleteDir;

#[async_trait]
impl Tool for DeleteDir {
    fn name(&self) -> &'static str {
        "DeleteDir"
    }

    fn description(&self) -> &'static str {
        "Delete a directory together with everything in it. For a single file, use \
         Delete. The project root, and any directory with a .git or .yaca inside, \
         cannot be deleted: a rollback could not bring them back."
    }

    fn properties(&self) -> Value {
        json!({
            "path": { "type": "string", "description": "Path relative to the project root." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    /// §5.7: the name, the full path and the files inside.
    ///
    /// Nothing is asked about a call the tool will refuse anyway — a file, a
    /// link, the root, a directory holding a repository: it says why itself,
    /// and a question about it would be a question about nothing.
    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let abs = ws.resolve_new(input["path"].as_str()?).ok()?;
        if refusal(ws, &abs).is_some() || !is_directory(&abs) {
            return None;
        }
        let contents = survey(&abs, Some(COUNT_FOR_QUESTION)).ok()?;
        if contents.kept.is_some() {
            return None;
        }
        let mut need =
            PermissionNeed::new(PermissionKind::DeleteDirectory, self.name(), name_of(&abs))
                .detail(ws.display(&abs))
                .path(abs.clone())
                .files(FileCount {
                    files: contents.files,
                    more: contents.more,
                });
        if let Some(unsaved) = unsaved::of_dir(ws, &abs).ok()? {
            need = need.unsaved(unsaved);
        }
        if let Some(found) = invariant::of_path(ws.root_for(&abs), &abs) {
            need = need.invariant(found);
        }
        Some(need)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let abs = ctx.ws.resolve_new(raw)?;
        if let Some(reason) = refusal(&ctx.ws, &abs) {
            return Err(reason);
        }

        let meta = std::fs::symlink_metadata(&abs)
            .map_err(|_| ToolError::failed(format!("not found: {raw}")))?;
        if meta.file_type().is_symlink() {
            return Err(ToolError::failed(format!(
                "{raw} is a symlink, not a directory; deleting a symlink vs what it \
                 points to are different requests"
            )));
        }
        if !meta.is_dir() {
            return Err(ToolError::failed(format!(
                "{raw} is a file; delete it with Delete"
            )));
        }

        let rel = ctx.ws.display(&abs);
        let ws = ctx.ws.clone();
        let cancel = ctx.cancel.clone();
        let dir = abs.clone();
        // The whole walk before anything goes: the question counted only so
        // far, and a repository deep inside must stop the deletion before it
        // starts, not halfway through it.
        let files = tokio::task::spawn_blocking(move || {
            let contents = survey(&dir, None).map_err(|e| ToolError::failed(e.to_string()))?;
            if let Some(kept) = contents.kept {
                return Err(ToolError::rejected(format!(
                    "{} contains {} — snapshots do not store it, and rollback will not restore it",
                    ws.display(&dir),
                    ws.display(&kept)
                )));
            }
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            std::fs::remove_dir_all(&dir).map_err(|e| {
                ToolError::failed(format!(
                    "directory {} was not deleted completely: {e}",
                    ws.display(&dir)
                ))
            })?;
            Ok(contents.files)
        })
        .await
        .map_err(|_| ToolError::failed("directory deletion did not complete"))??;

        // Nothing read in there can be edited any more: it is gone.
        ctx.ws.forget_under(&abs);

        Ok(ToolOutput::new(format!("Deleted directory {rel}, files: {files}\n")).items(files))
    }
}

/// The deletions no mode permits (§8): they are invariants, not questions.
fn refusal(ws: &Workspace, abs: &Path) -> Option<ToolError> {
    // Every root, not only the project's: a root is the boundary the session
    // works inside, and deleting it would delete the boundary with its contents.
    if abs == ws.root() {
        return Some(ToolError::rejected(
            "this is the project root; it cannot be deleted entirely",
        ));
    }
    if ws.is_a_root(abs) {
        return Some(ToolError::rejected(
            "this is an additional workspace root; it cannot be deleted entirely",
        ));
    }
    let name = abs.file_name()?;
    KEPT.iter().any(|kept| name == OsStr::new(kept)).then(|| {
        ToolError::rejected(format!(
            "{} — snapshots do not store it, and rollback will not restore it",
            ws.display(abs)
        ))
    })
}

/// A real directory, not a link to one.
fn is_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// What a directory holds.
struct Contents {
    files: u32,
    /// The count stopped at the cap.
    more: bool,
    /// The first entry a rollback could not bring back.
    kept: Option<PathBuf>,
}

/// Walks `dir` without following links, counting files and looking for what
/// must not be deleted. With a cap the walk stops there, and so does the
/// looking — the full walk before the deletion is what decides.
fn survey(dir: &Path, cap: Option<u32>) -> std::io::Result<Contents> {
    let mut contents = Contents {
        files: 0,
        more: false,
        kept: None,
    };
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let name = entry.file_name();
            if KEPT.iter().any(|kept| name == OsStr::new(kept)) {
                contents.kept = Some(entry.path());
                return Ok(contents);
            }
            // `DirEntry::file_type` does not follow links: a link to a
            // directory is counted as the entry it is, not walked into.
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if cap.is_some_and(|cap| contents.files >= cap) {
                contents.more = true;
                return Ok(contents);
            }
            contents.files += 1;
        }
    }
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::Unsaved;
    use crate::testutil::TempDir;
    use crate::tool::read_file::ReadFile;
    use crate::tool::tests_support::ctx_for;

    #[test]
    fn the_count_stops_at_the_cap_and_says_there_is_more() {
        let dir = TempDir::new();
        for i in 0..5 {
            dir.write(&format!("d/sub/{i}.txt"), "x");
        }
        let all = survey(&dir.path().join("d"), None).unwrap();
        assert_eq!((all.files, all.more), (5, false));

        let capped = survey(&dir.path().join("d"), Some(3)).unwrap();
        assert_eq!((capped.files, capped.more), (3, true));
    }

    #[test]
    fn a_repository_deep_inside_is_found() {
        let dir = TempDir::new();
        dir.write("d/a/b/.git/HEAD", "ref\n");
        let contents = survey(&dir.path().join("d"), None).unwrap();
        assert!(contents.kept.unwrap().ends_with(".git"));
    }

    #[tokio::test]
    async fn the_repository_directory_itself_is_refused() {
        let dir = TempDir::new();
        dir.write(".git/HEAD", "ref\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = DeleteDir
            .run(&ctx, json!({ "path": ".git" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
        assert!(dir.path().join(".git/HEAD").exists());
    }

    #[tokio::test]
    async fn a_file_is_sent_to_delete() {
        let dir = TempDir::new();
        dir.write("a.txt", "x");
        let (ctx, _ws) = ctx_for(&dir);

        let err = DeleteDir
            .run(&ctx, json!({ "path": "a.txt" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Delete"), "{err}");
        assert!(dir.path().join("a.txt").exists());
    }

    #[tokio::test]
    async fn a_missing_directory_is_said_to_be_missing() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = DeleteDir
            .run(&ctx, json!({ "path": "nope" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    fn unsaved_of(dir: &TempDir, path: &str) -> Option<Unsaved> {
        let ws = Workspace::open(dir.path()).unwrap();
        DeleteDir
            .permission(&ws, &json!({ "path": path }))
            .expect("a directory is always a question")
            .unsaved
    }

    #[test]
    fn a_directory_holding_build_output_says_what_will_not_come_back() {
        let dir = TempDir::new();
        dir.write("pkg/src/lib.rs", "fn a() {}\n");
        dir.write("pkg/target/debug/app.rlib", "built\n");

        let unsaved = unsaved_of(&dir, "pkg").expect("target/ is not in any snapshot");
        assert_eq!(unsaved.dirs, vec!["pkg/target".to_string()]);
        assert_eq!(unsaved.dir_count, 1);
        // The file under target/ is not counted: it is lost with its directory,
        // and the question names the directory, not every file in it.
        assert_eq!(unsaved.files, 0);
    }

    #[test]
    fn a_directory_the_snapshot_keeps_in_full_has_nothing_unsaved() {
        let dir = TempDir::new();
        dir.write("docs/a.md", "text\n");
        dir.write("docs/nested/b.md", "text\n");
        assert_eq!(unsaved_of(&dir, "docs"), None);
    }

    #[test]
    fn a_directory_inside_a_skipped_one_is_unsaved_as_a_whole() {
        let dir = TempDir::new();
        dir.write("target/debug/a.rlib", "x\n");
        dir.write("pkg/node_modules/left-pad/index.js", "x\n");

        // The snapshot never looked inside target/, so nothing in it is kept.
        let unsaved = unsaved_of(&dir, "target/debug").expect("inside target/");
        assert_eq!(unsaved.dirs, vec!["target/debug".to_string()]);
        assert_eq!(unsaved.dir_count, 1);

        let unsaved = unsaved_of(&dir, "pkg/node_modules/left-pad").expect("inside node_modules");
        assert_eq!(unsaved.dir_count, 1);
    }

    #[test]
    fn a_file_over_the_size_limit_is_unsaved() {
        let dir = TempDir::new();
        dir.write(
            "big/data.bin",
            &"x".repeat(crate::checkpoint::scan::MAX_FILE_BYTES as usize + 1),
        );
        dir.write("big/small.txt", "x\n");

        let unsaved = unsaved_of(&dir, "big").expect("the big file is not in any snapshot");
        assert_eq!(unsaved.files, 1);
        assert_eq!(unsaved.dir_count, 0);
    }

    #[test]
    fn the_listing_names_five_and_counts_the_rest() {
        use crate::permission::unsaved::LISTED_DIRS;
        let dir = TempDir::new();
        for i in 0..7 {
            dir.write(&format!("ws/p{i}/node_modules/x.js"), "x\n");
        }
        let unsaved = unsaved_of(&dir, "ws").unwrap();
        assert_eq!(unsaved.dirs.len(), LISTED_DIRS);
        assert_eq!(unsaved.dir_count, 7);
    }

    #[tokio::test]
    async fn what_was_read_inside_is_forgotten() {
        let dir = TempDir::new();
        dir.write("d/a.rs", "fn a() {}\n");
        let (ctx, ws) = ctx_for(&dir);
        ReadFile
            .run(&ctx, json!({ "path": "d/a.rs" }))
            .await
            .unwrap();
        let read = ws.resolve_existing("d/a.rs").unwrap();
        assert!(ws.was_read(&read));

        let out = DeleteDir.run(&ctx, json!({ "path": "d" })).await.unwrap();

        assert_eq!(out.meta.items, Some(1));
        // A file recreated there later has to be read before it is edited.
        assert!(!ws.was_read(&read));
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use crate::permission::Invariant;
    use crate::testutil::TempDir;

    #[test]
    fn a_directory_the_project_excludes_is_unsaved_when_settings_say_so() {
        let dir = TempDir::new();
        dir.write(".yaca/settings.json", r#"{ "shadow": { "exclude": ["fixtures"] } }"#);
        dir.write("suite/fixtures/a.json", "{}\n");
        let settings = crate::settings::Settings::load(dir.path()).unwrap();
        let ws = Workspace::open(dir.path())
            .unwrap()
            .with_shadow_exclude(&settings.shadow.exclude);

        let need = DeleteDir
            .permission(&ws, &json!({ "path": "suite" }))
            .unwrap();
        let unsaved = need.unsaved.expect("fixtures/ is not in any snapshot");
        assert_eq!(unsaved.dirs, vec!["suite/fixtures".to_string()]);
    }

    #[test]
    fn a_directory_inside_git_is_git_configuration_and_asks_in_every_mode() {
        let dir = TempDir::new();
        dir.write(".git/hooks/pre-commit", "#!/bin/sh\n");
        let ws = Workspace::open(dir.path()).unwrap();

        // The repository itself is refused outright; its hooks are a question.
        assert!(DeleteDir.permission(&ws, &json!({ "path": ".git" })).is_none());
        let need = DeleteDir
            .permission(&ws, &json!({ "path": ".git/hooks" }))
            .unwrap();
        assert_eq!(need.invariant, Some(Invariant::GitConfig));
    }

    #[test]
    fn an_ordinary_directory_carries_no_invariant() {
        let dir = TempDir::new();
        dir.write("docs/a.md", "text\n");
        let ws = Workspace::open(dir.path()).unwrap();
        let need = DeleteDir.permission(&ws, &json!({ "path": "docs" })).unwrap();
        assert_eq!(need.invariant, None);
        assert_eq!(need.unsaved, None);
    }
}

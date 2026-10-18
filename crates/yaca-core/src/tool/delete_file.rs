//! Deleting and moving a file.
//!
//! Read-before-edit is deliberately not required here. The invariant guards
//! against editing blind — against the model overwriting code it never saw.
//! Deletion overwrites nothing, and the safety comes from §8: both deleting
//! and moving ask for permission everywhere except "Auto".

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::{ChangeKind, FileFact};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{invariant, unsaved, PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

pub struct DeleteFile;

#[async_trait]
impl Tool for DeleteFile {
    fn name(&self) -> &'static str {
        "Delete"
    }

    fn description(&self) -> &'static str {
        "Delete a file."
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

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        let mut need = PermissionNeed::new(PermissionKind::DeleteFile, self.name(), short(&abs))
            .detail(ws.display(&abs))
            .path(abs.clone());
        if let Ok(meta) = std::fs::metadata(&abs) {
            need = need.size_bytes(meta.len());
        }
        // A file the snapshot never kept is lost for good on deletion (§5.8).
        if let Some(lost) = unsaved::of_file(ws, &abs) {
            need = need.unsaved(lost);
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

        let target = files::look_any(ctx, raw)?;
        if target.abs.is_dir() {
            return Err(ToolError::failed(format!(
                "{raw} is a directory; delete it with DeleteDir"
            )));
        }
        if !target.exists {
            return Err(ToolError::failed(format!("not found: {raw}")));
        }

        let rel = target.rel.clone();
        let (fact, diff) = files::write(ctx, &target, ChangeKind::Deleted, None)?;

        Ok(ToolOutput::new(format!("Deleted {rel}\n")).changed(FileChange { fact, diff }))
    }
}

pub struct MoveFile;

#[async_trait]
impl Tool for MoveFile {
    fn name(&self) -> &'static str {
        "Move"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file. The content is not changed."
    }

    fn properties(&self) -> Value {
        json!({
            "from": { "type": "string", "description": "Current path." },
            "to": { "type": "string", "description": "New path." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["from", "to"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        match (input["from"].as_str(), input["to"].as_str()) {
            (Some(from), Some(to)) => Some(format!("{from} → {to}")),
            _ => None,
        }
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let from = ws.resolve_new(input["from"].as_str()?).ok()?;
        let to = ws.resolve_new(input["to"].as_str()?).ok()?;
        let mut need = PermissionNeed::new(PermissionKind::MoveFile, self.name(), short(&from))
            .detail(format!("{} → {}", ws.display(&from), ws.display(&to)));
        // Either end may be git's configuration: moving a file into `.git`
        // writes there as surely as creating it.
        let found = invariant::of_path(ws.root_for(&from), &from)
            .or_else(|| invariant::of_path(ws.root_for(&to), &to));
        if let Some(found) = found {
            need = need.invariant(found);
        }
        // Either end in an additional root: that end is not in any snapshot.
        if let Some(lost) = unsaved::outside_checkpoint(ws, [from.as_path(), to.as_path()]) {
            need = need.unsaved(lost);
        }
        Some(need.path(from))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_from = input["from"]
            .as_str()
            .ok_or_else(|| ToolError::failed("from is required"))?;
        let raw_to = input["to"]
            .as_str()
            .ok_or_else(|| ToolError::failed("to is required"))?;

        let from = files::look_any(ctx, raw_from)?;
        let to = files::look_any(ctx, raw_to)?;

        if !from.exists {
            return Err(ToolError::failed(format!("not found: {raw_from}")));
        }
        if to.exists {
            return Err(ToolError::failed(format!(
                "{raw_to} already exists — move would overwrite it"
            )));
        }
        if from.abs == to.abs {
            return Err(ToolError::failed("from and to are the same"));
        }

        if let Some(parent) = to.abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::failed(e.to_string()))?;
        }
        std::fs::rename(&from.abs, &to.abs).map_err(|e| ToolError::failed(e.to_string()))?;
        ctx.ws.forget(&from.abs);
        ctx.ws.mark_read(&to.abs);

        // The content did not change — the diff is empty, and the move is labelled
        // by the interface from the fact (§5.5).
        let fact = FileFact {
            path: to.rel.clone(),
            kind: ChangeKind::Moved,
            moved_from: Some(from.rel.clone()),
            added: 0,
            removed: 0,
        };

        Ok(
            ToolOutput::new(format!("Moved {} → {}\n", from.rel, to.rel)).changed(FileChange {
                fact,
                diff: Default::default(),
            }),
        )
    }
}

/// File name without the path — the second tier of a permission request (§5.7).
fn short(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::ChangeKind;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn deletion_removes_file_from_disk() {
        let dir = TempDir::new();
        let path = dir.write("extra.rs", "body\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = DeleteFile
            .run(&ctx, json!({ "path": "extra.rs" }))
            .await
            .unwrap();

        assert_eq!(out.changed.unwrap().fact.kind, ChangeKind::Deleted);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn deleted_file_can_no_longer_be_read() {
        let dir = TempDir::new();
        dir.write("a.rs", "body\n");
        let (ctx, _ws) = ctx_for(&dir);

        DeleteFile
            .run(&ctx, json!({ "path": "a.rs" }))
            .await
            .unwrap();

        let err = DeleteFile
            .run(&ctx, json!({ "path": "a.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    fn write_binary(dir: &TempDir, name: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, [0x89, b'P', b'N', b'G', 0, 0, 1, 2, 0xff]).unwrap();
        path
    }

    #[tokio::test]
    async fn a_binary_file_can_be_deleted() {
        let dir = TempDir::new();
        let path = write_binary(&dir, "logo.png");
        let (ctx, _ws) = ctx_for(&dir);

        let out = DeleteFile
            .run(&ctx, json!({ "path": "logo.png" }))
            .await
            .unwrap();

        let fact = out.changed.unwrap().fact;
        assert_eq!(fact.kind, ChangeKind::Deleted);
        assert_eq!((fact.added, fact.removed), (0, 0));
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn a_binary_file_can_be_moved() {
        let dir = TempDir::new();
        let from = write_binary(&dir, "logo.png");
        let (ctx, _ws) = ctx_for(&dir);

        let out = MoveFile
            .run(&ctx, json!({ "from": "logo.png", "to": "assets/logo.png" }))
            .await
            .unwrap();

        assert_eq!(out.changed.unwrap().fact.kind, ChangeKind::Moved);
        assert!(!from.exists());
        assert!(dir.path().join("assets/logo.png").exists());
    }

    #[tokio::test]
    async fn moving_onto_an_existing_binary_file_is_refused() {
        let dir = TempDir::new();
        write_binary(&dir, "a.bin");
        let target = write_binary(&dir, "b.bin");
        let (ctx, _ws) = ctx_for(&dir);

        let err = MoveFile
            .run(&ctx, json!({ "from": "a.bin", "to": "b.bin" }))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("already exists"));
        assert!(target.exists());
    }

    #[tokio::test]
    async fn move_produces_one_line_not_two() {
        let dir = TempDir::new();
        let from = dir.write("was.rs", "body\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = MoveFile
            .run(&ctx, json!({ "from": "was.rs", "to": "src/became.rs" }))
            .await
            .unwrap();

        let fact = out.changed.unwrap().fact;
        assert_eq!(fact.kind, ChangeKind::Moved);
        assert_eq!(fact.moved_from.as_deref(), Some("was.rs"));
        assert!(!from.exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/became.rs")).unwrap(),
            "body\n"
        );
    }

    #[tokio::test]
    async fn moving_over_existing_file_is_forbidden() {
        let dir = TempDir::new();
        dir.write("a.rs", "first\n");
        dir.write("b.rs", "second\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = MoveFile
            .run(&ctx, json!({ "from": "a.rs", "to": "b.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use crate::checkpoint::scan::MAX_FILE_BYTES;
    use crate::permission::Invariant;
    use crate::testutil::TempDir;

    fn ws(dir: &TempDir) -> Workspace {
        Workspace::open(dir.path()).unwrap()
    }

    #[test]
    fn a_file_over_the_size_limit_is_a_question_that_names_the_loss() {
        let dir = TempDir::new();
        dir.write("data/dump.bin", &"x".repeat(MAX_FILE_BYTES as usize + 1));
        let need = DeleteFile
            .permission(&ws(&dir), &json!({ "path": "data/dump.bin" }))
            .unwrap();
        assert_eq!(need.unsaved.map(|u| u.files), Some(1));
    }

    #[test]
    fn a_file_inside_build_output_is_a_question() {
        let dir = TempDir::new();
        dir.write("pkg/target/debug/app.rlib", "built\n");
        let need = DeleteFile
            .permission(&ws(&dir), &json!({ "path": "pkg/target/debug/app.rlib" }))
            .unwrap();
        assert!(need.unsaved.is_some());
    }

    #[test]
    fn a_small_source_file_is_not_a_question_about_loss() {
        let dir = TempDir::new();
        dir.write("src/a.rs", "fn a() {}\n");
        let need = DeleteFile
            .permission(&ws(&dir), &json!({ "path": "src/a.rs" }))
            .unwrap();
        assert_eq!(need.unsaved, None);
        assert_eq!(need.invariant, None);
    }

    #[test]
    fn a_file_in_a_directory_the_project_excludes_is_a_question() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/settings.json",
            r#"{ "shadow": { "exclude": ["fixtures"] } }"#,
        );
        dir.write("fixtures/golden.json", "{}\n");
        let settings = crate::settings::Settings::load(dir.path()).unwrap();
        let ws = Workspace::open(dir.path())
            .unwrap()
            .with_shadow_exclude(&settings.shadow.exclude);

        let need = DeleteFile
            .permission(&ws, &json!({ "path": "fixtures/golden.json" }))
            .unwrap();
        assert!(need.unsaved.is_some(), "the snapshot skips fixtures/");
    }

    #[test]
    fn deleting_git_configuration_is_an_invariant() {
        let dir = TempDir::new();
        dir.write(".git/config", "[core]\n");
        dir.write(".gitattributes", "*.rs diff=x\n");
        dir.write("vendor/.gitmodules", "[submodule]\n");
        let ws = ws(&dir);
        for path in [".git/config", ".gitattributes", "vendor/.gitmodules"] {
            let need = DeleteFile
                .permission(&ws, &json!({ "path": path }))
                .unwrap();
            assert_eq!(need.invariant, Some(Invariant::GitConfig), "{path}");
        }
    }

    #[test]
    fn moving_a_file_into_git_is_an_invariant_on_either_end() {
        let dir = TempDir::new();
        dir.write("hook.sh", "#!/bin/sh\n");
        dir.write(".git/hooks/post-commit", "x\n");
        let ws = ws(&dir);

        let into = MoveFile
            .permission(
                &ws,
                &json!({ "from": "hook.sh", "to": ".git/hooks/post-commit" }),
            )
            .unwrap();
        assert_eq!(into.invariant, Some(Invariant::GitConfig));

        let out = MoveFile
            .permission(
                &ws,
                &json!({ "from": ".git/hooks/post-commit", "to": "hook.sh" }),
            )
            .unwrap();
        assert_eq!(out.invariant, Some(Invariant::GitConfig));
    }

    #[test]
    fn an_ordinary_git_adjacent_name_is_not_an_invariant() {
        let dir = TempDir::new();
        dir.write(".gitignore", "target\n");
        let need = DeleteFile
            .permission(&ws(&dir), &json!({ "path": ".gitignore" }))
            .unwrap();
        assert_eq!(need.invariant, None);
    }
}

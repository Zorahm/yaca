//! Creating a file and writing it completely. The interface captions it (§5.4.1):
//! "Created segments.ts +124 −0" — a fact derived by the tool itself.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::ChangeKind;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::PermissionNeed;
use crate::workspace::Workspace;

pub struct WriteFile;

#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "Write"
    }

    fn description(&self) -> &'static str {
        "Write a file completely: create a new file or replace the contents of an existing one. \
         For targeted changes to an existing file, use Edit — it is cheaper and \
         does not overwrite what you have not seen."
    }

    fn properties(&self) -> Value {
        json!({
            "path": { "type": "string", "description": "Path relative to the project root." },
            "content": { "type": "string", "description": "Full content of the file." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path", "content"]
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

    /// Asked in "Manual" mode (§8). There are no `+12 −3` counters here: they
    /// are known only after writing, and the purpose of the request is to stop
    /// the agent before the file is touched.
    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        Some(files::edit_need(ws, self.name(), raw, abs))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| ToolError::failed("content is required"))?
            .to_string();

        // Overwriting an existing file is the blind edit that read-before-edit
        // guards against. Creating a new file does not overwrite anything.
        let probe = files::look(ctx, raw)?;
        if probe.abs.is_dir() {
            return Err(ToolError::failed(format!("{raw} is a directory")));
        }
        let target = if probe.current.is_some() {
            files::look_editable(ctx, raw)?
        } else {
            probe
        };

        let existed = target.current.is_some();
        let kind = if existed {
            ChangeKind::Modified
        } else {
            ChangeKind::Created
        };
        let (fact, diff) = files::write(ctx, &target, kind, Some(&content))?;

        let verb = if existed { "Modified" } else { "Created" };
        Ok(ToolOutput::new(format!(
            "{verb} {} (+{} −{})\n",
            target.rel, fact.added, fact.removed
        ))
        .changed(FileChange { fact, diff }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::ChangeKind;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn creates_file_along_with_directory() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = WriteFile
            .run(
                &ctx,
                json!({ "path": "src/new.rs", "content": "fn main() {}\n" }),
            )
            .await
            .unwrap();

        let changed = out.changed.unwrap();
        assert_eq!(changed.fact.kind, ChangeKind::Created);
        assert_eq!(changed.fact.added, 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/new.rs")).unwrap(),
            "fn main() {}\n"
        );
    }

    #[tokio::test]
    async fn overwriting_unread_file_is_rejected() {
        let dir = TempDir::new();
        dir.write("a.rs", "foreign work\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = WriteFile
            .run(&ctx, json!({ "path": "a.rs", "content": "mine\n" }))
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
        assert!(err.to_string().contains("was not read"));
    }

    #[tokio::test]
    async fn overwriting_read_file_counts_as_edit() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = WriteFile
            .run(&ctx, json!({ "path": "a.rs", "content": "one\ntwo\n" }))
            .await
            .unwrap();

        let changed = out.changed.unwrap();
        assert_eq!(changed.fact.kind, ChangeKind::Modified);
        assert_eq!((changed.fact.added, changed.fact.removed), (1, 0));
    }

    #[tokio::test]
    async fn created_file_can_be_written_again_without_rereading() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        WriteFile
            .run(&ctx, json!({ "path": "new.rs", "content": "one\n" }))
            .await
            .unwrap();
        // Writing marks the file as read: otherwise the agent would hit "file
        // changed after reading" because of its own edit.
        let out = WriteFile
            .run(&ctx, json!({ "path": "new.rs", "content": "one\ntwo\n" }))
            .await
            .unwrap();

        assert_eq!(out.changed.unwrap().fact.kind, ChangeKind::Modified);
    }

    #[tokio::test]
    async fn writing_outside_project_is_rejected() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = WriteFile
            .run(&ctx, json!({ "path": "../outside.rs", "content": "x" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }
}

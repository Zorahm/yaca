//! The `Memory` tool: notes the agent keeps for the project between sessions.
//!
//! Not a file change, and so not in the change bar (§5.5). The change bar
//! lists what the human reviews and may roll back, and a note is the agent's
//! own record, not part of the work under review: a rollback of the turn must
//! not take a lesson with it, and Esc Esc does not reach `.yaca` at all (the
//! snapshot store leaves it out). The feed still shows every call and its
//! result, with the model's own caption, so nothing happens unseen.
//!
//! No permission question either. The tool can only touch `.yaca/memory/`, and
//! the name check plus the workspace boundary keep it there.

pub mod store;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::project;

pub struct Memory;

#[async_trait]
impl Tool for Memory {
    fn name(&self) -> &'static str {
        "Memory"
    }

    fn description(&self) -> &'static str {
        "Keep notes that outlive this session in the project's memory directory, \
         .yaca/memory/. Keep what a future session would otherwise have to rediscover: \
         a decision and its reason, a convention, where something lives, how the human \
         wants work done. Do not keep what is already in the code or in git, or what \
         is true only for the task at hand. \
         Operations: `write` creates or replaces a note from its name, a one-line hook \
         and a body; `read` returns a note; `delete` removes one. \
         MEMORY.md, the index of notes, is kept up to date by this tool, so do not edit \
         it by hand. A note written now can be read back with `read` at once, but it is \
         not in the index the prompt carries until the next session starts, so do not \
         rely on the prompt for it within this session. \
         Names are lowercase letters, digits and hyphens ending in .md, for example \
         build-quirks.md."
    }

    fn properties(&self) -> Value {
        json!({
            "op": {
                "type": "string",
                "enum": ["write", "read", "delete"],
                "description": "What to do with the note.",
            },
            "name": {
                "type": "string",
                "pattern": "^[a-z0-9-]+\\.md$",
                "description": "The note's file name, e.g. build-quirks.md.",
            },
            "title": {
                "type": "string",
                "description": "Optional human title for the index line; defaults to the name without .md.",
            },
            "hook": {
                "type": "string",
                "description": "Required for write. One line saying what the note is about; it is the line the index shows, so make it specific enough to decide from.",
            },
            "content": {
                "type": "string",
                "description": "Required for write. The note's body in markdown.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["op", "name"]
    }

    fn narration(&self) -> Narration {
        // Nothing in the interface knows what a note means, so the model says
        // it in its own words, as for the other tools without a file fact.
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["name"]
            .as_str()
            .map(|name| format!("{}/{}/{name}", project::DIR, store::NOTES_DIR))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let op = input["op"]
            .as_str()
            .ok_or_else(|| ToolError::failed("missing op"))?;
        let name = input["name"]
            .as_str()
            .ok_or_else(|| ToolError::failed("missing name"))?;

        match op {
            "write" => {
                let content = input["content"]
                    .as_str()
                    .ok_or_else(|| ToolError::failed("missing content"))?;
                let hook = input["hook"]
                    .as_str()
                    .ok_or_else(|| ToolError::failed("missing hook"))?;
                let title = input["title"].as_str();
                let replaced = store::write(&ctx.ws, name, title, hook, content)?;
                let verb = if replaced { "Replaced" } else { "Wrote" };
                Ok(ToolOutput::new(format!("{verb} note {name}\n")))
            }
            "read" => Ok(ToolOutput::new(store::read(&ctx.ws, name)?)),
            "delete" => {
                store::delete(&ctx.ws, name)?;
                Ok(ToolOutput::new(format!("Deleted note {name}\n")))
            }
            other => Err(ToolError::failed(format!("unknown operation: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;
    use crate::tool::ToolRegistry;

    #[tokio::test]
    async fn write_read_delete_through_the_tool() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = Memory
            .run(
                &ctx,
                json!({ "op": "write", "name": "deploy.md", "hook": "how releases go out", "content": "Tag first." }),
            )
            .await
            .unwrap();
        assert_eq!(out.text, "Wrote note deploy.md\n");

        let read = Memory
            .run(&ctx, json!({ "op": "read", "name": "deploy.md" }))
            .await
            .unwrap();
        assert!(read.text.contains("Tag first."));

        let out = Memory
            .run(&ctx, json!({ "op": "delete", "name": "deploy.md" }))
            .await
            .unwrap();
        assert_eq!(out.text, "Deleted note deploy.md\n");
        assert!(Memory
            .run(&ctx, json!({ "op": "read", "name": "deploy.md" }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_note_is_readable_at_once_and_is_not_a_change() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let out = Memory
            .run(&ctx, json!({ "op": "write", "name": "a.md", "hook": "h", "content": "b" }))
            .await
            .unwrap();
        // Not in the change bar: the tool reports no file change at all.
        assert!(out.changed.is_none());

        let out = Memory
            .run(&ctx, json!({ "op": "write", "name": "a.md", "hook": "h", "content": "c" }))
            .await
            .unwrap();
        assert_eq!(out.text, "Replaced note a.md\n");
    }

    #[tokio::test]
    async fn an_unknown_operation_or_a_missing_field_is_refused_with_a_reason() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = Memory
            .run(&ctx, json!({ "op": "rename", "name": "a.md" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown operation"), "{err}");

        let err = Memory
            .run(&ctx, json!({ "op": "write", "name": "a.md", "hook": "h" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("content"), "{err}");
    }

    #[tokio::test]
    async fn traversal_through_the_tool_is_rejected() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        for name in ["../escape.md", "/tmp/x.md", "sub/x.md"] {
            let err = Memory
                .run(&ctx, json!({ "op": "write", "name": name, "hook": "h", "content": "x" }))
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::Rejected(_)), "{name}: {err:?}");
        }
        assert!(!dir.path().join("escape.md").exists());
    }

    #[test]
    fn the_tool_exists_only_in_agent_mode() {
        let registry = ToolRegistry::standard();
        assert!(registry.get("Memory", Intent::Agent).is_some());
        assert!(registry.get("Memory", Intent::Plan).is_none());
    }

    #[test]
    fn the_caption_field_is_required_because_the_result_is_not_a_fact() {
        let schema = ToolRegistry::standard()
            .schemas(Intent::Agent)
            .into_iter()
            .find(|s| s.name == "Memory")
            .unwrap();
        let required = schema.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "description"));
        // Only `write` needs a hook or content, so the schema leaves them optional
        // and the tool refuses a write that lacks them.
        assert!(!required.iter().any(|r| r == "hook"));
    }
}

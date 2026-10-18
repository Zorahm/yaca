//! Task list (§5.6). Narrated by the interface: it renders items and their
//! statuses directly, so a model narration over them would repeat the same thing.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::event::{AgentEvent, TodoItem, TodoStatus};
use crate::intent::Intent;

pub struct TodoWrite;

#[async_trait]
impl Tool for TodoWrite {
    fn name(&self) -> &'static str {
        "TodoWrite"
    }

    fn description(&self) -> &'static str {
        "Write or update the task list. The list is shown to the human \
         and replaced as a whole: send it completely each time. \
         Exactly one task may be in status inProgress."
    }

    fn properties(&self) -> Value {
        json!({
            "todos": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "content": { "type": "string" },
                        "status": {
                            "type": "string",
                            "enum": ["pending", "inProgress", "completed"],
                        },
                    },
                    "required": ["content", "status"],
                    "additionalProperties": false,
                },
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["todos"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let items: Vec<TodoItem> = serde_json::from_value(input["todos"].clone())
            .map_err(|e| ToolError::failed(format!("failed to parse todos: {e}")))?;

        let in_progress = items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count();
        if in_progress > 1 {
            return Err(ToolError::failed(
                "at most one task may be in status inProgress",
            ));
        }

        ctx.events.emit(AgentEvent::TodosUpdated {
            items: items.clone(),
        });

        let done = items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        Ok(ToolOutput::new(format!(
            "Task list updated: {done} of {} completed\n",
            items.len()
        ))
        .items(items.len() as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_with_events;

    #[tokio::test]
    async fn update_emitted_as_event() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);

        TodoWrite
            .run(
                &ctx,
                json!({ "todos": [
                    { "content": "read spec",  "status": "completed" },
                    { "content": "write core", "status": "inProgress" },
                    { "content": "attach UI",  "status": "pending" },
                ]}),
            )
            .await
            .unwrap();

        match rx.try_recv().unwrap() {
            AgentEvent::TodosUpdated { items } => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[1].status, TodoStatus::InProgress);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn two_current_tasks_is_an_error() {
        let dir = TempDir::new();
        let (ctx, _rx) = ctx_with_events(&dir);

        let err = TodoWrite
            .run(
                &ctx,
                json!({ "todos": [
                    { "content": "one", "status": "inProgress" },
                    { "content": "two", "status": "inProgress" },
                ]}),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("at most one"));
        assert!(err.to_string().contains("inProgress"), "{err}");
    }

    #[test]
    fn the_description_names_the_statuses_the_schema_accepts() {
        // A model without constrained decoding trusts the prose over the enum:
        // told `in_progress`, it sends `in_progress`, and the list does not parse.
        let accepted = TodoWrite.properties()["todos"]["items"]["properties"]["status"]["enum"].clone();
        assert_eq!(accepted, json!(["pending", "inProgress", "completed"]));
        assert!(TodoWrite.description().contains("inProgress"));
        assert!(!TodoWrite.description().contains("in_progress"));
    }
}

//! Список задач (§5.6). Подписывает интерфейс: он сам рисует строки со
//! статусами, и подпись модели поверх них была бы вторым рассказом о том же.

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
            .map_err(|e| ToolError::failed(format!("не разобрал todos: {e}")))?;

        let in_progress = items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count();
        if in_progress > 1 {
            return Err(ToolError::failed(
                "в статусе inProgress может быть только одна задача",
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
            "Список обновлён: {done} из {} сделано\n",
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
    async fn обновление_уходит_событием() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);

        TodoWrite
            .run(
                &ctx,
                json!({ "todos": [
                    { "content": "прочитать спеку", "status": "completed" },
                    { "content": "написать ядро",   "status": "inProgress" },
                    { "content": "прикрутить UI",   "status": "pending" },
                ]}),
            )
            .await
            .unwrap();

        match rx.try_recv().unwrap() {
            AgentEvent::TodosUpdated { items } => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[1].status, TodoStatus::InProgress);
            }
            other => panic!("не то событие: {other:?}"),
        }
    }

    #[tokio::test]
    async fn две_текущие_задачи_это_ошибка() {
        let dir = TempDir::new();
        let (ctx, _rx) = ctx_with_events(&dir);

        let err = TodoWrite
            .run(
                &ctx,
                json!({ "todos": [
                    { "content": "раз", "status": "inProgress" },
                    { "content": "два", "status": "inProgress" },
                ]}),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("только одна"));
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

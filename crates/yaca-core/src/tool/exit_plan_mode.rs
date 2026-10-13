//! Leaving «План» mode (§7.1): show the plan and wait for the human's decision.
//!
//! The tool does not switch the mode itself. Switching is the human's decision,
//! and the core waits for the `SetMode` command rather than guessing it for them.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::event::AgentEvent;
use crate::intent::Intent;

pub struct ExitPlanMode;

#[async_trait]
impl Tool for ExitPlanMode {
    fn name(&self) -> &'static str {
        "ExitPlanMode"
    }

    fn description(&self) -> &'static str {
        "Present the finished plan and wait for the human to choose a mode \
         to execute it. Call when the plan is complete, not to report \
         progress."
    }

    fn properties(&self) -> Value {
        json!({
            "plan": {
                "type": "string",
                "description": "The entire plan in Markdown.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["plan"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan]
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let plan = input["plan"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан plan"))?
            .to_string();

        ctx.events.emit(AgentEvent::PlanProposed { plan });

        Ok(ToolOutput::new(
            "План показан человеку. Дождись решения: переключать режим самому нельзя.\n"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_with_events;

    #[tokio::test]
    async fn план_уходит_событием() {
        let dir = TempDir::new();
        let (ctx, mut rx) = ctx_with_events(&dir);

        ExitPlanMode
            .run(&ctx, json!({ "plan": "1. Прочитать\n2. Написать" }))
            .await
            .unwrap();

        match rx.try_recv().unwrap() {
            AgentEvent::PlanProposed { plan } => assert!(plan.contains("Прочитать")),
            other => panic!("не то событие: {other:?}"),
        }
    }

    #[test]
    fn доступен_только_в_режиме_plan() {
        assert_eq!(ExitPlanMode.intents(), &[Intent::Plan]);
    }
}

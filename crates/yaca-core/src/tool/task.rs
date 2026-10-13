//! `Task`: hand a piece of work to a subagent and get a report back.
//!
//! The tool is the door; the conversation behind it is
//! [`crate::session::subagent`].

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;

pub struct Task;

#[async_trait]
impl Tool for Task {
    fn name(&self) -> &'static str {
        "Task"
    }

    fn description(&self) -> &'static str {
        "Hand a self-contained task to a subagent. It works in a fresh context of          its own and returns only a report, so the steps it takes do not fill          yours. Use it for work that takes many steps and comes down to a short          result: finding where something is implemented or used, surveying an          unfamiliar part of the project, a contained change you can describe          completely, a question to research on the web. For something one or two          calls would settle, make the calls yourself.          The subagent has the tools you have, under the same mode and the same          permissions: the human sees its calls and is asked about them as about          yours. It sees none of this conversation and cannot ask a question, so          the prompt has to carry everything it needs. Independent tasks requested          together in one response run at the same time; give those different          files to work on. You wait for the report unless you set `background`:          then the call returns at once, you go on with your own work, and the          report is handed to you when it is ready. What the subagent read or changed, you have not seen:          read a file yourself before you edit it or rely on what it now says."
    }

    fn properties(&self) -> Value {
        json!({
            "prompt": {
                "type": "string",
                "description": "The task, complete on its own: what to do or find                     out, what you already know that narrows it, where it must stop,                     and what the report should contain.",
            },
            "background": {
                "type": "boolean",
                "description": "Set to true when you have work of your own to do                     while the subagent does this, on files it will not touch. Leave                     it out when the report is the next thing you need: waiting for                     it in place costs nothing.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["prompt"]
    }

    /// Why a subagent was sent is something only the model knows (§5.4.1).
    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// In plan as well: the mode binds the subagent as it binds the agent,
    /// so one sent from plan gets no tool that changes anything either.
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    /// Sending a subagent changes nothing and asks nothing by itself; what it
    /// goes on to do takes its turn like any other call (`session::calls`).
    /// Running several at once is most of the reason to have them.
    fn concurrent(&self) -> bool {
        true
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let prompt = input["prompt"]
            .as_str()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .ok_or_else(|| ToolError::failed("не задан prompt"))?;
        let agents = ctx
            .agents
            .as_ref()
            .ok_or_else(|| ToolError::failed("подагент не может запустить подагента"))?;

        if input["background"].as_bool() == Some(true) {
            return Ok(agents.send(prompt.to_string(), &ctx.cancel));
        }
        let report = agents.run(prompt.to_string(), &ctx.cancel).await?;
        Ok(ToolOutput::new(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn a_task_without_a_prompt_is_not_sent() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        for input in [json!({}), json!({ "prompt": "  " })] {
            let err = Task.run(&ctx, input).await.unwrap_err();
            assert!(err.to_string().contains("prompt"), "{err}");
        }
    }

    #[tokio::test]
    async fn a_subagent_cannot_send_another() {
        let dir = TempDir::new();
        // The context a subagent's own tools get carries no way to start one.
        let (ctx, _ws) = ctx_for(&dir);

        let err = Task
            .run(&ctx, json!({ "prompt": "look around" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "{err:?}");
    }
}

//! File delivery is separate from reading: the model receives only a receipt.

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::{event::AgentEvent, intent::Intent, ToolError};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct Send;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: String,
    title: Option<String>,
    caption: Option<String>,
    #[serde(default = "auto")]
    display_hint: String,
}
fn auto() -> String {
    "auto".into()
}

#[async_trait]
impl Tool for Send {
    fn name(&self) -> &'static str {
        "Send"
    }
    fn description(&self) -> &'static str {
        "Send a file from your workspace to the user in chat. Use this when you create a final file, image, chart, screenshot or document (.docx, .pdf, .png, .csv, etc.) to show or deliver. The application chooses the preview from the file type. You receive only a receipt; do not repeat the file contents."
    }
    fn properties(&self) -> Value {
        json!({
            "path": {"type": "string", "description": "Relative or absolute path to a file in the workspace."},
            "title": {"type": "string", "description": "Optional human-readable card title."},
            "caption": {"type": "string", "description": "Optional short caption."},
            "display_hint": {"type": "string", "enum": ["auto", "inline", "artifact", "attachment"], "default": "auto", "description": "Optional UI preference; auto chooses by MIME type. Unsupported hints fall back to a supported display."}
        })
    }
    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }
    fn narration(&self) -> Narration {
        Narration::Interface
    }
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }
    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }
    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let input: Input =
            serde_json::from_value(input).map_err(|e| ToolError::failed(e.to_string()))?;
        let ws = ctx.ws.clone();
        let file = tokio::task::spawn_blocking(move || {
            crate::sent::deliver(
                &ws,
                &input.path,
                input.title,
                input.caption,
                &input.display_hint,
            )
        })
        .await
        .map_err(|e| ToolError::failed(e.to_string()))??;
        let text = format!(
            "Success: File '{}' ({} bytes) was sent to the user ({}).",
            file.name,
            file.size,
            match file.render_type {
                crate::sent::RenderType::Inline => "inline",
                crate::sent::RenderType::Artifact => "artifact",
                crate::sent::RenderType::Attachment => "attachment",
            }
        );
        ctx.events.emit(AgentEvent::FileSent { file });
        Ok(ToolOutput::new(text))
    }
}

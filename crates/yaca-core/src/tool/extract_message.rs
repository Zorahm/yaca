//! A message of the conversation as a file.
//!
//! When the human pastes a page of their own text and asks for one paragraph
//! to be tightened, the model's only way to answer is to type the page again
//! with the paragraph changed. Here the text is put into a file as it was
//! written, and from there the road is the ordinary one: `Edit` changes what
//! was asked for, `SendText` shows the result.
//!
//! The file lives in the project's `.yaca`, under the session's name: it is
//! the session's scratch paper, not a part of the project, so it stays out of
//! the snapshots and out of git.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::message::Role;
use crate::store::transcript::Transcript;
use crate::workspace::Workspace;

pub struct ExtractMessage;

/// Where the draft of message `number` is kept.
fn draft_path(ws: &Workspace, transcript: &Transcript, number: usize) -> PathBuf {
    crate::project::dir(ws.home())
        .join("drafts")
        .join(transcript.session().to_string())
        .join(format!("{number}.md"))
}

#[async_trait]
impl Tool for ExtractMessage {
    fn name(&self) -> &'static str {
        "ExtractMessage"
    }

    fn description(&self) -> &'static str {
        "Save a message of this conversation into a file, word for word, and get \
         the file's path. Use it when the human asks you to change part of a long \
         text that is itself a message — theirs or yours: extract it, change the \
         file with Edit (the file counts as already read), then show the result \
         with SendText. That costs the changed lines instead of the whole text \
         typed again. Without `number` it takes the human's latest message; \
         SearchMessage gives the numbers of the others. A message extracted before \
         keeps its file and the edits made there."
    }

    fn properties(&self) -> Value {
        json!({
            "number": {
                "type": "integer",
                "minimum": 1,
                "description": "The number of the message, as SearchMessage lists it. Omit for the human's latest message.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &[]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// It writes a file, and the tools that change it are in agent mode only.
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["number"].as_u64().map(|number| format!("#{number}"))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let transcript = ctx
            .transcript
            .clone()
            .ok_or_else(|| ToolError::failed("this session has no transcript"))?;
        let entries = {
            let transcript = transcript.clone();
            tokio::task::spawn_blocking(move || transcript.entries())
                .await
                .map_err(|_| ToolError::failed("failed to read transcript"))?
                .map_err(|e| ToolError::failed(format!("failed to read transcript: {e}")))?
        };

        let entry = match input["number"].as_u64() {
            Some(number) => (number as usize)
                .checked_sub(1)
                .and_then(|index| entries.get(index))
                .ok_or_else(|| {
                    ToolError::failed(format!(
                        "message #{number} does not exist: there are {} messages in the conversation",
                        entries.len()
                    ))
                })?,
            None => entries
                .iter()
                .rev()
                .find(|entry| entry.role == Role::User)
                .ok_or_else(|| ToolError::failed("the conversation contains no user messages"))?,
        };

        let abs = draft_path(&ctx.ws, &transcript, entry.number);
        let rel = ctx.ws.display(&abs);

        // A second extraction of the same message would throw away the edits
        // made since the first, and those edits are the work.
        if abs.exists() {
            ctx.ws.mark_read(&abs);
            return Ok(ToolOutput::new(format!(
                "Message #{} is already in {rel}, not overwritten.\n",
                entry.number
            )));
        }

        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::failed(e.to_string()))?;
        }
        std::fs::write(&abs, &entry.text).map_err(|e| ToolError::failed(e.to_string()))?;
        // §8 asks that a file be read before it is edited so that the model
        // knows what it is changing. It read this text as a message.
        ctx.ws.mark_read(&abs);

        Ok(ToolOutput::new(format!(
            "Message #{} saved to {rel} · lines: {}\n",
            entry.number,
            entry.text.lines().count()
        )))
    }
}

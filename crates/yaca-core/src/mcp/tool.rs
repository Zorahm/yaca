//! A server's tool as a tool of the session.
//!
//! The model sees it under `mcp__<server>__<tool>`: the prefix keeps a server
//! from taking the name of a built-in tool, and the server's name keeps two
//! servers from taking each other's.

use std::path::Path;

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};

use super::hub::Mcp;
use super::protocol::{CallResult, Content, RemoteTool, ResourceContents};
use super::McpError;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::picture::{self, Picture};
use crate::tool::{Narration, Tool, ToolCtx, ToolOutput};
use crate::workspace::Workspace;

pub const PREFIX: &str = "mcp__";

/// What providers accept as a tool name.
const NAME_MAX_CHARS: usize = 64;

/// How much of a server's own description of a tool reaches the model. It is
/// part of the cached head of every request.
const DESCRIPTION_MAX_CHARS: usize = 2_000;

/// How much of a result reaches the model. A server can return a whole
/// database table, and the window is the session's, not the server's.
pub(crate) const RESULT_MAX_BYTES: usize = 100_000;

/// The name the model calls a server's tool by.
pub(crate) fn name_for(server: &str, tool: &str) -> String {
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect()
    };
    let name = format!("{PREFIX}{}__{}", clean(server), clean(tool));
    name.chars().take(NAME_MAX_CHARS).collect()
}

pub(crate) struct McpTool {
    hub: Mcp,
    server: String,
    /// The tool's name on its server.
    remote: String,
    name: String,
    description: String,
    schema: Value,
    read_only: bool,
    /// The server's schema has a `description` argument of its own, so the
    /// caption field of §5.4.1 cannot be added beside it.
    own_description: bool,
}

impl McpTool {
    pub(crate) fn new(hub: Mcp, server: &str, name: String, remote: &RemoteTool) -> Self {
        let schema = normalized(&remote.input_schema);
        let own_description = schema["properties"].get("description").is_some();
        let description = match remote.description.as_deref().map(str::trim) {
            Some(text) if !text.is_empty() => text.chars().take(DESCRIPTION_MAX_CHARS).collect(),
            _ => format!("The `{}` tool of the MCP server `{server}`.", remote.name),
        };
        McpTool {
            hub,
            server: server.to_string(),
            remote: remote.name.clone(),
            name,
            description,
            schema,
            read_only: remote.read_only(),
            own_description,
        }
    }
}

/// A server's schema in the shape the registry builds on: an object with
/// `properties`, and without the keys providers stumble over.
fn normalized(schema: &Value) -> Value {
    let mut schema = match schema {
        Value::Object(_) => schema.clone(),
        _ => json!({}),
    };
    // Checked to be an object just above.
    if let Some(object) = schema.as_object_mut() {
        object.insert("type".into(), json!("object"));
        if !object.get("properties").is_some_and(Value::is_object) {
            object.insert("properties".into(), json!({}));
        }
        // A dialect marker means nothing to a model, and OpenAI-compatible
        // servers refuse a schema that carries one.
        object.remove("$schema");
    }
    schema
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn properties(&self) -> Value {
        self.schema["properties"].clone()
    }

    fn required(&self) -> &[&str] {
        &[]
    }

    fn input_schema(&self) -> Option<Value> {
        Some(self.schema.clone())
    }

    /// §5.4.1 puts MCP calls with the actions only the model can caption.
    fn narration(&self) -> Narration {
        if self.own_description {
            Narration::Interface
        } else {
            Narration::Model
        }
    }

    fn keeps_description(&self) -> bool {
        self.own_description
    }

    /// Both: §8 gives "writing through MCP" a cell in the Plan column, and it
    /// says "ask".
    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        self.read_only
    }

    fn target(&self, _input: &Value) -> Option<String> {
        Some(format!("{} · {}", self.server, self.remote))
    }

    fn permission(&self, _ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        if self.read_only {
            return None;
        }
        let arguments = serde_json::to_string_pretty(input).unwrap_or_default();
        Some(
            PermissionNeed::new(PermissionKind::McpWrite, &self.name, &self.remote)
                .detail(arguments)
                .server(&self.server),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let failed = |err: McpError| match err {
            McpError::Cancelled => ToolError::Cancelled,
            other => ToolError::failed(format!("{}: {other}", self.server)),
        };
        let (client, deadline) = self.hub.client(&self.server).map_err(failed)?;
        let result = client
            .call_tool(&self.remote, input, deadline, &ctx.cancel)
            .await
            .map_err(failed)?;
        output(&self.server, result, ctx.vision, ctx.ws.home())
    }
}

/// A call's result as the session's tools return theirs.
///
/// `home` is the project, where the images are kept for the feed.
fn output(server: &str, result: CallResult, vision: bool, home: &Path) -> Result<ToolOutput, ToolError> {
    let mut text = String::new();
    let mut pictures: Vec<Picture> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    let mut add = |part: &str| {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(part);
    };

    for block in &result.content {
        match block {
            Content::Text { text } => add(text),
            Content::Image { data } => match decode_picture(data) {
                Ok((picture, bytes)) => {
                    // The human sees what the server returned, whether or
                    // not the model can.
                    kept.extend(keep(home, &bytes));
                    if vision {
                        add(&format!("[image {} — shown below]", pictures.len() + 1));
                        pictures.push(picture);
                    } else {
                        add("[image: this model cannot be shown images]");
                    }
                }
                Err(why) => add(&format!("[image that could not be read: {why}]")),
            },
            Content::Audio {} => add("[audio: not passed on]"),
            Content::Resource { resource } => add(&resource_text(resource)),
            Content::ResourceLink { uri, name } => match name {
                Some(name) => add(&format!("[resource {name}: {uri}]")),
                None => add(&format!("[resource: {uri}]")),
            },
            Content::Unknown => add("[content of a kind this client does not know]"),
        }
    }
    // Structured output alone: older clients are owed a text copy by the
    // protocol, but a server that skipped it still said something.
    if text.is_empty() {
        if let Some(structured) = &result.structured_content {
            text = serde_json::to_string_pretty(structured).unwrap_or_default();
        }
    }

    let (text, truncated) = capped(text, RESULT_MAX_BYTES);
    if result.is_error {
        let detail = if text.is_empty() { "The server gave no details.".to_string() } else { text };
        return Err(ToolError::failed_with(format!("{server}: the tool returned an error"), detail));
    }

    let mut out = ToolOutput::new(text).truncated(truncated);
    out.meta.pictures = kept;
    for (index, picture) in pictures.into_iter().enumerate() {
        out = out.picture(format!("image {}", index + 1), picture);
    }
    Ok(out)
}

/// The image for the model, and the bytes as the server sent them.
fn decode_picture(data: &str) -> Result<(Picture, Vec<u8>), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| e.to_string())?;
    let picture = picture::prepare(bytes.clone()).map_err(|e| e.to_string())?;
    Ok((picture, bytes))
}

/// Keeps an image of a result as a file of the project, at full size, and
/// says where, relative to the project. A file that cannot be written costs
/// the human the preview and the model nothing.
fn keep(home: &Path, bytes: &[u8]) -> Option<String> {
    match picture::stash(home, bytes) {
        Ok(path) => {
            let shown = path.strip_prefix(home).unwrap_or(&path);
            Some(shown.to_string_lossy().replace('\\', "/"))
        }
        Err(err) => {
            tracing::warn!(%err, "an MCP image was not kept for the feed");
            None
        }
    }
}

/// An embedded resource as text: its own, or a line saying what it is.
pub(crate) fn resource_text(resource: &ResourceContents) -> String {
    match (&resource.text, &resource.blob) {
        (Some(text), _) => text.clone(),
        (None, Some(blob)) => format!(
            "[binary resource {} ({}), {} bytes of base64 — not passed on]",
            resource.uri,
            resource.mime_type.as_deref().unwrap_or("unknown type"),
            blob.len()
        ),
        (None, None) => format!("[empty resource {}]", resource.uri),
    }
}

/// Cuts `text` to `max` bytes on a character boundary and says so in it.
pub(crate) fn capped(mut text: String, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let total = text.len();
    let cut = (0..=max).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
    text.truncate(cut);
    text.push_str(&format!(
        "\n[truncated: the first {cut} of {total} bytes are shown. Ask the server for less at a time.]"
    ));
    (text, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(value: Value) -> CallResult {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn a_name_is_made_of_what_providers_accept() {
        assert_eq!(name_for("files", "read"), "mcp__files__read");
        assert_eq!(name_for("my server", "do.it/now"), "mcp__my_server__do_it_now");
        assert_eq!(name_for("s", &"x".repeat(100)).chars().count(), 64);
    }

    #[test]
    fn a_schema_is_brought_to_the_shape_the_registry_builds_on() {
        let schema = normalized(&json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "properties": { "q": { "type": "string" } },
            "required": ["q"]
        }));
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["q"]));
        assert!(schema.get("$schema").is_none());
        // A server that sent no schema at all still gets a callable tool.
        assert_eq!(normalized(&Value::Null), json!({ "type": "object", "properties": {} }));
    }

    #[test]
    fn text_blocks_are_joined_and_links_are_named() {
        let out = output(
            "files",
            result(json!({ "content": [
                { "type": "text", "text": "one" },
                { "type": "text", "text": "two" },
                { "type": "resource_link", "uri": "file:///a.txt", "name": "a.txt" }
            ] })),
            true,
            Path::new("."),
        )
        .unwrap();
        assert_eq!(out.text, "one\ntwo\n[resource a.txt: file:///a.txt]");
        assert!(!out.meta.truncated);
    }

    #[test]
    fn structured_output_without_text_is_still_an_answer() {
        let out = output("s", result(json!({ "structuredContent": { "n": 1 } })), true, Path::new("."))
            .unwrap();
        assert!(out.text.contains("\"n\": 1"));
    }

    #[test]
    fn an_error_result_is_a_failed_call_with_the_servers_words() {
        let err = output(
            "tracker",
            result(json!({ "isError": true, "content": [{ "type": "text", "text": "no such task" }] })),
            true,
            Path::new("."),
        )
        .unwrap_err();
        assert!(err.to_string().contains("tracker"));
        assert!(err.for_model().contains("no such task"));
    }

    #[test]
    fn a_result_over_the_limit_is_cut_and_says_so() {
        let (text, cut) = capped("я".repeat(100), 51);
        assert!(cut);
        // Cut on a character boundary, so the two-byte letters stay whole.
        assert!(text.starts_with(&"я".repeat(25)));
        assert!(text.contains("truncated"));
        assert_eq!(capped("short".into(), 51), ("short".to_string(), false));
    }

    #[test]
    fn a_broken_image_is_named_rather_than_dropped() {
        let out = output(
            "s",
            result(json!({ "content": [{ "type": "image", "data": "not base64!", "mimeType": "image/png" }] })),
            true,
            Path::new("."),
        )
        .unwrap();
        assert!(out.text.contains("could not be read"));
        assert!(out.pictures.is_empty());
        assert!(out.meta.pictures.is_empty());
    }
}

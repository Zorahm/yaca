//! The messages of the Model Context Protocol this client reads and writes.
//!
//! Only the client's half, and only what the core uses: the handshake, tools,
//! resources and prompts. Every struct ignores fields it does not name, so a server on
//! a newer revision still parses.

use serde::Deserialize;
use serde_json::Value;

/// Revisions this client speaks, newest first. The first is what it offers; a
/// server may answer with any of the others.
pub(crate) const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub(crate) const TOOLS_LIST_CHANGED: &str = "notifications/tools/list_changed";
pub(crate) const PROMPTS_LIST_CHANGED: &str = "notifications/prompts/list_changed";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeResult {
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub server_info: Option<Implementation>,
    #[serde(default)]
    pub instructions: Option<String>,
}

/// Present means supported; what is inside is not read.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Capabilities {
    #[serde(default)]
    pub tools: Option<Value>,
    #[serde(default)]
    pub resources: Option<Value>,
    #[serde(default)]
    pub prompts: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptsPage {
    #[serde(default)]
    pub prompts: Vec<RemotePrompt>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// A prompt template a server offers, which Yaca shows as a slash command.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RemotePrompt {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PromptArgument {
    pub name: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetPromptResult {
    #[serde(default)]
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PromptMessage {
    #[serde(default)]
    pub role: String,
    pub content: Content,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Implementation {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolsPage {
    #[serde(default)]
    pub tools: Vec<RemoteTool>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Value,
    #[serde(default)]
    pub annotations: Option<Annotations>,
}

impl RemoteTool {
    /// The server says the tool changes nothing. Its own claim, and a hint by
    /// the protocol's wording; taken because the human chose the server, and
    /// because the alternative is a question before every search.
    pub(crate) fn read_only(&self) -> bool {
        self.annotations
            .as_ref()
            .and_then(|a| a.read_only_hint)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Annotations {
    #[serde(default)]
    pub read_only_hint: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CallResult {
    #[serde(default)]
    pub content: Vec<Content>,
    #[serde(default)]
    pub structured_content: Option<Value>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Content {
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        data: String,
    },
    Audio {},
    Resource {
        resource: ResourceContents,
    },
    ResourceLink {
        uri: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// A kind added after this was written.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourcesPage {
    #[serde(default)]
    pub resources: Vec<RemoteResource>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteResource {
    pub uri: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReadResult {
    #[serde(default)]
    pub contents: Vec<ResourceContents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceContents {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_result_with_a_kind_nobody_knows_still_parses() {
        let result: CallResult = serde_json::from_value(json!({
            "content": [
                { "type": "text", "text": "ok" },
                { "type": "hologram", "data": "…" },
                { "type": "resource_link", "uri": "file:///a", "name": "a" },
                { "type": "image", "data": "AAAA", "mimeType": "image/png" }
            ]
        }))
        .unwrap();
        assert!(matches!(result.content[0], Content::Text { .. }));
        assert!(matches!(result.content[1], Content::Unknown));
        assert!(matches!(result.content[2], Content::ResourceLink { .. }));
        assert!(matches!(result.content[3], Content::Image { .. }));
        assert!(!result.is_error);
    }

    #[test]
    fn a_tool_is_read_only_only_when_the_server_says_so() {
        let tool = |annotations: Value| -> RemoteTool {
            serde_json::from_value(json!({
                "name": "t", "inputSchema": { "type": "object" }, "annotations": annotations
            }))
            .unwrap()
        };
        assert!(tool(json!({ "readOnlyHint": true })).read_only());
        assert!(!tool(json!({ "readOnlyHint": false })).read_only());
        assert!(!tool(json!({})).read_only());
        assert!(!tool(Value::Null).read_only());
    }
}

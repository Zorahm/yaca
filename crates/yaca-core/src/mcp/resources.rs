//! Resources: what a server offers to be read, rather than done.
//!
//! Two tools of the client's own, present when at least one ready server has
//! resources. The model lists, then reads by address. Both only read, so
//! neither asks.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::hub::Mcp;
use super::tool::{capped, resource_text, RESULT_MAX_BYTES};
use super::McpError;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::tool::{Narration, Tool, ToolCtx, ToolOutput};

pub(crate) const LIST: &str = "ListMcpResources";
pub(crate) const READ: &str = "ReadMcpResource";

fn failed(server: &str, err: McpError) -> ToolError {
    match err {
        McpError::Cancelled => ToolError::Cancelled,
        other => ToolError::failed(format!("{server}: {other}")),
    }
}

pub(crate) struct ListResources(pub(crate) Mcp);

#[async_trait]
impl Tool for ListResources {
    fn name(&self) -> &str {
        LIST
    }

    fn description(&self) -> &str {
        "List the resources MCP servers offer: documents, records and other data a server \
         exposes for reading. Each line gives the server, the resource's URI, and what it is. \
         Read one with ReadMcpResource."
    }

    fn properties(&self) -> Value {
        json!({
            "server": {
                "type": "string",
                "description": "List only this server's resources. Leave out to list every server's."
            }
        })
    }

    fn required(&self) -> &[&str] {
        &[]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        true
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["server"].as_str().map(str::to_string)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let servers = match input["server"].as_str() {
            Some(one) => vec![one.to_string()],
            None => self.0.resource_servers(),
        };

        let mut text = String::new();
        let mut count = 0u32;
        for server in &servers {
            let (client, _) = self.0.client(server).map_err(|e| failed(server, e))?;
            let resources = client
                .list_resources(&ctx.cancel)
                .await
                .map_err(|e| failed(server, e))?;
            for resource in resources {
                count += 1;
                text.push_str(&format!("{server}\t{}", resource.uri));
                for part in [&resource.name, &resource.mime_type, &resource.description] {
                    if let Some(part) = part.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                        text.push('\t');
                        text.push_str(&part.replace(['\n', '\t'], " "));
                    }
                }
                text.push('\n');
            }
        }
        if count == 0 {
            text.push_str("No resources.");
        }
        let (text, truncated) = capped(text, RESULT_MAX_BYTES);
        Ok(ToolOutput::new(text).items(count).truncated(truncated))
    }
}

pub(crate) struct ReadResource(pub(crate) Mcp);

#[async_trait]
impl Tool for ReadResource {
    fn name(&self) -> &str {
        READ
    }

    fn description(&self) -> &str {
        "Read one resource of an MCP server by its URI, as ListMcpResources gives it."
    }

    fn properties(&self) -> Value {
        json!({
            "server": {
                "type": "string",
                "description": "The server that offers the resource."
            },
            "uri": {
                "type": "string",
                "description": "The resource's URI, exactly as listed."
            }
        })
    }

    fn required(&self) -> &[&str] {
        &["server", "uri"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        true
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["uri"].as_str().map(str::to_string)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let (Some(server), Some(uri)) = (input["server"].as_str(), input["uri"].as_str()) else {
            return Err(ToolError::failed_with(
                "server and uri are not set",
                "Both `server` and `uri` are required: take them from ListMcpResources.",
            ));
        };
        let (client, _) = self.0.client(server).map_err(|e| failed(server, e))?;
        let read = client
            .read_resource(uri, &ctx.cancel)
            .await
            .map_err(|e| failed(server, e))?;

        let parts: Vec<String> = read.contents.iter().map(resource_text).collect();
        let (text, truncated) = capped(parts.join("\n"), RESULT_MAX_BYTES);
        Ok(ToolOutput::new(text).truncated(truncated))
    }
}

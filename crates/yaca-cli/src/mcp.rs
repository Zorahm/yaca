//! `yaca mcp`: the project's MCP servers, connected to and listed.
//!
//! The same connection a session makes, without the session: the way to see
//! from a terminal whether a server starts and what it brings, and to approve
//! one from the project's `.mcp.json`, which a session does not start unasked.

use std::path::Path;

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use yaca_core::event::EventSink;
use yaca_core::mcp::{self, Mcp, McpSource, McpStatus};
use yaca_core::provider::registry;

pub async fn run(project: &Path, approve: Option<String>) -> Result<()> {
    let config_dir = registry::config_dir()?;
    // Nobody listens: the list is read once everything has settled.
    let (events, _) = tokio::sync::broadcast::channel(16);
    let servers = Mcp::for_project(
        project,
        &mcp::config::load_global(&config_dir)?,
        Some(&config_dir),
        EventSink::new(events),
    )?;
    if let Some(name) = &approve {
        servers.approve(name);
    }
    servers.settled(&CancellationToken::new()).await;

    let list = servers.snapshot();
    if list.is_empty() {
        println!("нет серверов: объявите их в .yaca/settings.json, .mcp.json или mcp.json");
    }
    for server in list {
        let source = match server.source {
            McpSource::Global => "mcp.json",
            McpSource::ProjectFile => ".mcp.json",
            McpSource::Project => ".yaca/settings.json",
        };
        let status = match server.status {
            McpStatus::Connecting => "подключается",
            McpStatus::Ready => "готов",
            McpStatus::Failed => "сбой",
            McpStatus::Disabled => "выключен",
            McpStatus::NeedsApproval => "ждёт одобрения: yaca mcp --approve <имя>",
        };
        println!("{}  {status}  ({source})  {}", server.name, server.target);
        if let Some(reason) = &server.reason {
            for line in reason.lines() {
                println!("    {line}");
            }
        }
        for tool in &server.tools {
            let mark = if tool.read_only { "читает" } else { "пишет " };
            let about = tool.description.as_deref().unwrap_or("");
            let about = about.lines().next().unwrap_or("");
            println!("    {mark}  {}  {about}", tool.name);
        }
        if server.resources {
            println!("    есть ресурсы");
        }
    }
    Ok(())
}

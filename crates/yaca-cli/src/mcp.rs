//! `yaca mcp`: the project's MCP servers, connected to and listed.
//!
//! The same connection a session makes, without the session: the way to see
//! from a terminal whether a server starts and what it brings, to approve
//! one from the project's `.mcp.json`, which a session does not start unasked,
//! and to log in to a remote server that asks for it.

use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use tokio_util::sync::CancellationToken;
use yaca_core::event::EventSink;
use yaca_core::mcp::{self, Mcp, McpSource, McpStatus};
use yaca_core::provider::registry;

pub async fn run(
    project: &Path,
    approve: Option<String>,
    login: Option<String>,
    logout: Option<String>,
) -> Result<()> {
    let config_dir = registry::config_dir()?;
    // Nobody listens: the list is read once everything has settled.
    let (events, _) = tokio::sync::broadcast::channel(16);
    let servers = Mcp::for_project(
        project,
        &mcp::config::load_global(&config_dir)?,
        &yaca_core::plugin::PluginSources::standard(&config_dir).load().enabled(),
        Some(&config_dir),
        EventSink::new(events),
    )?;
    if let Some(name) = &approve {
        servers.approve(name);
    }
    servers.settled(&CancellationToken::new()).await;
    if let Some(name) = &logout {
        servers.logout(name);
        servers.settled(&CancellationToken::new()).await;
        println!("вход на сервер {name} забыт");
    }
    if let Some(name) = &login {
        log_in(&servers, name).await;
    }

    let list = servers.snapshot();
    if list.is_empty() {
        println!("нет серверов: объявите их в .yaca/settings.json, .mcp.json или mcp.json");
    }
    for server in list {
        let source = match server.source {
            McpSource::Global => "mcp.json",
            McpSource::ProjectFile => ".mcp.json",
            McpSource::Project => ".yaca/settings.json",
            McpSource::Plugin => "плагин",
        };
        let status = match server.status {
            McpStatus::Connecting => "подключается",
            McpStatus::Ready => "готов",
            McpStatus::Failed => "сбой",
            McpStatus::Disabled => "выключен",
            McpStatus::NeedsApproval => "ждёт одобрения: yaca mcp --approve <имя>",
            McpStatus::NeedsLogin => "нужен вход: yaca mcp --login <имя>",
            McpStatus::LoggingIn => "идёт вход",
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

/// The OAuth login of one server: its page is opened in the browser (and
/// printed, for a terminal with no browser), and the server is connected
/// again once the login is done.
async fn log_in(servers: &Mcp, name: &str) {
    if !servers.snapshot().iter().any(|s| s.name == name) {
        println!("нет сервера {name}");
        return;
    }
    servers.login(name);
    let mut opened = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(330);
    while tokio::time::Instant::now() < deadline {
        let Some(server) = servers.snapshot().into_iter().find(|s| s.name == name) else {
            return;
        };
        if let (false, Some(url)) = (opened, &server.login_url) {
            println!("Войдите в браузере: {url}");
            open_browser(url);
            opened = true;
        }
        match server.status {
            McpStatus::LoggingIn | McpStatus::Connecting => {}
            McpStatus::Ready => {
                println!("вход выполнен, сервер {name} готов");
                return;
            }
            _ => {
                println!(
                    "вход не выполнен: {}",
                    server.reason.as_deref().unwrap_or("причина неизвестна")
                );
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    println!("вход не дождался браузера");
}

fn open_browser(url: &str) {
    let spawned = if cfg!(windows) {
        // Not `cmd /C start`: cmd would cut the address at its first `&`.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    let _ = spawned;
}

//! The same project servers and switches as the desktop, without a window.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use yaca_core::event::{AgentEvent, EventSink};
use yaca_core::lsp::runtime::Registry;
use yaca_core::LspStatus;

#[derive(Debug, Subcommand)]
pub enum Action {
    Enable { server: String },
    Disable { server: String },
    Install { server: String },
    Restart { server: String },
}

pub async fn run(root: &Path, action: Option<Action>) -> Result<()> {
    let registry = Registry::new(yaca_core::provider::registry::config_dir()?);
    let lsp = registry.project(root)?;
    let (tx, mut events) = tokio::sync::broadcast::channel(128);
    let _listener = lsp.listen(Arc::new(EventSink::new(tx)));
    let mut selected = None;
    let mut installing = false;
    if let Some(action) = action {
        let name = match &action {
            Action::Enable { server }
            | Action::Disable { server }
            | Action::Install { server }
            | Action::Restart { server } => server,
        };
        let server = lsp
            .snapshot()
            .into_iter()
            .find(|server| &server.name == name)
            .with_context(|| format!("No language server named {name} for this project"))?;
        match &action {
            Action::Enable { .. } => lsp.set_enabled(name, true),
            Action::Disable { .. } => lsp.set_enabled(name, false),
            Action::Restart { .. } => lsp.restart(name),
            Action::Install { .. } => {
                if server.status == LspStatus::Disabled {
                    bail!("Enable {name} before installing it");
                }
                let command = server
                    .installation
                    .context("This server has no built-in installer")?;
                println!("{command}");
                lsp.install(name);
                installing = true;
            }
        }
        selected = Some(name.clone());
    }
    tokio::time::timeout(
        Duration::from_secs(if installing { 335 } else { 35 }),
        async {
            loop {
                let pending = lsp.snapshot().iter().any(|server| {
                    selected.as_ref().is_none_or(|name| &server.name == name)
                        && matches!(
                            server.status,
                            LspStatus::Idle | LspStatus::Starting | LspStatus::Installing
                        )
                });
                if !pending {
                    break;
                }
                if let AgentEvent::Error { message, .. } = events.recv().await? {
                    bail!("{message}");
                }
            }
            Ok::<_, anyhow::Error>(())
        },
    )
    .await
    .context("Language server did not finish starting or installing")??;
    for server in lsp.snapshot() {
        println!(
            "{}: {:?} ({})",
            server.name,
            server.status,
            server.extensions.join(", ")
        );
        if let Some(reason) = server.reason {
            println!("  {reason}");
            if selected.as_ref() == Some(&server.name) {
                bail!("{} failed", server.name);
            }
        }
        if server.status == LspStatus::Missing {
            if let Some(command) = server.installation {
                println!("  Install: {command}");
            }
        }
    }
    // Errors from a synchronous switch (for example a read-only preferences
    // file) must not be hidden just because the previous status was ready.
    while let Ok(event) = events.try_recv() {
        if let AgentEvent::Error { message, .. } = event {
            bail!("{message}");
        }
    }
    Ok(())
}

//! `yaca-server`: Yaca's host on a machine without a window.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use yaca_server::tokens::Tokens;
use yaca_server::{router, Server};

#[derive(Parser)]
#[command(name = "yaca-server", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve the host. Put a TLS-terminating proxy in front of it before it
    /// leaves the machine: the socket itself is plain.
    Run {
        /// Address to listen on. Loopback by default, which is what a proxy on
        /// the same machine needs.
        #[arg(long, env = "YACA_LISTEN", default_value = "127.0.0.1:7878")]
        listen: SocketAddr,
    },
    /// The devices that may connect.
    #[command(subcommand)]
    Token(TokenCmd),
}

#[derive(Subcommand)]
enum TokenCmd {
    /// Make a token for a device and print it. It is shown once.
    Add { name: String },
    /// List the devices that have a token.
    List,
    /// Take a device's token away; the device is cut off at its next ping.
    Revoke { name: String },
}

fn config_dir() -> Result<PathBuf> {
    yaca_core::provider::registry::config_dir().context("could not find the settings directory")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Cli::parse().command {
        Cmd::Token(cmd) => {
            let tokens = Tokens::in_dir(&config_dir()?);
            match cmd {
                TokenCmd::Add { name } => println!("{}", tokens.add(&name)?),
                TokenCmd::List => {
                    for record in tokens.list()? {
                        println!("{}\tcreated {}", record.name, record.created);
                    }
                }
                TokenCmd::Revoke { name } => {
                    if tokens.revoke(&name)? {
                        println!("revoked {name}");
                    } else {
                        anyhow::bail!("no token named \"{name}\"");
                    }
                }
            }
            Ok(())
        }
        Cmd::Run { listen } => {
            let dir = config_dir()?;
            if Tokens::in_dir(&dir).list()?.is_empty() {
                anyhow::bail!("no device has a token yet: run `yaca-server token add <name>` first");
            }
            if !listen.ip().is_loopback() {
                tracing::warn!(%listen, "listening beyond this machine: the socket is not encrypted, put TLS in front of it");
            }
            let listener = tokio::net::TcpListener::bind(listen)
                .await
                .with_context(|| format!("could not listen on {listen}"))?;
            tracing::info!(%listen, "yaca-server {} ready", env!("CARGO_PKG_VERSION"));
            axum::serve(listener, router(Server::new(&dir)))
                .with_graceful_shutdown(shutdown())
                .await?;
            Ok(())
        }
    }
}

/// Ctrl-C, or the service manager's SIGTERM.
async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = term.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

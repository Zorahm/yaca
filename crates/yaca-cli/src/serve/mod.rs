//! `yaca serve`: this machine's projects, for a phone to open chats in, with no
//! window at all.
//!
//! The server itself is `yaca_remote`; this is the host it runs with. Running the
//! command is the consent to listen, so the `enabled` switch of the remote
//! settings is not read here: only the port is. The chats it starts are recorded
//! in the chats folder the window keeps, so the window lists them too.

mod commands;
mod host;
mod output;

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use yaca_core::provider::registry;
use yaca_remote::addresses::lan_addresses;
use yaca_remote::settings::{remote_dir, RemoteSettings};
use yaca_remote::{RemoteServer, ServerOptions};

use commands::Flow;
use host::CliHost;

#[derive(clap::Args, Debug)]
pub struct Options {
    /// The project folders the phone may open chats in. Without any, the
    /// project given with `-p` (default: the current folder).
    #[arg(value_name = "PROJECT")]
    pub projects: Vec<PathBuf>,
    /// The port to listen on. Without it, the port in the remote settings
    /// (47820 unless that was changed).
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,
    /// The address to listen on. The default reaches a phone on the network.
    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0")]
    pub bind: IpAddr,
    /// Show a pairing code at start, even when a phone is paired already.
    #[arg(long)]
    pub pair: bool,
}

/// Serves until Ctrl+C, or until `quit` is typed.
pub async fn run(default_project: &Path, options: Options) -> Result<()> {
    let config_dir = registry::config_dir()?;
    let requested = if options.projects.is_empty() {
        vec![default_project.to_path_buf()]
    } else {
        options.projects
    };
    let projects = host::canonical_projects(&requested)?;

    let dir = remote_dir(&config_dir);
    let port = options
        .port
        .unwrap_or_else(|| RemoteSettings::load(&dir).port);
    let bind = options.bind;
    let server = RemoteServer::start(
        ServerOptions { dir, port, bind },
        Arc::new(CliHost::new(projects.clone())),
    )
    .await
    .with_context(|| format!("could not listen on {}", SocketAddr::new(bind, port)))?;

    output::startup(
        server.local_addr(),
        server.fingerprint(),
        &projects,
        &lan_addresses(),
    );
    if options.pair || server.devices().is_empty() {
        output::pairing(&server.offer_pairing()?)?;
    }

    let mut lines = commands::read_stdin();
    let mut input_open = true;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            signal = &mut ctrl_c => {
                signal?;
                break;
            }
            line = lines.recv(), if input_open => match line {
                Some(line) => {
                    if commands::run_line(&server, &line) == Flow::Quit {
                        break;
                    }
                }
                // Standard input has ended, as it does for a server started
                // without a terminal. Serving goes on until Ctrl+C.
                None => input_open = false,
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use clap::Parser;

    use crate::{Args, Cmd};

    use super::*;

    fn serve_options(argv: &[&str]) -> Options {
        match Args::try_parse_from(argv).unwrap().command {
            Some(Cmd::Serve(options)) => options,
            other => panic!("expected serve, got {other:?}"),
        }
    }

    #[test]
    fn bare_serve_takes_the_defaults() {
        let options = serve_options(&["yaca", "serve"]);
        assert!(options.projects.is_empty());
        assert_eq!(options.port, None);
        assert_eq!(options.bind, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert!(!options.pair);
    }

    #[test]
    fn projects_port_bind_and_pair_are_read() {
        let options = serve_options(&[
            "yaca", "serve", "a", "b", "--port", "5000", "--bind", "127.0.0.1", "--pair",
        ]);
        assert_eq!(options.projects, vec![PathBuf::from("a"), PathBuf::from("b")]);
        assert_eq!(options.port, Some(5000));
        assert_eq!(options.bind, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert!(options.pair);
    }

    #[test]
    fn the_global_project_flag_works_before_or_after_serve() {
        let before = Args::try_parse_from(["yaca", "-p", "dir", "serve"]).unwrap();
        assert_eq!(before.project, PathBuf::from("dir"));
        let after = Args::try_parse_from(["yaca", "serve", "-p", "dir"]).unwrap();
        assert_eq!(after.project, PathBuf::from("dir"));
    }

    #[test]
    fn a_port_out_of_range_or_a_bad_address_is_refused() {
        assert!(Args::try_parse_from(["yaca", "serve", "--port", "70000"]).is_err());
        assert!(Args::try_parse_from(["yaca", "serve", "--bind", "not-an-address"]).is_err());
    }
}

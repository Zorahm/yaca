//! The console of `yaca serve`: what a person types while the server runs.
//!
//! Standard input is read on a thread of its own, so that Ctrl+C still stops the
//! server while a read is waiting. The commands are the few a host needs in the
//! field: a fresh pairing code, the paired devices, and revoking one.

use std::io::BufRead;

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use yaca_remote::protocol::DeviceId;
use yaca_remote::RemoteServer;

use super::output;

pub const HELP: &str =
    "commands: pair, devices, revoke <id>, help, quit (Ctrl+C also stops the server)";

#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Empty,
    Pair,
    Devices,
    Revoke(String),
    Help,
    Quit,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

pub fn parse(line: &str) -> Input {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        [] => Input::Empty,
        ["pair"] => Input::Pair,
        ["devices"] => Input::Devices,
        ["revoke", id] => Input::Revoke((*id).to_string()),
        ["help"] => Input::Help,
        ["quit"] => Input::Quit,
        _ => Input::Unknown,
    }
}

/// Runs one line. A failed command is reported and the server goes on: a typo at
/// the console must not stop a server that phones are connected to.
pub fn run_line(server: &RemoteServer, line: &str) -> Flow {
    match execute(server, parse(line)) {
        Ok(flow) => flow,
        Err(e) => {
            eprintln!("error: {e:#}");
            Flow::Continue
        }
    }
}

fn execute(server: &RemoteServer, input: Input) -> Result<Flow> {
    match input {
        Input::Empty => {}
        Input::Pair => output::pairing(&server.offer_pairing()?)?,
        Input::Devices => output::devices(&server.devices()),
        Input::Revoke(id) => {
            // The device's token stops working at once, and its open connections
            // close with it (the server does both).
            let existed = server
                .revoke(&DeviceId(id.clone()))
                .context("could not revoke the device")?;
            if existed {
                println!("revoked {id}");
            } else {
                println!("no device {id}");
            }
        }
        Input::Help | Input::Unknown => println!("{HELP}"),
        Input::Quit => return Ok(Flow::Quit),
    }
    Ok(Flow::Continue)
}

/// Lines of standard input, from a thread of its own. The channel ends when the
/// input does; the server takes that as "no more commands", not as a reason to
/// stop, since a server run without a terminal has no input at all.
pub fn read_stdin() -> mpsc::UnboundedReceiver<String> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_command_parses_to_its_own_input() {
        assert_eq!(parse("pair"), Input::Pair);
        assert_eq!(parse("devices"), Input::Devices);
        assert_eq!(parse("help"), Input::Help);
        assert_eq!(parse("quit"), Input::Quit);
        assert_eq!(parse("revoke 3f2a"), Input::Revoke("3f2a".into()));
    }

    #[test]
    fn surrounding_space_does_not_matter() {
        assert_eq!(parse("  devices  "), Input::Devices);
        assert_eq!(parse("\trevoke   3f2a \r"), Input::Revoke("3f2a".into()));
    }

    #[test]
    fn a_blank_line_is_nothing() {
        assert_eq!(parse(""), Input::Empty);
        assert_eq!(parse("   "), Input::Empty);
    }

    #[test]
    fn revoke_needs_exactly_one_id() {
        assert_eq!(parse("revoke"), Input::Unknown);
        assert_eq!(parse("revoke a b"), Input::Unknown);
    }

    #[test]
    fn anything_else_is_unknown() {
        assert_eq!(parse("frobnicate"), Input::Unknown);
        assert_eq!(parse("pair now"), Input::Unknown);
    }
}

//! The session panel of §9 and the key store of §10, from a terminal.
//!
//! Not a convenience. `yaca-cli` exists so that nothing in the core can only be
//! reached from the webview, and §9's panel is a plain API on
//! [`SessionStore`] precisely so that both sides can reach it. If a session
//! cannot be listed, searched, renamed, duplicated or exported from here, the
//! window has grown a feature the core does not have.

use anyhow::{Context, Result};
use clap::Subcommand;
use yaca_core::id::{SessionId, TurnId};
use yaca_core::provider::{models, registry};
use yaca_core::secrets::{self, KeySource};
use yaca_core::store::{MatchKind, SessionStore};

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// List the project's sessions, most recently touched first.
    Sessions,

    /// Search by message text and by the paths a session changed.
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Print a session as Markdown.
    Export { id: String },

    /// Give a session a name of its own.
    Rename { id: String, title: String },

    /// Delete a session: feed, history and index.
    Delete { id: String },

    /// Copy a session, optionally cutting it after a turn.
    Duplicate {
        id: String,
        /// Turn id to keep up to. Read one off `--jsonl` output.
        #[arg(long)]
        through: Option<String>,
    },

    /// The provider key in the system keychain (§10).
    Key {
        #[command(subcommand)]
        action: KeyAction,

        /// Which provider's key, by its id in `providers.json`.
        ///
        /// Required: with several providers, guessing which one was meant
        /// would store the key under the wrong name and leave the keychain
        /// quietly wrong.
        #[arg(long)]
        provider: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum KeyAction {
    /// Where the key is coming from, without printing it.
    Status,
    /// Read a key from stdin and store it.
    Set,
    /// Remove the key from the keychain.
    Clear,
}

/// Subcommand names, for telling a mistyped invocation from a prompt.
const NAMES: &[&str] = &[
    "sessions",
    "search",
    "export",
    "rename",
    "delete",
    "duplicate",
    "key",
    "mcp",
];

pub fn is_command_name(text: &str) -> bool {
    NAMES.contains(&text.trim())
}

pub fn run(root: &std::path::Path, cmd: Cmd) -> Result<()> {
    let store = SessionStore::open(root);

    match cmd {
        Cmd::Sessions => {
            for meta in store.list() {
                let title = if meta.title.is_empty() {
                    "(без названия)"
                } else {
                    &meta.title
                };
                println!(
                    "{}  {}  {} msg  ${:.4}  {}",
                    meta.id, meta.updated_at.0, meta.messages, meta.cost_usd, title
                );
            }
        }

        Cmd::Search { query, limit } => {
            for hit in store.search(&query, limit) {
                println!("{}  {}", hit.session.id, hit.session.title);
                for m in hit.matches {
                    let label = match m.kind {
                        MatchKind::Title => "title",
                        MatchKind::Path => "path",
                        MatchKind::Message => "text",
                    };
                    println!("    {label:>5}  {}", m.excerpt);
                }
            }
        }

        Cmd::Export { id } => {
            print!("{}", store.export_markdown(&SessionId::from(id))?);
        }

        Cmd::Rename { id, title } => {
            store.rename(&SessionId::from(id), &title)?;
        }

        Cmd::Delete { id } => {
            store.delete(&SessionId::from(id))?;
        }

        Cmd::Duplicate { id, through } => {
            let copy = store.duplicate(&SessionId::from(id), through.map(TurnId::from).as_ref())?;
            // The new id is the only thing worth printing: it is what
            // `--resume` takes next.
            println!("{}", copy.id);
        }

        Cmd::Key { action, provider } => key(action, &provider)?,
    }
    Ok(())
}

fn key(action: KeyAction, provider: &str) -> Result<()> {
    match action {
        KeyAction::Status => {
            let source = secrets::key_source(provider);
            println!(
                "{}",
                match source {
                    KeySource::Keychain => "keychain".to_string(),
                    KeySource::Environment =>
                        format!("{} (environment)", secrets::env_var(provider)),
                    KeySource::None => "no key".to_string(),
                }
            );
        }
        KeyAction::Set => {
            // Read rather than taken as an argument: an argument ends up in
            // the shell history and in the process list of every user on the
            // machine.
            let mut key = String::new();
            std::io::stdin()
                .read_line(&mut key)
                .context("не прочитать ключ со стандартного ввода")?;
            secrets::store_api_key(provider, key.trim())?;
            println!("stored in the keychain");
            ask_for_models(provider)?;
        }
        KeyAction::Clear => {
            secrets::forget_api_key(provider)?;
            println!("removed from the keychain");
        }
    }
    Ok(())
}

/// Asks the provider for its models now that there is a key to ask with — the
/// same `models::refresh` the window runs on the same occasion.
///
/// The key is already stored when this runs, so a provider that refuses is
/// reported and not returned as the command's failure. A name that is not in
/// the list is no provider to ask: the web search key is set this way too.
fn ask_for_models(provider: &str) -> Result<()> {
    let dir = registry::config_dir()?;
    if registry::Providers::load(&dir)?.get(provider).is_none() {
        return Ok(());
    }
    // `run` is not async, and the one caller is already inside the runtime
    // `main` started: blocking in place hands this thread's other work over
    // instead of starving it.
    let asked = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(models::refresh(&dir, provider))
    });
    match asked {
        Ok(providers) => {
            let count = providers.get(provider).map_or(0, |p| p.models.len());
            println!("{provider} answered with {count} models");
        }
        Err(e) => println!("{provider} did not answer with its models: {e}"),
    }
    Ok(())
}

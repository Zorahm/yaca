//! `yaca plugins`: what a new chat would load, the switches, and installing.
//!
//! The same sources the window reads — Yaca's own plugin folder, what Yaca
//! installed, what Claude Code installed — so a plugin turned off here is off
//! there too.

use anyhow::{anyhow, bail, Result};
use clap::Subcommand;
use yaca_core::plugin::install::{CatalogEntry, Installer};
use yaca_core::plugin::install::marketplace::MarketplaceOrigin;
use yaca_core::plugin::{self, PluginFormat, PluginOrigin, PluginSources, PluginSummary};
use yaca_core::provider::registry;

#[derive(Subcommand, Debug)]
pub enum Action {
    /// The marketplaces plugins are installed from.
    Marketplace {
        #[command(subcommand)]
        action: Market,
    },
    /// What a marketplace offers, narrowed by a word in the name or description.
    Browse { marketplace: String, query: Option<String> },
    /// Install `plugin@marketplace`, or a plugin from a folder, `owner/repo`
    /// or a git address. Installed plugins are on.
    Install { target: String },
    /// Fetch an installed plugin again, or all of them.
    Update { id: Option<String> },
    /// Remove a plugin Yaca installed. Its PLUGIN_DATA is kept.
    Uninstall { id: String },
}

#[derive(Subcommand, Debug)]
pub enum Market {
    List,
    /// A folder, `owner/repo` on GitHub, or a git address.
    Add { source: String },
    Remove { name: String },
    /// Fetch one marketplace again, or all that were fetched before.
    Update { name: Option<String> },
}

pub fn run(action: Option<Action>, enable: Option<String>, disable: Option<String>) -> Result<()> {
    let config_dir = registry::config_dir()?;
    let installer = Installer::standard(&config_dir);
    match action {
        None => {}
        Some(Action::Marketplace { action }) => return market(&installer, action),
        Some(Action::Browse { marketplace, query }) => {
            let entries = installer.catalog(&marketplace)?;
            print_catalog(&entries, query.as_deref());
            return Ok(());
        }
        Some(Action::Install { target }) => {
            let summary = install(&installer, &target)?;
            println!("Установлен и включён:");
            print_plugin(&summary);
            return Ok(());
        }
        Some(Action::Update { id }) => {
            let outcomes = match id {
                Some(id) => vec![(id.clone(), installer.update(&id))],
                None => installer.update_all(),
            };
            if outcomes.is_empty() {
                println!("Через Yaca ничего не установлено.");
            }
            for (id, outcome) in outcomes {
                match outcome {
                    Ok(o) if o.changed => println!("{id}: {} → {}", o.from, o.to),
                    Ok(_) => println!("{id}: без изменений"),
                    Err(e) => println!("{id}: {e}"),
                }
            }
            return Ok(());
        }
        Some(Action::Uninstall { id }) => {
            installer.uninstall(&id)?;
            println!("Удалён {id}. Его данные остались в plugin-data.");
            return Ok(());
        }
    }

    let sources = PluginSources::standard(&config_dir);
    for (id, on) in [(enable, true), (disable, false)] {
        if let Some(id) = id {
            if sources.load().get(&id).is_none() {
                bail!("нет плагина {id}; список — yaca plugins");
            }
            plugin::set_enabled(&config_dir, &id, on)?;
        }
    }

    let report = sources.load().report();
    if report.plugins.is_empty() {
        println!("Плагинов нет.");
    }
    for p in &report.plugins {
        print_plugin(p);
    }
    if !report.diagnostics.is_empty() {
        println!();
        for d in &report.diagnostics {
            println!("не загружен: {} — {}", d.path, d.message);
        }
    }
    Ok(())
}

/// `plugin@marketplace` when the part after `@` is a marketplace on the list;
/// anything else is an address (`git@github.com:…` has an `@` too).
fn install(installer: &Installer, target: &str) -> Result<PluginSummary> {
    let is_entry = target
        .rsplit_once('@')
        .is_some_and(|(_, m)| installer.marketplaces().iter().any(|k| k.name == m));
    let summary = if is_entry {
        installer.install(target)
    } else {
        installer.install_from(target)
    };
    summary.map_err(|e| anyhow!(e))
}

fn market(installer: &Installer, action: Market) -> Result<()> {
    match action {
        Market::List => {
            for m in installer.marketplaces() {
                let origin = match m.origin {
                    MarketplaceOrigin::Builtin => "встроенный",
                    MarketplaceOrigin::ClaudeCode => "из Claude Code",
                    MarketplaceOrigin::Own => "ваш",
                };
                let state = match (m.fetched, m.plugins, &m.error) {
                    (_, _, Some(e)) => format!("ошибка: {e}"),
                    (true, Some(n), _) => format!("плагинов: {n}"),
                    _ => "ещё не загружен".to_string(),
                };
                println!("{} ({origin}) — {}, {state}", m.name, m.source);
            }
        }
        Market::Add { source } => {
            let m = installer.add_marketplace(&source)?;
            println!("Добавлен {}: плагинов {}", m.name, m.plugins.unwrap_or(0));
        }
        Market::Remove { name } => {
            installer.remove_marketplace(&name)?;
            println!("Убран {name}.");
        }
        Market::Update { name } => {
            let names: Vec<String> = match name {
                Some(n) => vec![n],
                None => installer.marketplaces().into_iter().filter(|m| m.fetched).map(|m| m.name).collect(),
            };
            for name in names {
                match installer.refresh_marketplace(&name) {
                    Ok(m) => println!("{}: плагинов {}", m.name, m.plugins.unwrap_or(0)),
                    Err(e) => println!("{name}: {e}"),
                }
            }
        }
    }
    Ok(())
}

fn print_catalog(entries: &[CatalogEntry], query: Option<&str>) {
    let query = query.map(str::to_lowercase);
    let matches = |e: &CatalogEntry| {
        query.as_deref().is_none_or(|q| {
            e.name.to_lowercase().contains(q) || e.description.as_deref().unwrap_or("").to_lowercase().contains(q)
        })
    };
    for e in entries.iter().filter(|e| matches(e)) {
        let state = match (&e.installed, &e.unavailable) {
            (Some(v), _) => format!(" [установлен {v}]"),
            (None, Some(why)) => format!(" [{why}]"),
            _ => String::new(),
        };
        println!("{}{state}", e.id);
        if let Some(d) = &e.description {
            println!("  {d}");
        }
    }
}

fn print_plugin(p: &PluginSummary) {
    let state = if p.enabled { "вкл" } else { "выкл" };
    let origin = match p.origin {
        PluginOrigin::Yaca => "папка plugins",
        PluginOrigin::Marketplace => "установлен Yaca",
        PluginOrigin::ClaudeCode => "Claude Code",
    };
    let format = match p.format {
        PluginFormat::Claude => "формат Claude",
        PluginFormat::AgentPlugins => "Agent Plugins",
    };
    let version = p.version.as_deref().map(|v| format!(" {v}")).unwrap_or_default();
    println!("[{state}] {}{version} ({origin}, {format})", p.id);
    if let Some(d) = &p.description {
        println!("  {d}");
    }
    println!(
        "  навыков {}, команд {}, серверов MCP {}, хуков {}",
        p.skills, p.commands, p.mcp_servers, p.hooks
    );
    println!("  {}", p.path);
    for note in &p.notes {
        println!("  внимание: {note}");
    }
}

//! Plugins: packages of skills, commands, MCP servers and hooks, in either of
//! two formats.
//!
//! * Claude Code's (`.claude-plugin/plugin.json`), read leniently ([`claude`]).
//! * Agent Plugins 1.0 (agent-plugins.org, root `plugin.json`), read as
//!   strictly as its specification asks ([`agent`]).
//!
//! A package that carries both is read as Claude Code's: it has more
//! components, and joining the two would load each MCP server twice.
//!
//! Where plugins come from ([`PluginSources`]), in this order:
//!
//! 1. `<config dir>/plugins/<dir>/` — Yaca's own, put there by hand.
//! 2. What Yaca installed from a marketplace or an address ([`install`]).
//! 3. What Claude Code installed for the human (`scope: user` in
//!    `~/.claude/plugins/installed_plugins.json`). Read, never written: Yaca
//!    keeps its own switches and its own data directories.
//!
//! Every component carries the plugin's name as a prefix — `superpowers:tdd`,
//! `/tools:review`, the MCP server `plugin:tools:files` — so two plugins never
//! fight over a name, and the first of two plugins with one name wins.
//!
//! Application-level, like the human's `mcp.json`: the applications load the
//! plugins and hand the enabled ones to the core through
//! [`SessionConfig::plugins`](crate::SessionConfig). A test session has none,
//! so what it does never depends on the machine it runs on.

pub mod agent;
pub mod claude;
pub mod hooks;
pub mod install;
mod state;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::error::CoreError;
use crate::mcp::{McpServerSettings, McpSource, PluginLaunch, ServerSpec};
use crate::slash::SlashCommand;
use crate::workspace::roots::plain;
use hooks::PluginHook;

/// Yaca's own plugins, under the configuration directory.
pub const DIR: &str = "plugins";
/// Where each plugin's `PLUGIN_DATA` lives, under the configuration directory.
pub const DATA_DIR: &str = "plugin-data";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PluginFormat {
    Claude,
    AgentPlugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PluginOrigin {
    /// `<config dir>/plugins/`.
    Yaca,
    /// Installed by Claude Code.
    ClaudeCode,
    /// Installed by Yaca, from a marketplace or an address (`install`).
    Marketplace,
}

/// A plugin, loaded.
#[derive(Debug, Clone)]
pub struct Plugin {
    /// What the switches are keyed by: the manifest name for Yaca's own,
    /// `name@marketplace` for Claude Code's, as Claude Code keys them.
    pub id: String,
    /// The prefix of its components.
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    /// Canonical.
    pub root: PathBuf,
    /// `PLUGIN_DATA`: Yaca's, never Claude Code's.
    pub data: PathBuf,
    pub format: PluginFormat,
    pub origin: PluginOrigin,
    pub enabled: bool,
    pub skill_dirs: Vec<PathBuf>,
    /// Directories of named subagents (`agents/`), Claude Code's format.
    pub agent_dirs: Vec<PathBuf>,
    /// Named `<plugin>:<command>`, the plugin's root put in their bodies.
    pub commands: Vec<SlashCommand>,
    /// By the plugin's own names; [`mcp_specs`] prefixes them.
    pub mcp_servers: BTreeMap<String, McpServerSettings>,
    pub hooks: Vec<PluginHook>,
    /// What did not load and why, in words for the human.
    pub notes: Vec<String>,
}

/// What a format's reader makes of a package, before Yaca decides who it is.
#[derive(Debug, Default)]
pub struct Contents {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub skill_dirs: Vec<PathBuf>,
    pub agent_dirs: Vec<PathBuf>,
    pub commands: Vec<SlashCommand>,
    pub mcp_servers: BTreeMap<String, McpServerSettings>,
    pub hooks: Vec<PluginHook>,
    pub notes: Vec<String>,
}

impl Contents {
    pub(crate) fn named(name: String, version: Option<String>, description: Option<String>) -> Self {
        Contents {
            name,
            version,
            description,
            ..Default::default()
        }
    }
}

/// `path` resolved, if it stays inside `root` (canonical) once links are
/// followed. `None` for a path that leaves it, and for one that does not exist.
pub(crate) fn contained(root: &Path, path: &Path) -> Option<PathBuf> {
    let real = std::fs::canonicalize(path).ok()?;
    real.starts_with(root).then_some(real)
}

/// Something about a plugin that kept it from loading at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginDiagnostic {
    pub path: String,
    pub message: String,
}

/// A plugin as the settings window and the CLI show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginSummary {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub origin: PluginOrigin,
    pub format: PluginFormat,
    pub enabled: bool,
    pub path: String,
    pub skills: u32,
    pub commands: u32,
    pub mcp_servers: u32,
    pub hooks: u32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginReport {
    pub plugins: Vec<PluginSummary>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

/// Where plugins are looked for.
#[derive(Debug, Clone)]
pub struct PluginSources {
    config_dir: PathBuf,
    /// Claude Code's directory, `~/.claude`. `None` reads only Yaca's own.
    claude_home: Option<PathBuf>,
}

impl PluginSources {
    pub fn new(config_dir: impl Into<PathBuf>, claude_home: Option<PathBuf>) -> Self {
        PluginSources {
            config_dir: config_dir.into(),
            claude_home,
        }
    }

    /// Yaca's own and Claude Code's, where this machine keeps them.
    pub fn standard(config_dir: &Path) -> Self {
        PluginSources::new(config_dir, claude_home())
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Every plugin found, enabled or not, and the reasons for those that
    /// would not load.
    pub fn load(&self) -> PluginCatalog {
        let switches = state::Switches::load(&self.config_dir);
        let mut catalog = PluginCatalog::default();
        let data_root = self.config_dir.join(DATA_DIR);

        for dir in state::own_plugins(&self.config_dir.join(DIR)) {
            catalog.add(&dir, None, PluginOrigin::Yaca, &data_root, &switches);
        }
        // Ahead of Claude Code's: the same plugin installed in both is Yaca's
        // copy, which Yaca updates.
        for (id, path) in install::installed(&self.config_dir) {
            catalog.add(&path, Some((id, None)), PluginOrigin::Marketplace, &data_root, &switches);
        }
        if let Some(home) = &self.claude_home {
            let enabled_in_claude = state::claude_enabled(home);
            for install in state::claude_installs(home, &mut catalog.diagnostics) {
                let fallback = enabled_in_claude.get(&install.id).copied();
                catalog.add(
                    &install.path,
                    Some((install.id, fallback)),
                    PluginOrigin::ClaudeCode,
                    &data_root,
                    &switches,
                );
            }
        }
        catalog
    }
}

/// `~/.claude`, or where `CLAUDE_CONFIG_DIR` says Claude Code keeps it.
pub fn claude_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".claude"))
}

/// The plugins found under a [`PluginSources`].
#[derive(Debug, Clone, Default)]
pub struct PluginCatalog {
    plugins: Vec<Plugin>,
    diagnostics: Vec<PluginDiagnostic>,
}

impl PluginCatalog {
    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn diagnostics(&self) -> &[PluginDiagnostic] {
        &self.diagnostics
    }

    /// The ones a session gets.
    pub fn enabled(&self) -> Vec<Plugin> {
        self.plugins.iter().filter(|p| p.enabled).cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.id == id)
    }

    /// `installed` is the id the plugin was installed under and the switch its
    /// installer keeps for it (Claude Code's `enabledPlugins`); `None` for a
    /// plugin put in Yaca's folder by hand.
    fn add(
        &mut self,
        dir: &Path,
        installed: Option<(String, Option<bool>)>,
        origin: PluginOrigin,
        data_root: &Path,
        switches: &state::Switches,
    ) {
        let shown = plain(dir);
        let Ok(root) = std::fs::canonicalize(dir) else {
            return self.reject(&shown, "the plugin directory cannot be opened".into());
        };
        let installed_as = installed.as_ref().map(|(id, _)| id.split('@').next().unwrap_or(id).to_string());
        let (format, contents) = match read_package(&root, installed_as.as_deref()) {
            Ok(read) => read,
            Err(why) => return self.reject(&shown, why),
        };
        if let Some(first) = self.plugins.iter().find(|p| p.name == contents.name) {
            let why = format!("plugin «{}» is already loaded from {}", contents.name, plain(&first.root));
            return self.reject(&shown, why);
        }

        let (id, fallback) = match installed {
            Some((id, in_claude)) => (id, in_claude.unwrap_or(true)),
            None => (contents.name.clone(), true),
        };
        let data = data_root.join(data_name(&id));
        self.plugins.push(Plugin {
            enabled: switches.get(&id).unwrap_or(fallback),
            id,
            name: contents.name,
            version: contents.version,
            description: contents.description,
            root,
            data,
            format,
            origin,
            skill_dirs: contents.skill_dirs,
            agent_dirs: contents.agent_dirs,
            commands: contents.commands,
            mcp_servers: contents.mcp_servers,
            hooks: contents.hooks,
            notes: contents.notes,
        });
    }

    fn reject(&mut self, path: &str, message: String) {
        self.diagnostics.push(PluginDiagnostic {
            path: path.to_string(),
            message,
        });
    }

    pub fn report(&self) -> PluginReport {
        PluginReport {
            plugins: self.plugins.iter().map(summary).collect(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

/// Reads a package in whichever format it is in. `installed_as` is the name an
/// installer gave it: what was installed is a Claude Code plugin with or
/// without a manifest, and without one it takes that name.
pub(crate) fn read_package(root: &Path, installed_as: Option<&str>) -> Result<(PluginFormat, Contents), String> {
    if root.join(claude::MANIFEST_DIR).join(claude::MANIFEST).is_file() {
        Ok((PluginFormat::Claude, claude::load(root, installed_as)?))
    } else if root.join(agent::MANIFEST).is_file() {
        Ok((PluginFormat::AgentPlugins, agent::load(root)?))
    } else if installed_as.is_some() {
        Ok((PluginFormat::Claude, claude::load(root, installed_as)?))
    } else {
        Err("no manifest: neither .claude-plugin/plugin.json nor plugin.json".into())
    }
}

/// A directory name for an id: `name@marketplace` keeps its `@`, anything a
/// filesystem might refuse becomes `_`.
fn data_name(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || "-_.@".contains(c) { c } else { '_' })
        .collect()
}

fn summary(p: &Plugin) -> PluginSummary {
    let skills = p
        .skill_dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|r| r.flatten())
        .filter(|e| e.path().join(crate::skills::catalog::FILE).is_file())
        .count();
    PluginSummary {
        id: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone(),
        description: p.description.clone(),
        origin: p.origin,
        format: p.format,
        enabled: p.enabled,
        path: plain(&p.root),
        skills: skills as u32,
        commands: p.commands.len() as u32,
        mcp_servers: p.mcp_servers.len() as u32,
        hooks: p.hooks.len() as u32,
        notes: p.notes.clone(),
    }
}

/// Turns a plugin on or off for the sessions opened after this.
pub fn set_enabled(config_dir: &Path, id: &str, enabled: bool) -> Result<(), CoreError> {
    let mut switches = state::Switches::load(config_dir);
    switches.set(id, enabled);
    switches.save(config_dir)
}

/// Drops a plugin's switch, for one that is no longer installed.
pub(crate) fn forget_switch(config_dir: &Path, id: &str) -> Result<(), CoreError> {
    let mut switches = state::Switches::load(config_dir);
    switches.forget(id);
    switches.save(config_dir)
}

/// The MCP servers of `plugins`, as the session declares them: each under
/// `plugin:<plugin>:<server>`, started from its plugin's root and with the
/// plugin's variables.
pub fn mcp_specs(plugins: &[Plugin]) -> Vec<ServerSpec> {
    plugins
        .iter()
        .flat_map(|p| {
            p.mcp_servers.iter().map(move |(name, settings)| ServerSpec {
                name: format!("plugin:{}:{name}", p.name),
                source: McpSource::Plugin,
                settings: settings.clone(),
                plugin: Some(PluginLaunch {
                    root: p.root.clone(),
                    data: p.data.clone(),
                    // Agent Plugins §9: nothing but the two plugin variables
                    // is expanded; Claude Code expands the environment too.
                    expand_env: p.format == PluginFormat::Claude,
                }),
                app: None,
            })
        })
        .collect()
}

/// The commands of `plugins`, in name order.
pub fn commands(plugins: &[Plugin]) -> Vec<SlashCommand> {
    let mut out: Vec<SlashCommand> = plugins.iter().flat_map(|p| p.commands.iter().cloned()).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

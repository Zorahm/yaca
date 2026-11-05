//! Reviewable component declarations. Environment values and HTTP credentials
//! are deliberately omitted from the settings report.

use super::{
    hooks::{HookRun, PluginHook},
    PluginOrigin,
};
use super::{Plugin, PluginSummary};
use crate::workspace::roots::plain;
use crate::{mcp::McpServerSettings, slash::SlashCommand};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginDuplicate {
    pub path: String,
    pub id: String,
    pub origin: PluginOrigin,
    pub message: String,
    pub ignored: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PluginContents {
    pub skills: Vec<String>,
    pub commands: Vec<PluginCommand>,
    pub hooks: Vec<PluginHookSummary>,
    pub mcp_servers: Vec<PluginServer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PluginCommand {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PluginHookSummary {
    pub event: String,
    pub command: String,
    pub runner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PluginServer {
    pub name: String,
    pub command: String,
}

pub(super) fn contents(
    root: &Path,
    skill_dirs: &[PathBuf],
    commands: &[SlashCommand],
    servers: &BTreeMap<String, McpServerSettings>,
    hooks: &[PluginHook],
) -> PluginContents {
    let mut skills: Vec<String> = skill_dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flat_map(|read| read.flatten())
        .filter(|entry| entry.path().join(crate::skills::catalog::FILE).is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    skills.sort();
    skills.dedup();
    let replace_root = |s: &str| s.replace(&crate::workspace::roots::plain(root), "${PLUGIN_ROOT}");
    PluginContents {
        skills,
        commands: commands
            .iter()
            .map(|c| PluginCommand {
                name: c.name.clone(),
                description: c.description.clone(),
            })
            .collect(),
        hooks: hooks
            .iter()
            .map(|h| {
                let (command, runner) = match &h.run {
                    HookRun::Shell {
                        command,
                        powershell,
                    } => (
                        command.clone(),
                        if *powershell { "PowerShell" } else { "bash" },
                    ),
                    HookRun::Exec { program, args } => (
                        format!(
                            "{program} {}",
                            args.iter()
                                .map(|a| format!("{a:?}"))
                                .collect::<Vec<_>>()
                                .join(" ")
                        ),
                        "exec",
                    ),
                };
                PluginHookSummary {
                    event: h.event.name().into(),
                    command: replace_root(&command),
                    runner: runner.into(),
                }
            })
            .collect(),
        mcp_servers: servers
            .iter()
            .map(|(name, s)| PluginServer {
                name: name.clone(),
                command: replace_root(&s.url.clone().unwrap_or_else(|| {
                    format!(
                        "{} {}",
                        s.command.as_deref().unwrap_or_default(),
                        s.args
                            .iter()
                            .map(|a| format!("{a:?}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                })),
            })
            .collect(),
    }
}

pub(super) fn summary(p: &Plugin) -> PluginSummary {
    let contents = contents(
        &p.root,
        &p.skill_dirs,
        &p.commands,
        &p.mcp_servers,
        &p.hooks,
    );
    PluginSummary {
        id: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone(),
        description: p.description.clone(),
        origin: p.origin,
        format: p.format,
        enabled: p.enabled,
        path: plain(&p.root),
        skills: contents.skills.len() as u32,
        commands: p.commands.len() as u32,
        mcp_servers: p.mcp_servers.len() as u32,
        hooks: p.hooks.len() as u32,
        notes: p.notes.clone(),
        contents,
        duplicates: Vec::new(),
    }
}

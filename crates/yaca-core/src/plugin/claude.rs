//! Claude Code plugins: `.claude-plugin/plugin.json` and its components.
//!
//! Read leniently, as Claude Code reads them: only `name` is required, and a
//! component Yaca has no subsystem for is a note on the plugin rather than a
//! reason to drop it. Most plugins on a machine were written for Claude Code
//! and tested nowhere else; refusing one for a field Yaca does not use would
//! leave the human with nothing.
//!
//! The manifest may point a component at other paths. For skills, commands,
//! hooks and MCP servers those add to the default location rather than replace
//! it, which is Claude Code's rule; the same directory named twice is read once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::hooks::{self, PluginHook};
use super::{contained, Contents};
use crate::mcp::McpServerSettings;
use crate::slash::SlashCommand;
use crate::workspace::roots::plain;

pub const MANIFEST_DIR: &str = ".claude-plugin";
pub const MANIFEST: &str = "plugin.json";

/// Loads the plugin at `root` (canonical), or says why it cannot be.
///
/// The manifest is optional in Claude Code: without one, the components are
/// looked for in their default places and the plugin takes the name it was
/// installed under, `fallback_name`.
pub fn load(root: &Path, fallback_name: Option<&str>) -> Result<Contents, String> {
    let fields = match read_manifest(root)? {
        Some(fields) => fields,
        None => match fallback_name {
            Some(name) => serde_json::Map::from_iter([("name".to_string(), Value::String(name.into()))]),
            None => return Err("no .claude-plugin/plugin.json".into()),
        },
    };
    let name = match fields.get("name").and_then(Value::as_str).map(str::trim) {
        Some(n) if is_namespace(n) => n.to_string(),
        Some(n) => return Err(format!("name «{n}» cannot be typed after a slash: only Latin letters, digits, - _ . are allowed")),
        None => return Err("plugin.json has no name".into()),
    };
    let text_field = |key: &str| fields.get(key).and_then(Value::as_str).map(str::to_string);
    let mut c = Contents::named(name, text_field("version"), text_field("description"));

    for dir in paths(root, "skills", fields.get("skills"), &mut c.notes) {
        if dir.is_dir() {
            push_unique(&mut c.skill_dirs, dir);
        }
    }

    for dir in paths(root, "agents", fields.get("agents"), &mut c.notes) {
        if dir.is_dir() {
            push_unique(&mut c.agent_dirs, dir);
        }
    }

    let mut command_files = Vec::new();
    for path in paths(root, "commands", fields.get("commands"), &mut c.notes) {
        command_files.extend(markdown_files(&path));
    }
    command_files.sort();
    command_files.dedup();
    c.commands = commands(root, &c.name, &command_files);

    c.mcp_servers = mcp(root, fields.get("mcpServers"), &mut c.notes);
    c.hooks = hooks_of(root, fields.get("hooks"), &mut c.notes);

    let empty = c.skill_dirs.is_empty()
        && c.agent_dirs.is_empty()
        && c.commands.is_empty()
        && c.mcp_servers.is_empty()
        && c.hooks.is_empty();
    for (key, default, what) in [
        ("lspServers", ".lsp.json", "language servers"),
        ("outputStyles", "output-styles", "output styles"),
    ] {
        if fields.contains_key(key) || root.join(default).exists() {
            c.notes.push(format!("{what} of the plugin are not supported in Yaca"));
        }
    }
    if empty && c.notes.is_empty() {
        c.notes.push("the plugin has nothing that Yaca can load".into());
    }
    Ok(c)
}

fn read_manifest(root: &Path) -> Result<Option<serde_json::Map<String, Value>>, String> {
    let path = root.join(MANIFEST_DIR).join(MANIFEST);
    if !path.exists() {
        return Ok(None);
    }
    let manifest = contained(root, &path).ok_or(".claude-plugin/plugin.json points outside the plugin")?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("cannot read plugin.json: {e}"))?;
    match serde_json::from_str(&text).map_err(|e| format!("plugin.json is not JSON: {e}"))? {
        Value::Object(fields) => Ok(Some(fields)),
        _ => Err("plugin.json is not an object".into()),
    }
}

/// What a component's name may be prefixed with: it is typed after a slash.
pub fn is_namespace(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// The default location of a component and whatever the manifest adds, each
/// checked to stay inside the plugin. Paths that do not exist are left out.
fn paths(root: &Path, default: &str, declared: Option<&Value>, notes: &mut Vec<String>) -> Vec<PathBuf> {
    let mut raw = vec![default.to_string()];
    match declared {
        None => {}
        Some(Value::String(s)) => raw.push(s.clone()),
        Some(Value::Array(items)) => raw.extend(items.iter().filter_map(Value::as_str).map(str::to_string)),
        Some(_) => notes.push(format!("{default}: the manifest has neither a path nor a list of paths")),
    }
    let mut out = Vec::new();
    for path in raw {
        let full = root.join(path.trim_start_matches("./"));
        if !full.exists() {
            continue;
        }
        match contained(root, &full) {
            Some(p) => push_unique(&mut out, p),
            None => notes.push(format!("{path} points outside the plugin and was skipped")),
        }
    }
    out
}

fn push_unique(list: &mut Vec<PathBuf>, path: PathBuf) {
    if !list.contains(&path) {
        list.push(path);
    }
}

/// The `.md` files a command path stands for: the file itself, or those
/// directly in the directory.
fn markdown_files(path: &Path) -> Vec<PathBuf> {
    let is_md = |p: &Path| p.extension().is_some_and(|e| e == "md");
    if path.is_file() {
        return if is_md(path) { vec![path.to_path_buf()] } else { Vec::new() };
    }
    let Ok(read) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    read.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_md(p)).collect()
}

/// Commands under the plugin's name: `commands/review.md` of `tools` is
/// `/tools:review`. The plugin's root is put where the body names it, since
/// that is how a command reaches the scripts it ships with.
fn commands(root: &Path, namespace: &str, files: &[PathBuf]) -> Vec<SlashCommand> {
    let root_text = plain(root);
    let mut out: Vec<SlashCommand> = files
        .iter()
        .filter_map(|file| {
            let stem = file.file_stem()?.to_str()?;
            let text = std::fs::read_to_string(file).ok()?;
            let mut command = crate::slash::parse(format!("{namespace}:{stem}"), &text);
            command.body = substitute(&command.body, &root_text);
            crate::slash::is_command_name(&command.name).then_some(command)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// `${CLAUDE_PLUGIN_ROOT}` and `${PLUGIN_ROOT}` in text a plugin ships.
pub fn substitute(text: &str, root: &str) -> String {
    text.replace("${CLAUDE_PLUGIN_ROOT}", root).replace("${PLUGIN_ROOT}", root)
}

/// `.mcp.json` at the root and whatever `mcpServers` adds: a path to such a
/// file, a list of paths, or the servers written inline.
fn mcp(root: &Path, declared: Option<&Value>, notes: &mut Vec<String>) -> BTreeMap<String, McpServerSettings> {
    let mut servers = BTreeMap::new();
    let files = match declared {
        Some(Value::Object(inline)) => {
            let map = inline.get("mcpServers").and_then(Value::as_object).unwrap_or(inline);
            add_servers(map, &mut servers, notes);
            paths(root, ".mcp.json", None, notes)
        }
        other => paths(root, ".mcp.json", other, notes),
    };
    for file in files.into_iter().filter(|f| f.is_file()) {
        let parsed = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let Some(Value::Object(doc)) = parsed else {
            notes.push(format!("{} is not JSON, its servers were not loaded", plain(&file)));
            continue;
        };
        let map = doc.get("mcpServers").and_then(Value::as_object).unwrap_or(&doc);
        add_servers(map, &mut servers, notes);
    }
    servers
}

fn add_servers(
    map: &serde_json::Map<String, Value>,
    servers: &mut BTreeMap<String, McpServerSettings>,
    notes: &mut Vec<String>,
) {
    for (name, entry) in map {
        if entry.get("type").and_then(Value::as_str) == Some("sse") {
            notes.push(format!("server «{name}»: the sse transport is not supported in Yaca"));
            continue;
        }
        match serde_json::from_value::<McpServerSettings>(entry.clone()) {
            Ok(settings) => {
                servers.insert(name.clone(), settings);
            }
            Err(e) => notes.push(format!("server «{name}» could not be parsed: {e}")),
        }
    }
}

/// `hooks/hooks.json` and whatever `hooks` adds: a path, a list of paths, or
/// the hooks written inline.
fn hooks_of(root: &Path, declared: Option<&Value>, notes: &mut Vec<String>) -> Vec<PluginHook> {
    let mut out = Vec::new();
    let files = match declared {
        Some(inline @ Value::Object(_)) => {
            out.extend(hooks::parse(inline, notes));
            paths(root, "hooks/hooks.json", None, notes)
        }
        other => paths(root, "hooks/hooks.json", other, notes),
    };
    for file in files.into_iter().filter(|f| f.is_file()) {
        match std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            Some(doc) => out.extend(hooks::parse(&doc, notes)),
            None => notes.push(format!("{} is not JSON, its hooks were not loaded", plain(&file))),
        }
    }
    out
}

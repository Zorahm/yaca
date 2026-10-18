//! Agent Plugins 1.0 (agent-plugins.org): the portable format, read strictly.
//!
//! Strict because the specification is: a closed manifest schema, fixed
//! component locations, and failure boundaries it spells out (§4, §5, §7). A
//! client that loads what the spec says to reject is a client plugin authors
//! cannot test against. Where the spec says "report and ignore", this reports
//! and goes on; where it says "reject", the plugin, the component or the entry
//! is dropped and the reason is a note.
//!
//! Only the two portable components exist here: `skills/` and `mcp.json`.
//! Everything else in the package belongs to some client's extension namespace,
//! which Yaca does not implement and so ignores without looking (§8).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde_json::{Map, Value};

use super::{contained, Contents};
use crate::mcp::McpServerSettings;

pub const MANIFEST: &str = "plugin.json";
pub const PLUGIN_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
pub const MCP_SCHEMA: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";
const MCP_FILE: &str = "mcp.json";
const SKILLS_DIR: &str = "skills";

const TOP_LEVEL: [&str; 10] = [
    "$schema",
    "name",
    "version",
    "description",
    "author",
    "homepage",
    "repository",
    "license",
    "keywords",
    "extensions",
];

/// Loads the plugin at `root` (canonical), or says why it is rejected.
pub fn load(root: &Path) -> Result<Contents, String> {
    let manifest = contained(root, &root.join(MANIFEST))
        .ok_or_else(|| "plugin.json points outside the plugin".to_string())?;
    let text = std::fs::read_to_string(&manifest).map_err(|e| format!("cannot read plugin.json: {e}"))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("plugin.json is not JSON: {e}"))?;
    let Value::Object(fields) = value else {
        return Err("plugin.json is not an object".into());
    };

    let mut notes = Vec::new();
    match fields.get("$schema") {
        Some(Value::String(s)) if s == PLUGIN_SCHEMA => {}
        Some(Value::String(s)) => return Err(format!("unsupported Agent Plugins version: {s}")),
        _ => return Err("plugin.json has no $schema".into()),
    }
    let name = match fields.get("name") {
        Some(Value::String(n)) if is_plugin_name(n) => n.clone(),
        Some(Value::String(n)) => return Err(format!("name «{n}» does not follow the Agent Plugins rules")),
        _ => return Err("plugin.json has no name".into()),
    };
    for key in ["version", "description", "homepage", "repository", "license"] {
        if fields.get(key).is_some_and(|v| !v.is_string()) {
            return Err(format!("field {key} must be a string"));
        }
    }
    check_author(fields.get("author"))?;
    if let Some(keywords) = fields.get("keywords") {
        let ok = keywords.as_array().is_some_and(|a| a.iter().all(Value::is_string));
        if !ok {
            return Err("keywords must be a list of strings".into());
        }
    }
    if fields.get("extensions").is_some_and(|v| !v.is_object()) {
        notes.push("extensions is not an object and was skipped".to_string());
    }
    for key in fields.keys().filter(|k| !TOP_LEVEL.contains(&k.as_str())) {
        notes.push(format!("unknown plugin.json field «{key}» was skipped"));
    }

    let mut contents = Contents::named(name, string(&fields, "version"), string(&fields, "description"));
    contents.notes = notes;

    let skills = root.join(SKILLS_DIR);
    if skills.exists() {
        match contained(root, &skills) {
            Some(dir) if dir.is_dir() => contents.skill_dirs.push(dir),
            Some(_) => contents.notes.push("skills is not a directory, its skills were not loaded".into()),
            None => contents.notes.push("skills points outside the plugin, its skills were not loaded".into()),
        }
    }

    let mcp = root.join(MCP_FILE);
    if mcp.exists() {
        match load_mcp(root, &mcp) {
            Ok((servers, notes)) => {
                contents.mcp_servers = servers;
                contents.notes.extend(notes);
            }
            Err(why) => contents.notes.push(format!("mcp.json: {why}; the plugin's servers are turned off")),
        }
    }
    Ok(contents)
}

/// §5: lowercase letters, digits, `-` and `.`, alphanumeric at both ends, no
/// doubled separator.
pub fn is_plugin_name(name: &str) -> bool {
    let ok_char = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.';
    let alnum = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    (1..=64).contains(&name.len())
        && name.bytes().all(ok_char)
        && alnum(name.as_bytes().first())
        && alnum(name.as_bytes().last())
        && !name.contains("--")
        && !name.contains("..")
}

fn check_author(author: Option<&Value>) -> Result<(), String> {
    let Some(author) = author else { return Ok(()) };
    let Value::Object(fields) = author else {
        return Err("author must be an object".into());
    };
    for (key, value) in fields {
        if !["name", "email", "url"].contains(&key.as_str()) || !value.is_string() {
            return Err(format!("author.{key} is not allowed by the schema"));
        }
    }
    Ok(())
}

fn string(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(Value::as_str).map(str::to_string)
}

type Servers = BTreeMap<String, McpServerSettings>;

/// §7.2. An error disables MCP for the plugin; a bad entry skips that server.
fn load_mcp(root: &Path, path: &Path) -> Result<(Servers, Vec<String>), String> {
    let path = contained(root, path).ok_or("file points outside the plugin")?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot be read: {e}"))?;
    let Value::Object(fields) = serde_json::from_str::<Value>(&text).map_err(|e| format!("not JSON: {e}"))? else {
        return Err("not an object".into());
    };
    match fields.get("$schema") {
        Some(Value::String(s)) if s == MCP_SCHEMA => {}
        Some(Value::String(s)) => return Err(format!("version {s} does not match plugin.json")),
        _ => return Err("no $schema".into()),
    }
    if let Some(key) = fields.keys().find(|k| *k != "$schema" && *k != "mcpServers") {
        return Err(format!("extra field «{key}»"));
    }
    let Some(Value::Object(servers)) = fields.get("mcpServers") else {
        return Err("mcpServers must be an object".into());
    };

    let mut out = BTreeMap::new();
    let mut notes = Vec::new();
    for (name, entry) in servers {
        match server(root, entry) {
            Ok(Some(settings)) => {
                out.insert(name.clone(), settings);
            }
            Ok(None) => notes.push(format!("server «{name}»: the sse transport is not supported in Yaca")),
            Err(why) => notes.push(format!("server «{name}» was skipped: {why}")),
        }
    }
    Ok((out, notes))
}

/// One server entry, or `None` for a transport Yaca does not speak.
/// `${PLUGIN_ROOT}` and `${PLUGIN_DATA}` are left in `args`, `env` and `cwd` as
/// written: the session fills them in when it starts the server
/// (`mcp::config`), and nothing else is expanded there (§9).
fn server(root: &Path, entry: &Value) -> Result<Option<McpServerSettings>, String> {
    let Value::Object(fields) = entry else {
        return Err("not an object".into());
    };
    let kind = fields.get("type").and_then(Value::as_str).ok_or("no type")?;
    let allowed: &[&str] = match kind {
        "stdio" => &["type", "command", "args", "env", "cwd"],
        "streamable-http" | "sse" => &["type", "url", "headers"],
        other => return Err(format!("unknown type «{other}»")),
    };
    if let Some(key) = fields.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!("field «{key}» does not belong to this transport"));
    }
    if kind == "sse" {
        return Ok(None);
    }
    if kind == "streamable-http" {
        let url = fields.get("url").and_then(Value::as_str).ok_or("no url")?;
        check_url(url)?;
        let headers = strings(fields.get("headers"), "headers")?;
        let mut seen = HashSet::new();
        if headers.keys().any(|k| !seen.insert(k.to_ascii_lowercase())) {
            return Err("a header is repeated in a different case".into());
        }
        return Ok(Some(McpServerSettings {
            kind: Some("http".into()),
            url: Some(url.to_string()),
            headers,
            ..Default::default()
        }));
    }

    let command = fields.get("command").and_then(Value::as_str).ok_or("no command")?;
    let program = program(root, command)?;
    let args = match fields.get("args") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or("args must be a list of strings"))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err("args must be a list of strings".into()),
    };
    let env = strings(fields.get("env"), "env")?;
    if env.contains_key("PLUGIN_ROOT") || env.contains_key("PLUGIN_DATA") {
        return Err("env cannot set PLUGIN_ROOT and PLUGIN_DATA".into());
    }
    let cwd = match fields.get("cwd") {
        None => None,
        Some(Value::String(raw)) => Some(cwd(root, raw)?),
        Some(_) => return Err("cwd must be a string".into()),
    };
    Ok(Some(McpServerSettings {
        kind: Some("stdio".into()),
        command: Some(program),
        args,
        env,
        cwd,
        ..Default::default()
    }))
}

fn strings(value: Option<&Value>, what: &str) -> Result<BTreeMap<String, String>, String> {
    match value {
        None => Ok(BTreeMap::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())).ok_or(format!("{what}.{k} is not a string")))
            .collect(),
        Some(_) => Err(format!("{what} must be an object of strings")),
    }
}

/// §9: `command` is one token — a bare program for `PATH`, or `./` inside the
/// plugin, made absolute here because the process's working directory does not
/// decide where a relative program is found.
fn program(root: &Path, command: &str) -> Result<String, String> {
    if command.is_empty() || command.chars().any(char::is_whitespace) {
        return Err("command must be a single word".into());
    }
    if let Some(rel) = command.strip_prefix("./") {
        let path = contained(root, &root.join(rel)).ok_or("command points outside the plugin")?;
        return Ok(path.display().to_string());
    }
    if command.contains(['/', '\\']) {
        return Err("command is either a program name or a ./ path inside the plugin".into());
    }
    Ok(command.to_string())
}

/// §9's accepted forms only: `./path`, or a path under one of the two
/// variables. Written back in the `${PLUGIN_ROOT}` form, which the session
/// resolves against the plugin's root.
fn cwd(root: &Path, raw: &str) -> Result<String, String> {
    let (var, rest) = if let Some(rest) = raw.strip_prefix("${PLUGIN_ROOT}") {
        ("${PLUGIN_ROOT}", rest)
    } else if let Some(rest) = raw.strip_prefix("${PLUGIN_DATA}") {
        ("${PLUGIN_DATA}", rest)
    } else if let Some(rest) = raw.strip_prefix("./") {
        ("${PLUGIN_ROOT}", rest)
    } else {
        return Err(format!("cwd «{raw}» is not one of the allowed forms"));
    };
    let rest = rest.trim_start_matches('/');
    if var == "${PLUGIN_ROOT}" {
        // A directory that does not exist yet cannot be checked through links;
        // the lexical check still stops `..`.
        let path = root.join(rest);
        if rest.split('/').any(|p| p == "..") || (path.exists() && contained(root, &path).is_none()) {
            return Err("cwd points outside the plugin".into());
        }
    }
    Ok(if rest.is_empty() { var.to_string() } else { format!("{var}/{rest}") })
}

fn check_url(url: &str) -> Result<(), String> {
    let (scheme, rest) = url.split_once("://").ok_or("url is not absolute")?;
    if url.contains('#') {
        return Err("url with a fragment".into());
    }
    let authority = rest.split(['/', '?']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err("url with a login in the address".into());
    }
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
    };
    match scheme {
        "https" => Ok(()),
        "http" if host == "localhost" || host == "::1" || host.starts_with("127.") => Ok(()),
        "http" => Err("http is allowed only for localhost, otherwise https".into()),
        _ => Err("url is neither http nor https".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_rule() {
        assert!(is_plugin_name("my-plugin.v2"));
        for bad in ["", "-a", "a-", "a--b", "a..b", "A", "a_b", ".a"] {
            assert!(!is_plugin_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn plain_http_is_for_loopback_only() {
        assert!(check_url("https://example.com/mcp").is_ok());
        assert!(check_url("http://localhost:3000/mcp").is_ok());
        assert!(check_url("http://127.0.0.1/mcp").is_ok());
        assert!(check_url("http://[::1]:8080/mcp").is_ok());
        assert!(check_url("http://example.com/mcp").is_err());
        assert!(check_url("https://u:p@example.com/").is_err());
        assert!(check_url("https://example.com/#x").is_err());
        assert!(check_url("ftp://example.com").is_err());
    }
}

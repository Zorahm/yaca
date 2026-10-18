//! Where MCP servers are declared, and what one declaration means.
//!
//! Three places, least specific first, and a later one replaces an earlier one
//! of the same name whole:
//!
//! 1. `<config dir>/mcp.json` — the human's own servers, for every project.
//!    Application-level, so it reaches the core through [`SessionConfig`] the
//!    way the provider list does, and a test session has none.
//! 2. `<project>/.mcp.json` — the file other tools read. Read here so a
//!    project set up for one of them works without copying.
//! 3. `mcpServers` in `<project>/.yaca/settings.json`.
//!
//! All three use the same shape per server, the one `.mcp.json` has, so a
//! block can be moved between them unchanged.
//!
//! The second is the odd one out in trust. `.yaca/settings.json` and the
//! global file are written by the human or through the settings window;
//! `.mcp.json` arrives with a cloned repository, and a stdio server is a
//! command that runs the moment a session opens. So a server from that file
//! waits for the human's approval (`approval`), and the approval is bound to
//! the declaration's fingerprint: a `git pull` that changes the command asks
//! again.
//!
//! [`SessionConfig`]: crate::SessionConfig

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::app::AppLink;
use crate::error::CoreError;

/// The project's file, at its root.
pub const PROJECT_FILE: &str = ".mcp.json";
/// The human's file, in the configuration directory.
pub const GLOBAL_FILE: &str = "mcp.json";

/// One server, as any of the three files declares it.
///
/// `command` makes it a local process spoken to over stdio; `url` makes it a
/// remote one spoken to over Streamable HTTP. Exactly one of the two.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct McpServerSettings {
    /// The program to start. Run directly, not through a shell.
    pub command: Option<String>,
    pub args: Vec<String>,
    /// Added to the environment the server inherits from Yaca.
    pub env: BTreeMap<String, String>,
    /// Where the server starts. Absent means the project root; a relative
    /// path is relative to it.
    pub cwd: Option<String>,
    /// `stdio` or `http`. Optional: `command` and `url` already say which.
    /// Kept because `.mcp.json` files written for other tools carry it.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub url: Option<String>,
    /// Sent with every HTTP request, e.g. `Authorization`.
    pub headers: BTreeMap<String, String>,
    /// Declared but not connected.
    pub disabled: bool,
    /// Seconds the server has to start and list its tools. Absent means
    /// [`McpServerSettings::DEFAULT_STARTUP_SECS`].
    #[ts(type = "number | null")]
    pub startup_timeout_secs: Option<u64>,
    /// Seconds one tool call may take. Absent means
    /// [`McpServerSettings::DEFAULT_TOOL_SECS`].
    #[ts(type = "number | null")]
    pub tool_timeout_secs: Option<u64>,
}

impl McpServerSettings {
    /// Long enough for `npx` to fetch a package it has not cached; a session's
    /// first message waits for this at most.
    pub const DEFAULT_STARTUP_SECS: u64 = 30;
    pub const MAX_STARTUP_SECS: u64 = 600;
    /// The ceiling Bash puts on a foreground command, for the same reason: a
    /// call that runs longer is usually waiting on something.
    pub const DEFAULT_TOOL_SECS: u64 = 600;
    pub const MAX_TOOL_SECS: u64 = 3_600;

    pub fn startup_timeout(&self) -> Duration {
        Duration::from_secs(self.startup_timeout_secs.unwrap_or(Self::DEFAULT_STARTUP_SECS))
    }

    pub fn tool_timeout(&self) -> Duration {
        Duration::from_secs(self.tool_timeout_secs.unwrap_or(Self::DEFAULT_TOOL_SECS))
    }

    pub fn transport(&self) -> McpTransport {
        if self.url.is_some() && self.command.is_none() {
            McpTransport::Http
        } else {
            McpTransport::Stdio
        }
    }

    /// Why this declaration cannot be used, in words a human can act on.
    pub fn validate(&self) -> Result<(), String> {
        let command = self.command.as_deref().map(str::trim).unwrap_or("");
        let url = self.url.as_deref().map(str::trim).unwrap_or("");
        match (command.is_empty(), url.is_empty()) {
            (true, true) => return Err("не задано ни command, ни url".into()),
            (false, false) => return Err("заданы и command, и url: нужно что-то одно".into()),
            _ => {}
        }
        match self.kind.as_deref() {
            None => {}
            Some("stdio") if !command.is_empty() => {}
            Some("http" | "streamable-http" | "streamableHttp") if !url.is_empty() => {}
            // The transport before Streamable HTTP: two endpoints and a
            // different handshake. Named rather than tried, because trying
            // gives a 405 nobody can read anything from.
            Some("sse") => {
                return Err(
                    "транспорт «sse» устарел и не поддерживается: нужен сервер со Streamable HTTP"
                        .into(),
                )
            }
            Some(other) => return Err(format!("type «{other}» не подходит к этому серверу")),
        }
        let web = url.starts_with("http://") || url.starts_with("https://");
        if !url.is_empty() && !web {
            return Err(format!("url должен начинаться с http:// или https://, а не «{url}»"));
        }
        check_range("startupTimeoutSecs", self.startup_timeout_secs, Self::MAX_STARTUP_SECS)?;
        check_range("toolTimeoutSecs", self.tool_timeout_secs, Self::MAX_TOOL_SECS)
    }

    /// What identifies this declaration for an approval: any change to what
    /// would run or be reached changes it.
    pub(crate) fn fingerprint(&self) -> String {
        // A `BTreeMap` serializes in key order, so the text is the same for
        // the same declaration however the file ordered it.
        let text = serde_json::to_string(self).unwrap_or_default();
        blake3::hash(text.as_bytes()).to_hex().to_string()
    }

    /// The line a human recognises the server by: the command as written, or
    /// the address. Unexpanded on purpose — `${TOKEN}` stays `${TOKEN}`.
    pub(crate) fn target(&self) -> String {
        // A declaration is never of an application server: those have no
        // file to be declared in ([`ServerSpec::target`]).
        match self.transport() {
            McpTransport::Http => self.url.clone().unwrap_or_default(),
            McpTransport::Stdio | McpTransport::App => {
                let mut line = self.command.clone().unwrap_or_default();
                for arg in &self.args {
                    line.push(' ');
                    line.push_str(arg);
                }
                line
            }
        }
    }
}

fn check_range(key: &str, value: Option<u64>, max: u64) -> Result<(), String> {
    match value {
        Some(secs) if !(1..=max).contains(&secs) => {
            Err(format!("{key} должен быть от 1 до {max} с, а не {secs}"))
        }
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum McpTransport {
    Stdio,
    Http,
    /// A server in the application's own process ([`super::app`]).
    App,
}

/// Which file a server was declared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum McpSource {
    /// `<config dir>/mcp.json`.
    Global,
    /// `<project>/.mcp.json`.
    ProjectFile,
    /// `<project>/.yaca/settings.json`.
    Project,
    /// A plugin the human enabled (`plugin`). Enabling it was the consent, so
    /// its servers start without a second question, like the human's own.
    Plugin,
    /// The application itself ([`super::app`]): declared in no file, and
    /// trusted like the application is.
    App,
}

/// The shape of `mcp.json` and `.mcp.json`. Other keys are left alone: the
/// file may belong to another tool as much as to this one.
#[derive(Debug, Default, Serialize, Deserialize)]
struct McpFile {
    #[serde(default, rename = "mcpServers")]
    servers: BTreeMap<String, McpServerSettings>,
}

fn read(path: &Path) -> Result<BTreeMap<String, McpServerSettings>, CoreError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => {
            return Err(CoreError::Config(format!("не прочитать {}: {e}", path.display())))
        }
    };
    // Stops the session, like a broken `settings.json`: skipping the file
    // would leave a human waiting for tools that were quietly dropped.
    let file: McpFile = serde_json::from_str(&text).map_err(|e| {
        CoreError::Config(format!("{} разобран не до конца: {e}", path.display()))
    })?;
    Ok(file.servers)
}

/// The human's servers for every project.
pub fn load_global(config_dir: &Path) -> Result<BTreeMap<String, McpServerSettings>, CoreError> {
    read(&config_dir.join(GLOBAL_FILE))
}

/// Writes the human's servers. The settings window calls this; the core never
/// does on its own.
pub fn save_global(
    config_dir: &Path,
    servers: &BTreeMap<String, McpServerSettings>,
) -> Result<(), CoreError> {
    std::fs::create_dir_all(config_dir)
        .map_err(|e| CoreError::Config(format!("не создать {}: {e}", config_dir.display())))?;
    let file = McpFile {
        servers: servers.clone(),
    };
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| CoreError::Config(format!("не собрать список серверов: {e}")))?;
    std::fs::write(config_dir.join(GLOBAL_FILE), text)
        .map_err(|e| CoreError::Config(format!("не записать список серверов: {e}")))
}

/// The servers of `<project>/.mcp.json`.
pub fn load_project_file(root: &Path) -> Result<BTreeMap<String, McpServerSettings>, CoreError> {
    read(&root.join(PROJECT_FILE))
}

/// One server of a session, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    pub name: String,
    pub source: McpSource,
    pub settings: McpServerSettings,
    /// Set for a plugin's server: where it starts and what its variables are.
    pub plugin: Option<PluginLaunch>,
    /// Set for a server of the application: `settings` then says nothing
    /// about how to reach it.
    pub app: Option<AppLink>,
}

/// What a plugin's server is started with on top of its declaration.
///
/// The plugin's root replaces the project as the base of a relative `cwd` and
/// of a `./` program, and four variables name the plugin's directories — the
/// two of Agent Plugins §9 and Claude Code's spelling of the same two. They are
/// put in the server's environment and filled in where the declaration names
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLaunch {
    pub root: PathBuf,
    pub data: PathBuf,
    /// Whether other `${NAME}` are taken from the environment. Claude Code
    /// expands them; Agent Plugins leaves them as written.
    pub expand_env: bool,
}

impl PluginLaunch {
    fn var(&self, name: &str) -> Option<String> {
        match name {
            "PLUGIN_ROOT" | "CLAUDE_PLUGIN_ROOT" => Some(self.root.display().to_string()),
            "PLUGIN_DATA" | "CLAUDE_PLUGIN_DATA" => Some(self.data.display().to_string()),
            _ => None,
        }
    }

    fn vars(&self) -> [(String, String); 4] {
        let (root, data) = (self.root.display().to_string(), self.data.display().to_string());
        [
            ("PLUGIN_ROOT".into(), root.clone()),
            ("CLAUDE_PLUGIN_ROOT".into(), root),
            ("PLUGIN_DATA".into(), data.clone()),
            ("CLAUDE_PLUGIN_DATA".into(), data),
        ]
    }
}

/// The servers a session has, by name. A more specific file replaces a less
/// specific one's server of the same name whole: half of one declaration
/// under half of another is a server nobody wrote.
pub fn merge(
    global: &BTreeMap<String, McpServerSettings>,
    project_file: &BTreeMap<String, McpServerSettings>,
    project: &BTreeMap<String, McpServerSettings>,
) -> Vec<ServerSpec> {
    let mut all = BTreeMap::new();
    let layers = [
        (McpSource::Global, global),
        (McpSource::ProjectFile, project_file),
        (McpSource::Project, project),
    ];
    for (source, layer) in layers {
        for (name, settings) in layer {
            all.insert(
                name.clone(),
                ServerSpec {
                    name: name.clone(),
                    source,
                    settings: settings.clone(),
                    plugin: None,
                    app: None,
                },
            );
        }
    }
    all.into_values().collect()
}

/// How to reach a server, with every `${VAR}` filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Launch {
    Stdio {
        program: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        cwd: PathBuf,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
    },
    App(AppLink),
}

impl ServerSpec {
    /// Why the declaration cannot be used. A server of the application has
    /// nothing declared to get wrong.
    pub(crate) fn validate(&self) -> Result<(), String> {
        match self.app {
            Some(_) => Ok(()),
            None => self.settings.validate(),
        }
    }

    pub(crate) fn transport(&self) -> McpTransport {
        match self.app {
            Some(_) => McpTransport::App,
            None => self.settings.transport(),
        }
    }

    /// The line the panel shows for the server, see [`McpServerSettings::target`].
    pub(crate) fn target(&self) -> String {
        match &self.app {
            Some(app) => app.target.clone(),
            None => self.settings.target(),
        }
    }

    /// Resolves the declaration against the project root and the environment.
    pub(crate) fn launch(&self, root: &Path) -> Result<Launch, String> {
        self.launch_with(root, &|name| std::env::var(name).ok())
    }

    pub(crate) fn launch_with(
        &self,
        root: &Path,
        var: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Launch, String> {
        if let Some(app) = &self.app {
            return Ok(Launch::App(app.clone()));
        }
        let s = &self.settings;
        s.validate()?;
        let plugin = self.plugin.as_ref();
        let root = plugin.map_or(root, |p| p.root.as_path());
        let lookup = |name: &str| -> Lookup {
            match plugin {
                Some(p) => match p.var(name) {
                    Some(value) => Lookup::Value(value),
                    None if !p.expand_env => Lookup::Literal,
                    None => var(name).map_or(Lookup::Missing, Lookup::Value),
                },
                None => var(name).map_or(Lookup::Missing, Lookup::Value),
            }
        };
        let fill = |text: &str| expand_with(text, &lookup);
        let fill_map = |map: &BTreeMap<String, String>| {
            map.iter()
                .map(|(key, value)| Ok((key.clone(), fill(value)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()
        };

        if s.transport() == McpTransport::Http {
            return Ok(Launch::Http {
                url: fill(s.url.as_deref().unwrap_or_default().trim())?,
                headers: fill_map(&s.headers)?,
            });
        }
        let cwd = match s.cwd.as_deref().map(str::trim).filter(|cwd| !cwd.is_empty()) {
            // Joining an absolute path replaces the base, which is what an
            // absolute `cwd` means.
            Some(cwd) => root.join(fill(cwd)?),
            None => root.to_path_buf(),
        };
        let mut program = fill(s.command.as_deref().unwrap_or_default().trim())?;
        let mut env = fill_map(&s.env)?;
        if let Some(p) = plugin {
            // A program named relative to the plugin is found there, not
            // wherever Yaca happens to run from.
            if let Some(rel) = program.strip_prefix("./") {
                program = p.root.join(rel).display().to_string();
            }
            env.extend(p.vars());
            // §9: created before the server is launched.
            let _ = std::fs::create_dir_all(&p.data);
        }
        Ok(Launch::Stdio {
            program,
            args: s.args.iter().map(|arg| fill(arg)).collect::<Result<_, _>>()?,
            env,
            cwd,
        })
    }
}

/// [`expand_with`] over the environment alone.
#[cfg(test)]
fn expand(text: &str, var: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    expand_with(text, &|name| var(name).map_or(Lookup::Missing, Lookup::Value))
}

/// What a `${NAME}` stands for.
enum Lookup {
    Value(String),
    /// Not set, and an error unless the reference has a fallback.
    Missing,
    /// Not ours to expand: the reference stays as written.
    Literal,
}

/// Fills `${NAME}` and `${NAME:-fallback}`.
///
/// The spelling `.mcp.json` files already use, so a token stays in the
/// environment and out of a file that gets committed. A name with no value and
/// no fallback is an error rather than an empty string: a server started with
/// an empty token fails later and further from the cause.
fn expand_with(text: &str, var: &dyn Fn(&str) -> Lookup) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            // No closing brace: not a reference, and left as written.
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let inner = &after[..end];
        let (name, fallback) = match inner.split_once(":-") {
            Some((name, fallback)) => (name, Some(fallback)),
            None => (inner, None),
        };
        match var(name) {
            Lookup::Value(value) if !value.is_empty() => out.push_str(&value),
            Lookup::Literal => out.push_str(&rest[start..start + 2 + end + 1]),
            _ => match fallback {
                Some(fallback) => out.push_str(fallback),
                None => return Err(format!("переменная окружения {name} не задана")),
            },
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn stdio(command: &str) -> McpServerSettings {
        McpServerSettings {
            command: Some(command.into()),
            ..Default::default()
        }
    }

    fn http(url: &str) -> McpServerSettings {
        McpServerSettings {
            url: Some(url.into()),
            ..Default::default()
        }
    }

    fn one(name: &str, settings: McpServerSettings) -> BTreeMap<String, McpServerSettings> {
        BTreeMap::from([(name.to_string(), settings)])
    }

    #[test]
    fn a_file_written_for_another_tool_parses() {
        let dir = TempDir::new();
        dir.write(
            ".mcp.json",
            r#"{ "mcpServers": {
                "files": { "command": "npx", "args": ["-y", "server-files", "."], "env": { "A": "1" } },
                "tracker": { "type": "http", "url": "https://example.test/mcp", "headers": { "Authorization": "Bearer ${TOKEN}" } }
            }, "somethingElse": true }"#,
        );
        let servers = load_project_file(dir.path()).unwrap();
        assert_eq!(servers["files"].transport(), McpTransport::Stdio);
        assert_eq!(servers["files"].args, ["-y", "server-files", "."]);
        assert_eq!(servers["tracker"].transport(), McpTransport::Http);
        assert!(servers.values().all(|s| s.validate().is_ok()));
    }

    #[test]
    fn a_project_without_the_file_has_no_servers() {
        let dir = TempDir::new();
        assert!(load_project_file(dir.path()).unwrap().is_empty());
        assert!(load_global(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn a_broken_file_is_an_error_rather_than_no_servers() {
        let dir = TempDir::new();
        dir.write(".mcp.json", "{ mcpServers: ");
        assert!(matches!(load_project_file(dir.path()), Err(CoreError::Config(_))));
    }

    #[test]
    fn the_global_list_round_trips() {
        let dir = TempDir::new();
        let written = one("files", stdio("server-files"));
        save_global(dir.path(), &written).unwrap();
        assert_eq!(load_global(dir.path()).unwrap(), written);
    }

    #[test]
    fn the_more_specific_file_replaces_a_server_whole() {
        let global = one("a", stdio("from-global"));
        let file = BTreeMap::from([
            ("a".to_string(), stdio("from-file")),
            ("b".to_string(), stdio("only-file")),
        ]);
        let project = one("a", http("https://example.test/mcp"));

        let merged = merge(&global, &file, &project);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].source, McpSource::Project);
        // Nothing of the stdio declarations is left under the http one.
        assert_eq!(merged[0].settings.command, None);
        assert_eq!(merged[1].source, McpSource::ProjectFile);
    }

    #[test]
    fn a_declaration_needs_exactly_one_way_to_reach_the_server() {
        assert!(McpServerSettings::default().validate().is_err());
        let both = McpServerSettings {
            url: Some("https://example.test".into()),
            ..stdio("x")
        };
        assert!(both.validate().is_err());
        assert!(http("ftp://example.test").validate().is_err());
    }

    #[test]
    fn the_old_sse_transport_is_named_as_unsupported() {
        let old = McpServerSettings {
            kind: Some("sse".into()),
            ..http("https://example.test/sse")
        };
        assert!(old.validate().unwrap_err().contains("sse"));
    }

    #[test]
    fn timeouts_outside_their_range_are_refused() {
        for bad in [0, 601] {
            let s = McpServerSettings {
                startup_timeout_secs: Some(bad),
                ..stdio("x")
            };
            assert!(s.validate().is_err(), "{bad}");
        }
        let s = McpServerSettings {
            tool_timeout_secs: Some(3_601),
            ..stdio("x")
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn references_to_the_environment_are_filled_in() {
        let var = |name: &str| (name == "TOKEN").then(|| "s3cret".to_string());
        assert_eq!(expand("Bearer ${TOKEN}", &var).unwrap(), "Bearer s3cret");
        assert_eq!(expand("${HOST:-localhost}:80", &var).unwrap(), "localhost:80");
        assert_eq!(expand("plain $HOME ${", &var).unwrap(), "plain $HOME ${");
        assert!(expand("${MISSING}", &var).unwrap_err().contains("MISSING"));
    }

    #[test]
    fn a_stdio_server_starts_in_the_project_unless_told_otherwise() {
        let root = Path::new("/p");
        let spec = |settings| ServerSpec {
            plugin: None,
            app: None,
            name: "s".into(),
            source: McpSource::Project,
            settings,
        };
        let none = |_: &str| None;

        let Launch::Stdio { cwd, .. } = spec(stdio("x")).launch_with(root, &none).unwrap() else {
            panic!("stdio expected");
        };
        assert_eq!(cwd, root);

        let nested = McpServerSettings {
            cwd: Some("tools".into()),
            ..stdio("x")
        };
        let Launch::Stdio { cwd, .. } = spec(nested).launch_with(root, &none).unwrap() else {
            panic!("stdio expected");
        };
        assert_eq!(cwd, root.join("tools"));
    }
}

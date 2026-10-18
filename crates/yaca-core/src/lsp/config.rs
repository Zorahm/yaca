//! Which language servers a session may use, and for which files.
//!
//! Two sources, and only two. A built-in table of well-known servers, each
//! used only when its program is found on `PATH`; and `lsp.json` in the
//! application's configuration directory, the human's own list. A project file
//! is deliberately not a source: a stdio server is a command, a repository
//! that ships one would run it on whoever opens it, and `.mcp.json` needs an
//! approval flow for exactly that reason. Until there is one, projects do not
//! get to declare servers.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;

/// The file in the configuration directory.
pub const GLOBAL_FILE: &str = "lsp.json";

/// `"lspServers": { "<name>": { ... } }` in [`GLOBAL_FILE`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct LspServerSettings {
    /// The program, started directly and not through a shell.
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// File extensions the server answers for, without the dot. Empty for a
    /// name that is also a built-in one means the built-in list.
    pub extensions: Vec<String>,
    /// Declared but never started; also how a built-in server is switched off.
    pub disabled: bool,
}

#[derive(Deserialize, Serialize, Default)]
struct LspFile {
    #[serde(default, rename = "lspServers")]
    servers: BTreeMap<String, LspServerSettings>,
}

/// One server of a session, ready to be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSpec {
    pub name: String,
    pub settings: LspServerSettings,
    /// Came from the built-in table, not from the human's file.
    pub builtin: bool,
    /// Why it cannot be started, found when the declaration was read.
    pub invalid: Option<String>,
}

impl ServerSpec {
    pub fn handles(&self, extension: &str) -> bool {
        self.invalid.is_none()
            && !self.settings.disabled
            && self.settings.extensions.iter().any(|e| e.eq_ignore_ascii_case(extension))
    }
}

/// (name, program, arguments, extensions)
const BUILTIN: &[(&str, &str, &[&str], &[&str])] = &[
    ("rust-analyzer", "rust-analyzer", &[], &["rs"]),
    (
        "typescript-language-server",
        "typescript-language-server",
        &["--stdio"],
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
    ),
    ("pyright", "pyright-langserver", &["--stdio"], &["py", "pyi"]),
    ("gopls", "gopls", &[], &["go"]),
];

fn builtin(name: &str) -> Option<LspServerSettings> {
    BUILTIN.iter().find(|(n, ..)| *n == name).map(|(_, program, args, extensions)| {
        LspServerSettings {
            command: (*program).to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
            env: BTreeMap::new(),
            extensions: extensions.iter().map(|e| (*e).to_string()).collect(),
            disabled: false,
        }
    })
}

/// The human's servers.
pub fn load_global(config_dir: &Path) -> Result<BTreeMap<String, LspServerSettings>, CoreError> {
    let path = config_dir.join(GLOBAL_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(CoreError::Config(format!("failed to read {}: {e}", path.display()))),
    };
    if text.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str::<LspFile>(&text)
        .map(|file| file.servers)
        .map_err(|e| CoreError::Config(format!("{}: {e}", path.display())))
}

/// The servers of a session: built-in ones whose program `on_path` finds,
/// then the human's, which replace a built-in of the same name whole.
///
/// The human's servers are kept even when their program is missing — they
/// were asked for, and a failure with a reason is better than a server that
/// silently is not there.
pub fn resolve(
    global: &BTreeMap<String, LspServerSettings>,
    on_path: impl Fn(&str) -> bool,
) -> Vec<ServerSpec> {
    let mut out: Vec<ServerSpec> = BUILTIN
        .iter()
        .filter(|(name, program, ..)| !global.contains_key(*name) && on_path(program))
        .filter_map(|(name, ..)| {
            builtin(name).map(|settings| ServerSpec {
                name: (*name).to_string(),
                settings,
                builtin: true,
                invalid: None,
            })
        })
        .collect();

    for (name, declared) in global {
        let mut settings = declared.clone();
        let own = builtin(name);
        if let Some(own) = &own {
            if settings.command.trim().is_empty() {
                settings.command = own.command.clone();
                if settings.args.is_empty() {
                    settings.args = own.args.clone();
                }
            }
            if settings.extensions.is_empty() {
                settings.extensions = own.extensions.clone();
            }
        }
        settings.extensions = settings
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty())
            .collect();

        let invalid = if settings.command.trim().is_empty() {
            Some("command is required".to_string())
        } else if settings.extensions.is_empty() {
            Some("extensions are required: unclear which files the server handles".to_string())
        } else {
            None
        };
        out.push(ServerSpec {
            name: name.clone(),
            settings,
            builtin: own.is_some(),
            invalid,
        });
    }
    out
}

/// Whether `program` is a file some `PATH` directory holds, with the
/// extensions Windows would try.
pub fn on_path(program: &str) -> bool {
    let given = Path::new(program);
    if given.components().count() > 1 {
        return given.is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(str::to_string)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).any(|dir| {
        extensions
            .iter()
            .any(|ext| dir.join(format!("{program}{ext}")).is_file())
    })
}

/// The `languageId` a server is told about a file by.
pub(crate) fn language_id(extension: &str) -> String {
    match extension.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "py" | "pyi" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "java" => "java",
        "json" => "json",
        "rb" => "ruby",
        other => return other.to_string(),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn declared(command: &str, extensions: &[&str]) -> LspServerSettings {
        LspServerSettings {
            command: command.into(),
            extensions: extensions.iter().map(|e| (*e).into()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_builtin_server_is_used_only_when_its_program_is_found() {
        let found = resolve(&BTreeMap::new(), |p| p == "gopls");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "gopls");
        assert!(found[0].builtin && found[0].handles("go"));
        assert!(resolve(&BTreeMap::new(), |_| false).is_empty());
    }

    #[test]
    fn the_humans_server_replaces_a_builtin_of_the_same_name_whole() {
        let global = BTreeMap::from([("gopls".to_string(), declared("/opt/gopls", &["go", "mod"]))]);
        let specs = resolve(&global, |_| true);
        let gopls: Vec<_> = specs.iter().filter(|s| s.name == "gopls").collect();
        assert_eq!(gopls.len(), 1);
        assert_eq!(gopls[0].settings.command, "/opt/gopls");
        assert!(gopls[0].handles("mod"));
    }

    #[test]
    fn a_builtin_name_with_only_disabled_set_keeps_the_builtins_answer_list() {
        let global = BTreeMap::from([(
            "gopls".to_string(),
            LspServerSettings { disabled: true, ..Default::default() },
        )]);
        let specs = resolve(&global, |_| false);
        assert_eq!(specs.len(), 1);
        assert!(specs[0].invalid.is_none());
        assert!(!specs[0].handles("go"), "disabled answers for nothing");
    }

    #[test]
    fn a_server_without_extensions_is_listed_as_unusable_with_the_reason() {
        let global = BTreeMap::from([("mine".to_string(), declared("mine-ls", &[]))]);
        let specs = resolve(&global, |_| false);
        assert!(specs[0].invalid.as_deref().unwrap().contains("extensions"));
        assert!(!specs[0].handles("x"));
    }

    #[test]
    fn extensions_are_taken_without_a_dot_and_in_lower_case() {
        let global = BTreeMap::from([("mine".to_string(), declared("mine-ls", &[".RS", "Toml"]))]);
        let specs = resolve(&global, |_| false);
        assert_eq!(specs[0].settings.extensions, ["rs", "toml"]);
        assert!(specs[0].handles("RS"));
    }

    #[test]
    fn the_file_is_read_by_name_and_a_missing_one_is_no_servers() {
        let dir = TempDir::new();
        assert!(load_global(dir.path()).unwrap().is_empty());
        dir.write(
            "lsp.json",
            r#"{ "lspServers": { "mine": { "command": "x", "args": ["--stdio"], "extensions": ["zz"] } } }"#,
        );
        let got = load_global(dir.path()).unwrap();
        assert_eq!(got["mine"].args, ["--stdio"]);
        dir.write("lsp.json", "{ lspServers: ");
        assert!(matches!(load_global(dir.path()), Err(CoreError::Config(_))));
    }
}

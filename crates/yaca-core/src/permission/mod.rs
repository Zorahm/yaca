//! Permissions (§8) and the permission request (§5.7).
//!
//! Two axes that must not be confused. *Permissions* limit the human: they say
//! which classes of action Yaca may take without asking. *Invariants* — read
//! before edit, file freshness, anchor uniqueness, workspace bounds — protect
//! against the model hallucinating, and they have no switch at all. Bypass
//! turns off the first; it never touches the second.
//!
//! What the core sends is structured, not prose. §5.7 fixes the wording of
//! every question ("Allow Yaca to delete the file?"), and that wording is
//! interface copy: the core names the action and the target, the feed writes
//! the sentence. Same rule as §5.4.1 — the interface writes what it knows.
//!
//! Two things here are not in the spec and are marked as such below:
//! [`ScopeKey::Domain`] and [`ScopeKey::McpServer`]. §5.7 lists three scopes
//! behind "Always allow"; a network permission fits none of them, and a call
//! to an MCP server fits only the widest.

pub mod broker;
mod cmdline;
pub mod invariant;
pub mod policy;
pub mod readonly;
pub mod unsaved;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CallId, RequestId};
use crate::workspace::roots;

/// What is being asked for. The feed maps this onto the wording table in §5.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    RunCommand,
    DeleteFile,
    DeleteDirectory,
    MoveFile,
    /// Asked in Manual only: an edit reaches the working tree at once, and
    /// this question is the one confirmation there is (§8, §16.37).
    EditFile,
    FetchUrl,
    McpWrite,
}

impl PermissionKind {
    /// Which row of the §8 trust matrix this action belongs to.
    pub fn class(self) -> ActionClass {
        match self {
            PermissionKind::RunCommand => ActionClass::Command,
            PermissionKind::DeleteFile
            | PermissionKind::DeleteDirectory
            | PermissionKind::MoveFile => ActionClass::Destructive,
            PermissionKind::EditFile => ActionClass::Edit,
            PermissionKind::FetchUrl | PermissionKind::McpWrite => ActionClass::Reach,
        }
    }
}

/// Rows of the trust matrix (§8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionClass {
    /// Deletion, moving.
    Destructive,
    /// Running commands. Asked in every mode, including Auto.
    Command,
    /// Network, writing through MCP.
    Reach,
    /// Writing a file. Manual asks; the other two do not (§8).
    Edit,
}

/// How far an "Always allow" grant or an "Always deny" refusal reaches (§5.7).
///
/// Session-lived by design. Writing rules into `.yaca/settings.json` happens
/// through the settings screen and nowhere else: a rule granted in a hurry
/// should not outlive the hurry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScopeKey {
    /// Every call of this tool.
    #[serde(rename_all = "camelCase")]
    Tool { name: String },
    /// Anything under this directory.
    #[serde(rename_all = "camelCase")]
    Directory { path: String },
    /// Commands starting with this word.
    #[serde(rename_all = "camelCase")]
    CommandPrefix { prefix: String },
    /// Any page on this host.
    ///
    /// Not in §5.7, which lists three scopes. Added because none of them fits
    /// a network permission: "every call of web_fetch" hands over the whole
    /// web, while the question that was asked was about one domain.
    #[serde(rename_all = "camelCase")]
    Domain { host: String },
    /// Every tool of this MCP server.
    ///
    /// Not in §5.7 either. A server's tools are named one by one, so "every
    /// call of this tool" is the narrow rule, and without this one the next
    /// step up would be answering the same question for each of a server's
    /// twenty tools.
    #[serde(rename_all = "camelCase")]
    McpServer { server: String },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ScopeKeyDe {
    #[serde(rename_all = "camelCase")]
    Tool { name: String },
    #[serde(rename_all = "camelCase")]
    Directory { path: String },
    #[serde(rename_all = "camelCase")]
    CommandPrefix { prefix: String },
    #[serde(rename_all = "camelCase")]
    Domain { host: String },
    #[serde(rename_all = "camelCase")]
    McpServer { server: String },
}

impl<'de> Deserialize<'de> for ScopeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = ScopeKeyDe::deserialize(deserializer)?;
        Ok(match raw {
            ScopeKeyDe::Tool { name } => ScopeKey::Tool {
                name: crate::legacy::canonical_tool_name(&name).to_string(),
            },
            ScopeKeyDe::Directory { path } => ScopeKey::Directory { path },
            ScopeKeyDe::CommandPrefix { prefix } => ScopeKey::CommandPrefix { prefix },
            ScopeKeyDe::Domain { host } => ScopeKey::Domain { host },
            ScopeKeyDe::McpServer { server } => ScopeKey::McpServer { server },
        })
    }
}

/// A standing "no" and the note that goes to the agent with every call it
/// refuses (§5.7, §10).
///
/// Next to [`ScopeKey`] rather than inside the broker because it is no longer
/// only a session thing: `.yaca/settings.json` keeps a list of these, and the
/// settings window reads them. One shape for both, so the file and the broker
/// cannot disagree about what a refusal is.
///
/// The reason is optional and travels with the refusal: the agent is told
/// *why* it may not do this, which is the difference between a rule and a
/// wall it keeps walking into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    pub scope: ScopeKey,
    /// Written as `null` when there is none, like every other optional field
    /// of the settings file. `skip_serializing_if` would read better by hand
    /// but ts-rs cannot parse that attribute, and a build warning is a worse
    /// price than a null.
    pub reason: Option<String>,
}

impl Refusal {
    pub fn new(scope: ScopeKey, reason: Option<String>) -> Self {
        Refusal { scope, reason }
    }
}

/// A question that no standing rule and no bypass can settle for the human (§8).
///
/// The invariants of §8 that no rule settles: a commit or a push needs an
/// explicit command from the human, and a write into git's configuration runs
/// programs when git next reads it. The core cannot know whether the human
/// gave a command, or whether a configuration is theirs to change, so every
/// such call is put to them. No "always" is offered for these questions, and a
/// grant that matches one is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Invariant {
    /// `git commit`, however it is spelled on the command line.
    GitCommit,
    /// `git push`.
    GitPush,
    /// A git command whose subcommand the shell produces at run time, so it may
    /// be a commit or a push. Asked about rather than guessed at.
    GitUnresolved,
    /// A write, edit, delete or move that touches git's own configuration or
    /// attributes: anything under a `.git` directory or `.git` file (a
    /// worktree's or submodule's pointer), and any `.gitattributes` or
    /// `.gitmodules` file. Git runs what those files name (`core.fsmonitor`,
    /// diff and filter drivers, hooks) the next time it reads the repository,
    /// so the write is the moment to ask: a later `git status` would run it
    /// unasked. Asked in every mode, bypass included, as a push is.
    GitConfig,
}

/// What a deletion of a directory takes away that a rollback cannot bring back
/// (§5.7, §5.8).
///
/// The shadow repository keeps neither the directories it skips (build output,
/// caches) nor the files it cannot track: over the size limit, links, and names
/// it cannot spell. A rollback restores what it saved; these it never saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Unsaved {
    /// Project-relative paths of the skipped directories: the first few.
    pub dirs: Vec<String>,
    /// How many skipped directories there are in all. A number, not a list of
    /// everything, because a monorepo can have hundreds of `node_modules`.
    pub dir_count: u32,
    /// Files the snapshot does not keep.
    pub files: u32,
    /// Additional roots the action reaches. The checkpoints snapshot the primary
    /// root only, so nothing in these comes back on a rollback, whatever the
    /// action was. Plain absolute paths (`Workspace::additional_text`).
    ///
    /// `default`: a journal written before the field existed replays too.
    #[serde(default)]
    pub roots: Vec<String>,
}

/// Files inside a directory, counted for the question about it (§5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileCount {
    pub files: u32,
    /// Counting stopped at `files` and there are more. The question waits for
    /// the count, and a directory the size of `node_modules` would hold it up
    /// for seconds to say a number nobody reads to the last digit.
    pub more: bool,
}

/// A live question shown as an inline block over the feed (§5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: RequestId,
    /// Ties the block to the action row it belongs to.
    pub call_id: CallId,
    pub kind: PermissionKind,
    /// Tier two: the target in short — file name, first word of the command,
    /// domain.
    pub target: String,
    /// Tier three, monospaced and never truncated. This is where a human
    /// spots `&& rm -rf`, so it wraps or scrolls but is never cut.
    pub detail: String,
    /// The dimmed second line under the detail (§5.7). A path, never
    /// prose: the working directory a command would run in.
    pub note: Option<String>,
    /// Size of the file the action touches. A number, not a string:
    /// "1.2 KB" is copy, and copy belongs to the feed.
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    /// How many files deleting a directory takes with it (§5.7). A number for
    /// the same reason as the size.
    ///
    /// `default`: journals written before the field existed replay too (§9).
    #[serde(default)]
    pub files: Option<FileCount>,
    /// Set when the question is an invariant (§8): the feed says why it is
    /// asked at all, since no rule could have answered it. `default` for the
    /// same reason as `files`.
    #[serde(default)]
    pub invariant: Option<Invariant>,
    /// Set when deleting the directory loses what no snapshot keeps (§5.7).
    /// The feed names what will not come back.
    #[serde(default)]
    pub unsaved: Option<Unsaved>,
    /// The MCP server the call goes to (§5.7: "изменить данные в Asana"). The
    /// feed names it in the question. `default` for the same reason as `files`.
    #[serde(default)]
    pub server: Option<String>,
    /// Choices offered behind "Always allow" and "Always deny" alike. Empty for
    /// an invariant: there is no rule the human could grant that would apply.
    pub scopes: Vec<ScopeKey>,
    /// `2 of 3` in the corner of the block.
    pub position: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionAnswer {
    /// Just this once.
    Allow,
    /// This once and every matching call for the rest of the session.
    #[serde(rename_all = "camelCase")]
    AllowAlways { scope: ScopeKey },
    /// Refused. The note travels to the agent along with the refusal —
    /// without it the agent simply tries again (§5.7).
    #[serde(rename_all = "camelCase")]
    Deny { reason: Option<String> },
    /// Refused now and for every matching call for the rest of the session.
    /// Later calls are refused without a question, each with this note.
    ///
    /// The pair of `AllowAlways`, which §8 had only one half of: a model that
    /// keeps reaching for a refused action otherwise asks about it again and
    /// again, one block per attempt.
    #[serde(rename_all = "camelCase")]
    DenyAlways {
        scope: ScopeKey,
        reason: Option<String>,
    },
}

/// What a tool declares it is about to do. Built before the tool runs, so the
/// question can be answered before anything happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionNeed {
    pub kind: PermissionKind,
    pub tool: String,
    pub target: String,
    pub detail: String,
    pub note: Option<String>,
    pub size_bytes: Option<u64>,
    pub files: Option<FileCount>,
    /// Absolute path the action touches. Feeds directory-scoped rules.
    pub path: Option<PathBuf>,
    /// Full command line, when there is one. Feeds prefix-scoped rules.
    pub command: Option<String>,
    /// Host the action reaches. Feeds domain-scoped rules.
    pub domain: Option<String>,
    /// MCP server the call goes to. Feeds server-scoped rules.
    pub server: Option<String>,
    /// The call is an invariant (§8): asked in every mode, bypass included,
    /// and no grant answers it.
    pub invariant: Option<Invariant>,
    /// Deleting this loses what no snapshot keeps. Asked in Auto as well.
    pub unsaved: Option<Unsaved>,
}

impl PermissionNeed {
    pub fn new(kind: PermissionKind, tool: &str, target: impl Into<String>) -> Self {
        PermissionNeed {
            kind,
            tool: tool.to_string(),
            target: target.into(),
            detail: String::new(),
            note: None,
            size_bytes: None,
            files: None,
            path: None,
            command: None,
            domain: None,
            server: None,
            invariant: None,
            unsaved: None,
        }
    }

    pub fn server(mut self, server: impl Into<String>) -> Self {
        self.server = Some(server.into());
        self
    }

    pub fn invariant(mut self, invariant: Invariant) -> Self {
        self.invariant = Some(invariant);
        self
    }

    pub fn unsaved(mut self, unsaved: Unsaved) -> Self {
        self.unsaved = Some(unsaved);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn domain(mut self, host: impl Into<String>) -> Self {
        self.domain = Some(host.into());
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn size_bytes(mut self, size: u64) -> Self {
        self.size_bytes = Some(size);
        self
    }

    pub fn files(mut self, count: FileCount) -> Self {
        self.files = Some(count);
        self
    }

    /// Scopes to offer behind "Always allow", narrowest first.
    ///
    /// The narrow option comes first on purpose: the list is answered by
    /// number, and the digit that gets pressed without reading should be
    /// the one that grants least.
    ///
    /// `root` is the **primary** root, and that is deliberate. A directory
    /// scope inside it is stored relative to it, and a relative rule is only
    /// ever resolved against the primary root (see [`covered_by`]). A
    /// directory in an additional root is stored absolute, so a rule granted
    /// for `src` of the project can never match `src` of another root.
    ///
    /// [`covered_by`]: PermissionNeed::covered_by
    pub fn scopes(&self, root: &Path) -> Vec<ScopeKey> {
        // An invariant is answered for this call only. Offering a rule here
        // would promise a grant that the gate is built to ignore.
        if self.invariant.is_some() {
            return Vec::new();
        }
        let mut out = Vec::new();

        if let Some(command) = &self.command {
            if let Some(word) = command.split_whitespace().next() {
                out.push(ScopeKey::CommandPrefix {
                    prefix: word.to_string(),
                });
            }
        }

        if let Some(host) = &self.domain {
            out.push(ScopeKey::Domain { host: host.clone() });
        }

        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                out.push(ScopeKey::Directory {
                    path: display_rel(root, parent),
                });
            }
        }

        out.push(ScopeKey::Tool {
            name: self.tool.clone(),
        });
        // After the tool, not before: for an MCP call the tool is the narrow
        // rule and the server the wide one.
        if let Some(server) = &self.server {
            out.push(ScopeKey::McpServer {
                server: server.clone(),
            });
        }
        out
    }

    /// Does an already granted rule cover this call?
    pub fn covered_by(&self, scope: &ScopeKey, root: &Path) -> bool {
        match scope {
            ScopeKey::Tool { name } => {
                crate::legacy::canonical_tool_name(name)
                    == crate::legacy::canonical_tool_name(&self.tool)
            }
            // `root` is the primary root: a relative rule means a directory of
            // the project and never of another root. An absolute rule (an
            // additional root's directory) is joined as it is, because joining
            // an absolute path replaces the base.
            ScopeKey::Directory { path } => self.path.as_ref().is_some_and(|touched| {
                let dir = if path.is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(path)
                };
                let dir = roots::in_root_form(&dir, touched);
                touched.starts_with(&dir)
            }),
            ScopeKey::CommandPrefix { prefix } => self
                .command
                .as_deref()
                .and_then(|c| c.split_whitespace().next())
                .is_some_and(|word| word == prefix),
            // Exact host, never a suffix: a rule for `docs.rs` that also
            // covered `evil-docs.rs` would grant what was never asked.
            ScopeKey::Domain { host } => self.domain.as_deref() == Some(host.as_str()),
            ScopeKey::McpServer { server } => self.server.as_deref() == Some(server.as_str()),
        }
    }
}

/// A directory as a scope stores it: relative to the primary root, or, outside
/// it, absolute (`roots::plain`).
fn display_rel(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => roots::plain(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/p")
    }

    fn delete(path: &str) -> PermissionNeed {
        PermissionNeed::new(PermissionKind::DeleteFile, "Delete", path).path(root().join(path))
    }

    #[test]
    fn narrow_scope_is_offered_first() {
        let scopes = delete("src/legacy/old.rs").scopes(&root());
        assert_eq!(
            scopes,
            vec![
                ScopeKey::Directory {
                    path: "src/legacy".into()
                },
                ScopeKey::Tool {
                    name: "Delete".into()
                },
            ]
        );
    }

    #[test]
    fn command_scope_is_the_first_word_only() {
        let need = PermissionNeed::new(PermissionKind::RunCommand, "Bash", "npm")
            .command("npm ci --prefer-offline");
        assert_eq!(
            need.scopes(&root())[0],
            ScopeKey::CommandPrefix {
                prefix: "npm".into()
            }
        );
    }

    #[test]
    fn directory_rule_covers_files_below_it() {
        let rule = ScopeKey::Directory { path: "src".into() };
        assert!(delete("src/deep/a.rs").covered_by(&rule, &root()));
        assert!(!delete("tests/a.rs").covered_by(&rule, &root()));
    }

    #[test]
    fn tool_rule_covers_every_call_of_that_tool() {
        let rule = ScopeKey::Tool {
            name: "Delete".into(),
        };
        assert!(delete("anywhere.rs").covered_by(&rule, &root()));

        let other = PermissionNeed::new(PermissionKind::MoveFile, "Move", "a.rs");
        assert!(!other.covered_by(&rule, &root()));
    }

    #[test]
    fn old_tool_scope_covers_renamed_tool() {
        // A standing rule written with the old name "delete_file" still covers "Delete".
        let rule = ScopeKey::Tool {
            name: "delete_file".into(),
        };
        assert!(delete("anywhere.rs").covered_by(&rule, &root()));
    }

    #[test]
    fn old_tool_scope_name_is_canonicalized_on_deserialize() {
        let json = r#"{"type":"tool","name":"delete_file"}"#;
        let scope: ScopeKey = serde_json::from_str(json).unwrap();
        assert_eq!(
            scope,
            ScopeKey::Tool {
                name: "Delete".into()
            }
        );
    }

    #[test]
    fn prefix_rule_does_not_leak_to_a_longer_word() {
        let rule = ScopeKey::CommandPrefix {
            prefix: "npm".into(),
        };
        let npm =
            PermissionNeed::new(PermissionKind::RunCommand, "Bash", "npm").command("npm test");
        let other =
            PermissionNeed::new(PermissionKind::RunCommand, "Bash", "npmx").command("npmx test");
        assert!(npm.covered_by(&rule, &root()));
        assert!(!other.covered_by(&rule, &root()));
    }

    #[test]
    fn a_domain_rule_covers_that_host_and_no_other() {
        let rule = ScopeKey::Domain {
            host: "docs.rs".into(),
        };
        let fetch = |host: &str| {
            PermissionNeed::new(PermissionKind::FetchUrl, "WebFetch", host).domain(host)
        };

        assert!(fetch("docs.rs").covered_by(&rule, &root()));
        // Neither a subdomain nor a name that merely ends the same way.
        assert!(!fetch("static.docs.rs").covered_by(&rule, &root()));
        assert!(!fetch("evil-docs.rs").covered_by(&rule, &root()));
    }

    #[test]
    fn a_domain_rule_does_not_cover_a_different_action() {
        let rule = ScopeKey::Domain {
            host: "docs.rs".into(),
        };
        // A deletion carries no domain, so a network rule can never match it.
        assert!(!delete("a.rs").covered_by(&rule, &root()));
    }

    #[test]
    fn an_mcp_call_offers_its_tool_and_then_its_server() {
        let call = |server: &str, tool: &str| {
            PermissionNeed::new(PermissionKind::McpWrite, &format!("mcp__{server}__{tool}"), tool)
                .server(server)
        };
        let scopes = call("tracker", "create_task").scopes(&root());
        assert_eq!(
            scopes,
            vec![
                ScopeKey::Tool {
                    name: "mcp__tracker__create_task".into()
                },
                ScopeKey::McpServer {
                    server: "tracker".into()
                },
            ]
        );

        // The server rule covers its other tools, and nobody else's.
        assert!(call("tracker", "close_task").covered_by(&scopes[1], &root()));
        assert!(!call("files", "write").covered_by(&scopes[1], &root()));
        assert!(!call("tracker", "close_task").covered_by(&scopes[0], &root()));
        assert!(!delete("a.rs").covered_by(&scopes[1], &root()));
    }

    #[test]
    fn kinds_land_on_the_right_matrix_row() {
        assert_eq!(PermissionKind::DeleteFile.class(), ActionClass::Destructive);
        assert_eq!(PermissionKind::MoveFile.class(), ActionClass::Destructive);
        assert_eq!(PermissionKind::RunCommand.class(), ActionClass::Command);
        assert_eq!(PermissionKind::McpWrite.class(), ActionClass::Reach);
        assert_eq!(PermissionKind::FetchUrl.class(), ActionClass::Reach);
        assert_eq!(PermissionKind::EditFile.class(), ActionClass::Edit);
    }
}

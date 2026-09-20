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
//! One thing here is not in the spec and is marked as such below:
//! [`ScopeKey::Domain`]. §5.7 lists three scopes behind "Always allow" and a
//! network permission fits none of them.

pub mod broker;
pub mod policy;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::id::{CallId, RequestId};

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
        })
    }
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
    /// Choices offered behind "Always allow" and "Always deny" alike.
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
        }
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
    pub fn scopes(&self, root: &Path) -> Vec<ScopeKey> {
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
        out
    }

    /// Does an already granted rule cover this call?
    pub fn covered_by(&self, scope: &ScopeKey, root: &Path) -> bool {
        match scope {
            ScopeKey::Tool { name } => {
                crate::legacy::canonical_tool_name(name)
                    == crate::legacy::canonical_tool_name(&self.tool)
            }
            ScopeKey::Directory { path } => self.path.as_ref().is_some_and(|touched| {
                let dir = if path.is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(path)
                };
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
        }
    }
}

fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
    fn kinds_land_on_the_right_matrix_row() {
        assert_eq!(PermissionKind::DeleteFile.class(), ActionClass::Destructive);
        assert_eq!(PermissionKind::MoveFile.class(), ActionClass::Destructive);
        assert_eq!(PermissionKind::RunCommand.class(), ActionClass::Command);
        assert_eq!(PermissionKind::McpWrite.class(), ActionClass::Reach);
        assert_eq!(PermissionKind::FetchUrl.class(), ActionClass::Reach);
        assert_eq!(PermissionKind::EditFile.class(), ActionClass::Edit);
    }
}

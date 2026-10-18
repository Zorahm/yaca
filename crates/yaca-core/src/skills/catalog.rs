//! Finding skills: every root scanned in order, the first of a name kept.
//!
//! The order is the precedence. Project roots come before the human's own, so a
//! project's `release` shadows the human's `release`, the convention every
//! client follows. Within a scope `.yaca` comes before `.agents` before
//! `.claude`: the client's own place wins over the shared ones.
//!
//! Lenient the way the Agent Skills client guide asks, because most skills on a
//! machine were written for some other client. A cosmetic problem — a name that
//! breaks the naming rule, a header `name` that disagrees with its directory, a
//! description past the limit — is reported and the skill loads anyway. What
//! makes a skill unusable is reported and the skill is left out: no header, a
//! header that is not YAML, no description (the catalog line is the whole of
//! what the model decides by), a name nothing can be typed as.
//!
//! Every report is a [`Diagnostic`], in words for the human, so that the CLI and
//! the settings window can say why a skill they expected is missing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::frontmatter;
use crate::tool::memory::store::head;
use crate::workspace::roots::plain;

/// The file that makes a directory a skill.
pub const FILE: &str = "SKILL.md";

/// The catalog goes into every request, so it is capped. Room for a few dozen
/// skills with full descriptions; past that the list says it was cut.
pub const MAX_LIST_BYTES: usize = 12 * 1024;
/// The Agent Skills limit on a description. A longer one is cut in the catalog;
/// the skill's own file still holds all of it.
pub const MAX_DESCRIPTION_CHARS: usize = 1024;
/// The most of a skill file read to find its header. A header longer than this
/// reads as unclosed, which costs the skill, and is reported.
const MAX_HEADER_BYTES: usize = 16 * 1024;
/// The Agent Skills limit on a name. Longer is reported and loaded anyway, up
/// to [`MAX_USABLE_NAME_CHARS`].
const MAX_NAME_CHARS: usize = 64;
const MAX_USABLE_NAME_CHARS: usize = 128;

/// Where a skill was found. Project skills come with the repository; the
/// human's own are theirs on every project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SkillScope {
    Project,
    User,
    /// From an enabled plugin, named `<plugin>:<skill>`.
    Plugin,
}

/// One skill the catalog knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    /// One line, whitespace collapsed, cut to [`MAX_DESCRIPTION_CHARS`].
    pub description: String,
    /// The skill's directory, canonical. Relative paths in its body resolve here.
    pub dir: PathBuf,
    pub scope: SkillScope,
    /// Listed in the catalog the model reads.
    pub model_invocable: bool,
    /// Offered in the slash menu.
    pub user_invocable: bool,
    /// The root of the plugin the skill came with, which its body may name as
    /// `${CLAUDE_PLUGIN_ROOT}`.
    pub plugin_root: Option<PathBuf>,
    /// `context: fork`: run in a subagent, which reports back.
    pub fork: bool,
    /// The named agent a forked skill runs as.
    pub agent: Option<String>,
}

impl Skill {
    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }
}

/// Something about one skill file the human should hear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiagnostic {
    /// The `SKILL.md` it is about, as found.
    pub path: String,
    pub message: String,
    /// The skill was left out, rather than loaded with a warning.
    pub skipped: bool,
}

/// A skill as the CLI and the settings window show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub path: String,
    pub scope: SkillScope,
    pub model_invocable: bool,
    pub user_invocable: bool,
}

/// Everything the scan found, for the human.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SkillReport {
    pub skills: Vec<SkillInfo>,
    pub diagnostics: Vec<SkillDiagnostic>,
}

/// A skill as the catalog in the prompt shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub description: String,
}

impl Entry {
    pub fn line(&self) -> String {
        format!("- {} — {}", self.name, self.description)
    }
}

/// What the system prompt carries: the model-invocable skills that fit in the
/// cap, and whether some were left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}

/// The skills found under a set of roots, in catalog order: project skills
/// first, then the human's, each by name.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    skills: Vec<Skill>,
    diagnostics: Vec<SkillDiagnostic>,
}

/// One directory to scan, and what it is.
pub(super) struct Root<'a> {
    pub dir: &'a Path,
    pub scope: SkillScope,
    /// A skill directory must resolve inside this one: a project's skill that is
    /// a link out of the project is not the project's skill.
    pub fence: Option<&'a Path>,
    /// A plugin's name, put before each skill's: `tools:review`.
    pub namespace: Option<&'a str>,
}

impl Catalog {
    pub(super) fn scan(roots: &[Root]) -> Catalog {
        let mut scan = Scan::default();
        for root in roots {
            scan.root(root);
        }
        let mut skills = scan.skills;
        skills.sort_by(|a, b| (a.scope as u8, &a.name).cmp(&(b.scope as u8, &b.name)));
        Catalog {
            skills,
            diagnostics: scan.diagnostics,
        }
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    pub fn diagnostics(&self) -> &[SkillDiagnostic] {
        &self.diagnostics
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// The catalog for the prompt, or `None` when the model has no skills to
    /// pick from — then the prompt carries no section and no instructions.
    pub fn listing(&self) -> Option<Listing> {
        let mut entries = Vec::new();
        let mut bytes = 0;
        let mut truncated = false;
        for skill in self.skills.iter().filter(|s| s.model_invocable) {
            let entry = Entry {
                name: skill.name.clone(),
                description: skill.description.clone(),
            };
            let len = entry.line().len() + 1;
            if bytes + len > MAX_LIST_BYTES {
                truncated = true;
                break;
            }
            bytes += len;
            entries.push(entry);
        }
        (!entries.is_empty()).then_some(Listing { entries, truncated })
    }

    pub fn report(&self) -> SkillReport {
        SkillReport {
            skills: self
                .skills
                .iter()
                .map(|s| SkillInfo {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    path: plain(&s.file()),
                    scope: s.scope,
                    model_invocable: s.model_invocable,
                    user_invocable: s.user_invocable,
                })
                .collect(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

#[derive(Default)]
struct Scan {
    skills: Vec<Skill>,
    diagnostics: Vec<SkillDiagnostic>,
    by_name: HashMap<String, PathBuf>,
    /// One directory reached by two roots — `~/.claude/skills` linked to
    /// `~/.agents/skills` — is one skill, not a collision.
    seen: HashSet<PathBuf>,
}

impl Scan {
    fn root(&mut self, root: &Root) {
        let Ok(read) = std::fs::read_dir(root.dir) else {
            return;
        };
        // Sorted, so that which of two same-named skills in one root wins does
        // not depend on what order the filesystem lists them in.
        let mut dirs: Vec<(String, PathBuf)> = read
            .flatten()
            .filter_map(|e| Some((e.file_name().into_string().ok()?, e.path())))
            .filter(|(name, path)| !name.starts_with('.') && path.join(FILE).is_file())
            .collect();
        dirs.sort();
        for (dir_name, path) in dirs {
            self.skill(root, &dir_name, &path);
        }
    }

    fn skill(&mut self, root: &Root, dir_name: &str, path: &Path) {
        let shown = plain(&path.join(FILE));
        let Ok(dir) = std::fs::canonicalize(path) else {
            return;
        };
        if let Some(fence) = root.fence {
            if !dir.starts_with(fence) {
                return self.report(&shown, "skill directory leads outside project".into(), true);
            }
        }
        if !self.seen.insert(dir.clone()) {
            return;
        }

        let Some((text, _)) = head(&dir.join(FILE), MAX_HEADER_BYTES) else {
            return self.report(&shown, "cannot read file".into(), true);
        };
        let header = match frontmatter::parse(&text) {
            Ok((header, _)) => header,
            Err(why) => return self.report(&shown, why, true),
        };
        let Some(description) = header.description else {
            return self.report(&shown, "missing description: model has no basis to select skill".into(), true);
        };
        let name = header.name.unwrap_or_else(|| dir_name.to_string());
        if !is_usable_name(&name) {
            return self.report(
                &shown,
                format!("name \"{name}\" is not typeable: Latin letters, digits, - and _ only"),
                true,
            );
        }
        if !is_standard_name(&name) {
            self.report(
                &shown,
                format!("name \"{name}\" is non-standard: lowercase Latin letters, digits, and single hyphens, up to {MAX_NAME_CHARS} characters"),
                false,
            );
        }
        if name != dir_name {
            self.report(
                &shown,
                format!("name \"{name}\" does not match directory \"{dir_name}\"; skill is named \"{name}\""),
                false,
            );
        }
        let name = match root.namespace {
            Some(ns) => format!("{ns}:{name}"),
            None => name,
        };
        if let Some(first) = self.by_name.get(&name) {
            let first = plain(first);
            return self.report(&shown, format!("shadowed by skill of same name {first}"), true);
        }

        let description = one_line(&description);
        let description = if description.chars().count() > MAX_DESCRIPTION_CHARS {
            self.report(
                &shown,
                format!("description exceeds {MAX_DESCRIPTION_CHARS} characters and was truncated"),
                false,
            );
            let cut: String = description.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
            format!("{}…", cut.trim_end())
        } else {
            description
        };

        self.by_name.insert(name.clone(), dir.join(FILE));
        self.skills.push(Skill {
            name,
            description,
            dir,
            scope: root.scope,
            model_invocable: !header.disable_model_invocation,
            user_invocable: header.user_invocable,
            plugin_root: root.namespace.and(root.fence).map(Path::to_path_buf),
            fork: header.fork,
            agent: header.agent.clone(),
        });
    }

    fn report(&mut self, path: &str, message: String, skipped: bool) {
        self.diagnostics.push(SkillDiagnostic {
            path: path.to_string(),
            message,
            skipped,
        });
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Agent Skills naming rule: lowercase ASCII letters, digits and single
/// hyphens, not at either end, at most 64 characters.
pub fn is_standard_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// A name that can be typed after a slash and named in a tool call. Looser than
/// the standard, for skills written for clients that are looser too.
fn is_usable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_USABLE_NAME_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_name_rule() {
        assert!(is_standard_name("pdf-processing"));
        assert!(is_standard_name("a1"));
        for bad in ["", "PDF", "-pdf", "pdf-", "pdf--x", "a_b", "a.b", &"a".repeat(65)] {
            assert!(!is_standard_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn usable_names_are_looser_but_cannot_express_a_path() {
        assert!(is_usable_name("My_Skill"));
        for bad in ["", "a/b", "..", "a b", "a:b", "a\\b"] {
            assert!(!is_usable_name(bad), "{bad:?}");
        }
    }
}

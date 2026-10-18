//! Agent Skills (agentskills.io): instructions kept for one kind of task, which
//! the model pulls in when a task calls for them.
//!
//! A skill is a directory with a `SKILL.md` — a YAML header with `name` and
//! `description`, then the instructions — and whatever files the instructions
//! refer to. Three tiers, as the standard lays them out:
//!
//! 1. The catalog — name and description of each skill — goes into the system
//!    prompt when the session starts ([`Catalog::listing`]). Read once, because
//!    the prompt is the cached prefix of every request.
//! 2. The body arrives when the model calls the `Skill` tool, or when the human
//!    types `/name` ([`activate`]). Read at that moment, from the file, so an
//!    edit made during the session is what the model gets.
//! 3. The skill's other files are listed with the body and read one by one on
//!    request.
//!
//! Where skills come from ([`SkillRoots`]): the project, then the human. Each
//! scope has Yaca's own directory and the two shared ones, `.agents/skills`
//! (the cross-client convention) and `.claude/skills` (where most skills on a
//! machine already are). Yaca reads all of them and writes to none.
//!
//! Trust. Project skills load without asking, as `AGENTS.md` and
//! `.yaca/commands` do: they are text in the repository the human opened, and
//! what the model does with them goes through the same permissions as anything
//! else. That is also why `allowed-tools` grants nothing: a skill from a cloned
//! repository must not be able to pre-approve its own commands.

pub mod activate;
pub mod catalog;
pub mod frontmatter;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

pub use catalog::{
    Catalog, Entry, Listing, Skill, SkillDiagnostic, SkillInfo, SkillReport, SkillScope,
};

use catalog::Root;

/// Where the project keeps skills, relative to its root, in precedence order.
pub const PROJECT_DIRS: [&str; 3] = [".yaca/skills", ".agents/skills", ".claude/skills"];

/// The directories a scan reads, in precedence order.
#[derive(Debug, Clone, Default)]
pub struct SkillRoots {
    /// The project's home (where `.yaca` lives), canonical. `None` scans only
    /// the human's skills.
    project: Option<PathBuf>,
    user: Vec<PathBuf>,
    plugins: Vec<PluginSkills>,
}

/// The skill directories of one plugin.
#[derive(Debug, Clone)]
struct PluginSkills {
    name: String,
    root: PathBuf,
    dirs: Vec<PathBuf>,
}

impl SkillRoots {
    /// The project's skills and the human's `user` directories ([`user_dirs`]).
    /// An empty `user` is a session that sees only the project's, which is
    /// what a test gets, so the result does not depend on the machine.
    pub fn new(project_home: &Path, user: Vec<PathBuf>) -> Self {
        SkillRoots {
            project: std::fs::canonicalize(project_home).ok(),
            user,
            plugins: Vec::new(),
        }
    }

    /// Adds the skills of `plugins`, after the project's and the human's.
    pub fn with_plugins(mut self, plugins: &[crate::plugin::Plugin]) -> Self {
        self.plugins = plugins
            .iter()
            .map(|p| PluginSkills {
                name: p.name.clone(),
                root: p.root.clone(),
                dirs: p.skill_dirs.clone(),
            })
            .collect();
        self
    }

    pub fn scan(&self) -> Catalog {
        let project: Vec<PathBuf> = match &self.project {
            Some(home) => PROJECT_DIRS.iter().map(|d| home.join(d)).collect(),
            None => Vec::new(),
        };
        let mut roots: Vec<Root> = project
            .iter()
            .map(|dir| Root {
                dir,
                scope: SkillScope::Project,
                fence: self.project.as_deref(),
                namespace: None,
            })
            .collect();
        roots.extend(self.user.iter().map(|dir| Root {
            dir,
            scope: SkillScope::User,
            fence: None,
            namespace: None,
        }));
        // A plugin's skill must stay inside the plugin (Agent Plugins §4).
        for plugin in &self.plugins {
            roots.extend(plugin.dirs.iter().map(|dir| Root {
                dir,
                scope: SkillScope::Plugin,
                fence: Some(&plugin.root),
                namespace: Some(&plugin.name),
            }));
        }
        Catalog::scan(&roots)
    }
}

/// The human's skill directories, in precedence order: Yaca's own under the
/// configuration directory, then the shared ones in the home directory.
pub fn user_dirs(config_dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![config_dir.join("skills")];
    if let Some(home) = home_dir() {
        out.push(home.join(".agents").join("skills"));
        out.push(home.join(".claude").join("skills"));
    }
    out
}

fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

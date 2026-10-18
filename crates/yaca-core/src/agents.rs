//! Named subagents: a subagent kept under a name, with its own instructions,
//! tools and model.
//!
//! The file format is Claude Code's, because that is where these files
//! already are: a Markdown file with a YAML header — `name`, `description`,
//! `tools`, `model` — and the instructions as its body. `Task` takes the name
//! as `agent`, and the subagent it starts is the plain one with three changes:
//!
//! * the body is added to its system prompt, after Yaca's own parts;
//! * `tools` narrows the tools it gets, and never widens them: what an agent
//!   file lists that the subagent would not have anyway is ignored, so a file
//!   from a cloned repository cannot hand a subagent `Task` or `AskUser`;
//! * `model` picks the model, among the ones the session's provider has.
//!
//! Where agents come from, in precedence order: the project (`.yaca/agents`,
//! `.claude/agents`), the human (`agents/` in Yaca's settings folder,
//! `~/.claude/agents`), then the plugins, as `plugin:agent`. A name that comes
//! twice is the first one's. Read once when the session opens, like the
//! skills catalog: the list goes into the system prompt, which is the cached
//! head of every request.

use std::path::{Path, PathBuf};

use yaml_rust2::{Yaml, YamlLoader};

use crate::usage::ModelInfo;

/// Where the project keeps agents, relative to its root, in precedence order.
pub const PROJECT_DIRS: [&str; 2] = [".yaca/agents", ".claude/agents"];

/// How much of the catalog goes into the prompt. A project with sixty agents
/// would otherwise spend a page of every request on them.
const MAX_LISTED: usize = 40;
const MAX_DESCRIPTION_CHARS: usize = 400;
/// Ids named back for a model nobody has: a router lists hundreds.
const MAX_LISTED_MODELS: usize = 30;

/// Where an agent was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentScope {
    Project,
    User,
    Plugin,
}

/// One agent the session knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDef {
    pub name: String,
    /// One line: when to send this agent.
    pub description: String,
    /// The tools it may use, as written: `None` means all a subagent has.
    pub tools: Option<Vec<String>>,
    /// The model it asks for, as written: an id, or `inherit`.
    pub model: Option<String>,
    /// Its instructions: the file's body.
    pub prompt: String,
    pub scope: AgentScope,
    pub path: PathBuf,
}

/// The directories a scan reads.
#[derive(Debug, Clone, Default)]
pub struct AgentRoots {
    project: Option<PathBuf>,
    user: Vec<PathBuf>,
    /// Each plugin's name and its agent directories.
    plugins: Vec<(String, Vec<PathBuf>)>,
}

impl AgentRoots {
    pub fn new(project_home: &Path, user: Vec<PathBuf>) -> Self {
        AgentRoots {
            project: Some(project_home.to_path_buf()),
            user,
            plugins: Vec::new(),
        }
    }

    pub fn with_plugins(mut self, plugins: &[crate::plugin::Plugin]) -> Self {
        self.plugins = plugins
            .iter()
            .filter(|p| !p.agent_dirs.is_empty())
            .map(|p| (p.name.clone(), p.agent_dirs.clone()))
            .collect();
        self
    }

    /// Every agent, the first of each name kept.
    pub fn scan(&self) -> Vec<AgentDef> {
        let mut found: Vec<AgentDef> = Vec::new();
        let mut add = |dir: &Path, scope: AgentScope, prefix: Option<&str>| {
            for def in read_dir(dir, scope, prefix) {
                if !found.iter().any(|f| f.name == def.name) {
                    found.push(def);
                }
            }
        };
        if let Some(home) = &self.project {
            for dir in PROJECT_DIRS {
                add(&home.join(dir), AgentScope::Project, None);
            }
        }
        for dir in &self.user {
            add(dir, AgentScope::User, None);
        }
        for (plugin, dirs) in &self.plugins {
            for dir in dirs {
                add(dir, AgentScope::Plugin, Some(plugin));
            }
        }
        found
    }
}

/// The human's agent directories: Yaca's own, then Claude Code's.
pub fn user_dirs(config_dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![config_dir.join("agents")];
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Some(home) = std::env::var_os(var).filter(|v| !v.is_empty()) {
        out.push(PathBuf::from(home).join(".claude").join("agents"));
    }
    out
}

fn read_dir(dir: &Path, scope: AgentScope, prefix: Option<&str>) -> Vec<AgentDef> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let mut def = parse(&text, &path, scope)?;
            if let Some(prefix) = prefix {
                def.name = format!("{prefix}:{}", def.name);
            }
            Some(def)
        })
        .collect()
}

/// Reads one agent file. `None` for a file that is not one: no header, or no
/// description to choose it by.
pub fn parse(text: &str, path: &Path, scope: AgentScope) -> Option<AgentDef> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let (block, body) = crate::slash::split_frontmatter(text)?;
    let doc = YamlLoader::load_from_str(block).ok()?.into_iter().next()?;
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let name = scalar(&doc["name"]).unwrap_or(stem);
    if !name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.')) {
        return None;
    }
    let description = scalar(&doc["description"])?;
    let description: String = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let description = if description.chars().count() > MAX_DESCRIPTION_CHARS {
        let cut: String = description.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        description
    };
    let tools = match &doc["tools"] {
        Yaml::Array(items) => Some(items.iter().filter_map(scalar).collect()),
        other => scalar(other).map(|line| {
            line.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        }),
    };
    Some(AgentDef {
        name,
        description,
        tools,
        model: scalar(&doc["model"]),
        prompt: body.trim().to_string(),
        scope,
        path: path.to_path_buf(),
    })
}

fn scalar(value: &Yaml) -> Option<String> {
    let text = match value {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(n) => n.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Whether the agent's `tools` lets it use the Yaca tool `tool`. Claude Code's
/// names count for the tools Yaca names differently (`MultiEdit` is `Edit`,
/// `PowerShell` is `Bash`), and `mcp__server` covers that server's tools.
pub fn allows(def: &AgentDef, tool: &str) -> bool {
    let Some(list) = &def.tools else {
        return true;
    };
    let names = crate::plugin::hooks::claude_names(tool);
    list.iter().any(|listed| {
        listed == tool
            || listed == "*"
            || names.contains(&listed.as_str())
            || (listed.starts_with(crate::mcp::TOOL_PREFIX)
                && tool.starts_with(&format!("{listed}__")))
    })
}

/// The model `spec` names among `models`, the chat's provider's list: an
/// agent's `model`, or `Task`'s. `Ok(None)` is `inherit`: the conversation's
/// own model. `Err` names the ids there are.
///
/// A model is named by its id, whoever made it; there are no family aliases,
/// because a family name means a different model at every provider. Case is
/// forgiven. A provider that lists no models leaves nothing to check against,
/// so the id is taken as written and the provider judges it.
pub fn resolve_model(spec: &str, models: &[ModelInfo]) -> Result<Option<String>, String> {
    let spec = spec.trim();
    if spec.eq_ignore_ascii_case("inherit") {
        return Ok(None);
    }
    if models.is_empty() {
        return Ok(Some(spec.to_string()));
    }
    if let Some(found) = models
        .iter()
        .find(|m| m.id == spec)
        .or_else(|| models.iter().find(|m| m.id.eq_ignore_ascii_case(spec)))
    {
        return Ok(Some(found.id.clone()));
    }
    // One id can be listed under two providers of a router.
    let mut ids: Vec<&str> = Vec::new();
    for model in models {
        if !ids.contains(&model.id.as_str()) {
            ids.push(&model.id);
        }
    }
    let listed = ids.iter().take(MAX_LISTED_MODELS).copied().collect::<Vec<_>>().join(", ");
    let more = ids.len().saturating_sub(MAX_LISTED_MODELS);
    Err(if more > 0 {
        format!("this chat's provider has no model `{spec}`; its models include {listed} and {more} more")
    } else {
        format!("this chat's provider has no model `{spec}`; its models are {listed}")
    })
}

/// The catalog the agent's system prompt carries, or `None` when there are
/// no agents.
pub fn section(agents: &[AgentDef]) -> Option<String> {
    if agents.is_empty() {
        return None;
    }
    let mut s = String::from(
        "<available_agents>\nNamed subagents, each with instructions of its own. To hand a task to \
         one, call Task with its name as `agent`; without `agent`, Task sends a plain subagent. \
         Choose one whose description fits the task.\n",
    );
    for def in agents.iter().take(MAX_LISTED) {
        s.push_str(&format!("- {}: {}\n", def.name, def.description));
    }
    if agents.len() > MAX_LISTED {
        s.push_str(&format!(
            "(only the first {MAX_LISTED} are listed; Task names them all when given a name it does not know)\n"
        ));
    }
    s.push_str("</available_agents>\n");
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    const REVIEWER: &str = "---\nname: reviewer\ndescription: >\n  Reviews a diff\n  for bugs.\ntools: Read, Grep, Glob\nmodel: claude-haiku-4-5\n---\n\nYou review code. Be terse.\n";

    #[test]
    fn a_claude_code_agent_file_is_read() {
        let def = parse(REVIEWER, Path::new("reviewer.md"), AgentScope::Project).unwrap();
        assert_eq!(def.name, "reviewer");
        assert_eq!(def.description, "Reviews a diff for bugs.");
        assert_eq!(def.tools, Some(vec!["Read".into(), "Grep".into(), "Glob".into()]));
        assert_eq!(def.model.as_deref(), Some("claude-haiku-4-5"));
        assert_eq!(def.prompt, "You review code. Be terse.");
    }

    #[test]
    fn the_name_defaults_to_the_file_and_a_description_is_required() {
        let def = parse("---\ndescription: d\n---\nbody", Path::new("x/scout.md"), AgentScope::User).unwrap();
        assert_eq!(def.name, "scout");
        assert!(def.tools.is_none());
        assert!(parse("---\nname: x\n---\nbody", Path::new("x.md"), AgentScope::User).is_none());
        assert!(parse("no header", Path::new("x.md"), AgentScope::User).is_none());
    }

    #[test]
    fn tools_narrow_and_speak_claude_codes_names() {
        let def = parse(
            "---\ndescription: d\ntools: [MultiEdit, PowerShell, mcp__tracker]\n---\n",
            Path::new("a.md"),
            AgentScope::Project,
        )
        .unwrap();
        assert!(allows(&def, "Edit"));
        assert!(allows(&def, "Bash"));
        assert!(allows(&def, "mcp__tracker__create_task"));
        assert!(!allows(&def, "Write"));
        assert!(!allows(&def, "mcp__other__x"));
        let all = parse("---\ndescription: d\n---\n", Path::new("b.md"), AgentScope::Project).unwrap();
        assert!(allows(&all, "Write"));
    }

    #[test]
    fn a_model_is_any_listed_id_or_inherit() {
        let models: Vec<ModelInfo> = ["claude-sonnet-5-5", "gpt-5.1", "qwen/qwen3-coder"]
            .iter()
            .map(|id| crate::usage::unknown_model(id))
            .collect();
        assert_eq!(resolve_model("inherit", &models), Ok(None));
        assert_eq!(resolve_model("gpt-5.1", &models), Ok(Some("gpt-5.1".into())));
        assert_eq!(resolve_model(" Qwen/Qwen3-Coder ", &models), Ok(Some("qwen/qwen3-coder".into())));
        // No family aliases: a name is an id or nothing.
        let err = resolve_model("sonnet", &models).unwrap_err();
        assert!(err.contains("`sonnet`") && err.contains("gpt-5.1"), "{err}");
        // With no list, the provider is the judge.
        assert_eq!(resolve_model("anything-7b", &[]), Ok(Some("anything-7b".into())));
    }

    #[test]
    fn the_project_comes_first_and_plugins_are_prefixed() {
        let dir = TempDir::new();
        dir.write(".yaca/agents/reviewer.md", REVIEWER);
        dir.write(".claude/agents/reviewer.md", "---\ndescription: shadowed\n---\n");
        dir.write(".claude/agents/scout.md", "---\ndescription: Finds things\n---\nLook.");
        let found = AgentRoots::new(dir.path(), Vec::new()).scan();
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["reviewer", "scout"]);
        assert_eq!(found[0].description, "Reviews a diff for bugs.");

        let section = section(&found).unwrap();
        assert!(section.contains("- reviewer: Reviews a diff for bugs."), "{section}");
        assert!(section.contains("`agent`"));
    }
}

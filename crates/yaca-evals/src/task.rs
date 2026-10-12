//! Reading `evals/tasks/<id>/task.json` and checking that it can be verified.
//!
//! Validation happens before any model is called. A task with a typo in its
//! `expect` list, or with no way to verify it, would otherwise cost a full run
//! and then report a result nobody can trust.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// What the agent is allowed to do. `agent` is the full toolset; `plan` is the
/// read-only one, for tasks that only ask for an analysis written to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskMode {
    Agent,
    Plan,
}

/// The command that decides whether a task is solved. It runs through the
/// shell in the final tree, in `cwd` (relative to that tree).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub command: String,
    pub cwd: String,
}

/// A file assertion on the final tree. Used alone, or next to a [`Check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// The file exists and contains `text`.
    Contains { path: String, text: String },
    /// The file does not exist.
    Absent { path: String },
}

impl Expect {
    pub fn path(&self) -> &str {
        match self {
            Expect::Contains { path, .. } | Expect::Absent { path } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// The directory name. Also the name in the summary and the output path.
    pub id: String,
    /// `evals/tasks/<id>/`.
    pub dir: PathBuf,
    /// The starting tree, `dir/repo`. Never modified: every run copies it.
    pub repo: PathBuf,
    pub prompt: String,
    pub mode: TaskMode,
    /// Optional. Without it the provider decides, as it does for `yaca`.
    pub model: Option<String>,
    pub timeout: Duration,
    pub check: Option<Check>,
    pub expect: Vec<Expect>,
}

/// Ten minutes: enough for a small fix with a few builds and test runs, short
/// enough that a model stuck in a loop does not hold the whole suite.
const DEFAULT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawTask {
    prompt: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    check: Option<RawCheck>,
    #[serde(default)]
    expect: Vec<RawExpect>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawCheck {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawExpect {
    path: String,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default)]
    absent: Option<bool>,
}

/// Every task under `root`, in name order. `only` narrows the set; a name that
/// does not exist is an error, not a silent skip, so a typo cannot make a run
/// look green with nothing in it.
pub fn load_tasks(root: &Path, only: &[String]) -> Result<Vec<Task>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .with_context(|| format!("cannot read the tasks directory {}", root.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    for wanted in only {
        if !dirs.iter().any(|d| d.file_name().is_some_and(|n| n == wanted.as_str())) {
            bail!("no task named `{wanted}` in {}", root.display());
        }
    }

    let mut tasks = Vec::new();
    for dir in dirs {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        tasks.push(load_task(&dir)?);
    }
    if tasks.is_empty() {
        bail!("no tasks to run in {}", root.display());
    }
    Ok(tasks)
}

/// One task directory.
pub fn load_task(dir: &Path) -> Result<Task> {
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .context("task directory has no name")?;
    let file = dir.join("task.json");
    let text = std::fs::read_to_string(&file)
        .with_context(|| format!("task `{id}`: cannot read {}", file.display()))?;
    let raw: RawTask =
        serde_json::from_str(&text).with_context(|| format!("task `{id}`: bad task.json"))?;
    let repo = dir.join("repo");
    if !repo.is_dir() {
        bail!("task `{id}`: no repo/ directory next to task.json");
    }
    validate(&id, raw, dir.to_path_buf(), repo)
}

fn validate(id: &str, raw: RawTask, dir: PathBuf, repo: PathBuf) -> Result<Task> {
    let fail = |why: String| anyhow::anyhow!("task `{id}`: {why}");

    if raw.prompt.trim().is_empty() {
        return Err(fail("the prompt is empty".into()));
    }
    let mode = match raw.mode.as_deref().unwrap_or("agent") {
        "agent" => TaskMode::Agent,
        "plan" => TaskMode::Plan,
        other => return Err(fail(format!("mode must be `agent` or `plan`, not `{other}`"))),
    };
    let timeout_secs = raw.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    if timeout_secs == 0 {
        return Err(fail("timeoutSecs must be positive".into()));
    }

    let check = match raw.check {
        None => None,
        Some(c) => {
            if c.command.trim().is_empty() {
                return Err(fail("check.command is empty".into()));
            }
            let cwd = c.cwd.unwrap_or_else(|| ".".into());
            safe_relative(&cwd).map_err(|why| fail(format!("check.cwd {why}")))?;
            Some(Check { command: c.command, cwd })
        }
    };

    let mut expect = Vec::with_capacity(raw.expect.len());
    for e in raw.expect {
        safe_relative(&e.path).map_err(|why| fail(format!("expect path {why}")))?;
        expect.push(match (e.contains, e.absent) {
            (Some(text), None) => Expect::Contains { path: e.path, text },
            (None, Some(true)) => Expect::Absent { path: e.path },
            (None, Some(false)) => {
                return Err(fail(format!(
                    "expect for `{}`: `absent: false` says nothing; use `contains`",
                    e.path
                )))
            }
            (Some(_), Some(_)) => {
                return Err(fail(format!(
                    "expect for `{}` has both `contains` and `absent`",
                    e.path
                )))
            }
            (None, None) => {
                return Err(fail(format!(
                    "expect for `{}` needs `contains` or `absent`",
                    e.path
                )))
            }
        });
    }

    if check.is_none() && expect.is_empty() {
        return Err(fail(
            "nothing verifies it: give a `check` command or an `expect` list".into(),
        ));
    }

    Ok(Task {
        id: id.to_string(),
        dir,
        repo,
        prompt: raw.prompt,
        mode,
        model: raw.model.filter(|m| !m.trim().is_empty()),
        timeout: Duration::from_secs(timeout_secs),
        check,
        expect,
    })
}

/// A path that stays inside the run's tree. Assertions and the check cwd are
/// read from that tree; a `..` or an absolute path would read the machine
/// around it and make a result depend on what else is installed.
pub fn safe_relative(path: &str) -> std::result::Result<(), String> {
    if path.trim().is_empty() {
        return Err("is empty".into());
    }
    let p = Path::new(path);
    if p.is_absolute() || p.has_root() {
        return Err(format!("must be relative to the run's tree, not `{path}`"));
    }
    if p.components()
        .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir))
    {
        return Err(format!("must not leave the run's tree: `{path}`"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_json(dir: &Path, json: &str) -> PathBuf {
        let task = dir.join("task-under-test");
        std::fs::create_dir_all(task.join("repo")).unwrap();
        std::fs::write(task.join("task.json"), json).unwrap();
        task
    }

    fn scratch() -> PathBuf {
        crate::workdir::create_unique(&std::env::temp_dir().join("yaca-evals-tests"), "task")
            .unwrap()
    }

    #[test]
    fn a_minimal_task_gets_the_defaults() {
        let root = scratch();
        let dir = task_json(&root, r#"{ "prompt": "do it", "check": { "command": "exit 0" } }"#);
        let task = load_task(&dir).unwrap();
        assert_eq!(task.mode, TaskMode::Agent);
        assert_eq!(task.timeout, Duration::from_secs(600));
        assert_eq!(task.model, None);
        assert_eq!(task.check.as_ref().unwrap().cwd, ".");
        assert!(task.expect.is_empty());
    }

    #[test]
    fn both_expect_forms_are_read() {
        let root = scratch();
        let dir = task_json(
            &root,
            r#"{ "prompt": "p", "mode": "plan", "timeoutSecs": 30,
                 "expect": [ {"path": "src/a.rs", "contains": "fn a"},
                             {"path": "old.rs", "absent": true} ] }"#,
        );
        let task = load_task(&dir).unwrap();
        assert_eq!(task.mode, TaskMode::Plan);
        assert_eq!(task.timeout, Duration::from_secs(30));
        assert_eq!(
            task.expect,
            vec![
                Expect::Contains { path: "src/a.rs".into(), text: "fn a".into() },
                Expect::Absent { path: "old.rs".into() },
            ]
        );
    }

    #[test]
    fn an_expect_with_neither_or_both_conditions_is_refused() {
        let root = scratch();
        let neither = task_json(&root, r#"{ "prompt": "p", "expect": [ {"path": "a"} ] }"#);
        assert!(load_task(&neither).unwrap_err().to_string().contains("needs"));
        let both = task_json(
            &root,
            r#"{ "prompt": "p", "expect": [ {"path": "a", "contains": "x", "absent": true} ] }"#,
        );
        assert!(load_task(&both).unwrap_err().to_string().contains("both"));
    }

    #[test]
    fn a_task_nothing_verifies_is_refused() {
        let root = scratch();
        let dir = task_json(&root, r#"{ "prompt": "p" }"#);
        let err = load_task(&dir).unwrap_err().to_string();
        assert!(err.contains("nothing verifies it"), "{err}");
    }

    #[test]
    fn paths_leaving_the_tree_are_refused() {
        assert!(safe_relative("../secret").is_err());
        assert!(safe_relative("src/../../x").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
        assert!(safe_relative("").is_err());
        assert!(safe_relative("src/lib.rs").is_ok());
        let root = scratch();
        let dir = task_json(
            &root,
            r#"{ "prompt": "p", "check": { "command": "exit 0", "cwd": ".." } }"#,
        );
        assert!(load_task(&dir).is_err());
    }

    #[test]
    fn unknown_fields_are_refused_so_a_typo_cannot_drop_a_check() {
        let root = scratch();
        let dir = task_json(&root, r#"{ "prompt": "p", "chek": { "command": "exit 0" } }"#);
        assert!(load_task(&dir).is_err());
    }

    #[test]
    fn a_missing_repo_is_refused() {
        let root = scratch();
        let dir = task_json(&root, r#"{ "prompt": "p", "check": { "command": "exit 0" } }"#);
        std::fs::remove_dir_all(dir.join("repo")).unwrap();
        assert!(load_task(&dir).unwrap_err().to_string().contains("repo/"));
    }

    #[test]
    fn naming_an_unknown_task_is_an_error() {
        let root = scratch();
        task_json(&root, r#"{ "prompt": "p", "check": { "command": "exit 0" } }"#);
        let err = load_tasks(&root, &["nope".into()]).unwrap_err().to_string();
        assert!(err.contains("no task named `nope`"), "{err}");
    }
}

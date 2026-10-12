//! What a run reports: one JSON file per task, a `summary.json` for the whole
//! suite, and the table printed to stdout.
//!
//! The JSON uses camelCase like the rest of the event stream, so the same
//! reader can handle both. `solved` is the only field a pass/fail gate needs;
//! the rest exists to compare two runs of the same task.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use yaca_core::usage::TokenCounts;

use crate::check::CheckReport;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub task: String,
    /// The check passed. Nothing else decides it: a run that wrote the right
    /// file and then errored out is solved, and a run that stopped cleanly with
    /// the tree unchanged is not.
    pub solved: bool,
    pub provider: String,
    pub model: String,
    /// `agent` or `plan`, as the task wrote it.
    pub mode: String,
    /// `endTurn`, `maxTokens`, ... as the core reports them, or `cancelled`,
    /// `timeout`, `closed` and `setupFailed` for runs that never got there.
    pub stop_reason: String,
    pub turns: u32,
    pub tool_calls: BTreeMap<String, u32>,
    pub tool_calls_total: u32,
    pub tokens: TokenCounts,
    pub cost_usd: f64,
    pub wall_ms: u64,
    pub permissions_denied: u32,
    pub questions_dismissed: u32,
    pub errors: Vec<String>,
    pub checks: CheckReport,
    /// Why the task is not solved, in one line. `None` when it is.
    pub failure: Option<String>,
}

impl TaskResult {
    /// A result with nothing observed yet. Every field a run fills in starts
    /// at its zero value, so a run that dies early still writes a valid file.
    pub fn new(task: &str, provider: &str, model: &str, mode: &str) -> Self {
        TaskResult {
            task: task.into(),
            solved: false,
            provider: provider.into(),
            model: model.into(),
            mode: mode.into(),
            stop_reason: "setupFailed".into(),
            turns: 0,
            tool_calls: BTreeMap::new(),
            tool_calls_total: 0,
            tokens: TokenCounts::default(),
            cost_usd: 0.0,
            wall_ms: 0,
            permissions_denied: 0,
            questions_dismissed: 0,
            errors: Vec::new(),
            checks: CheckReport::default(),
            failure: None,
        }
    }

    /// Settles `solved` and `failure` from the check. Called once, at the end.
    /// A failure already recorded (a setup error, say) is kept as it is.
    pub fn judge(&mut self) {
        self.solved = self.failure.is_none() && self.checks.passed();
        if self.solved {
            self.failure = None;
            return;
        }
        if self.failure.is_none() {
            self.failure = Some(self.first_failure());
        }
    }

    fn first_failure(&self) -> String {
        if let Some(e) = self.checks.expects.iter().find(|e| !e.passed) {
            return format!("expect: {}", e.detail);
        }
        if let Some(c) = self.checks.command.as_ref().filter(|c| !c.passed) {
            if c.timed_out {
                return "check timed out".into();
            }
            return match c.exit_code {
                Some(code) => format!("check exited {code}"),
                None => "check did not run".into(),
            };
        }
        // Reached only when nothing was checked at all, which task validation
        // is meant to prevent; say so rather than blaming a check that is absent.
        "nothing was checked".into()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub started_at_ms: u64,
    pub provider: String,
    pub solved: usize,
    pub failed: usize,
    pub cost_usd: f64,
    pub wall_ms: u64,
    pub tasks: Vec<TaskResult>,
}

impl Summary {
    pub fn new(started_at_ms: u64, provider: &str, tasks: Vec<TaskResult>) -> Self {
        let solved = tasks.iter().filter(|t| t.solved).count();
        Summary {
            started_at_ms,
            provider: provider.into(),
            solved,
            failed: tasks.len() - solved,
            cost_usd: tasks.iter().map(|t| t.cost_usd).sum(),
            wall_ms: tasks.iter().map(|t| t.wall_ms).sum(),
            tasks,
        }
    }

    /// Non-zero when any task is not solved. An empty run is a failure too:
    /// a suite that ran nothing must not pass a gate.
    pub fn all_solved(&self) -> bool {
        !self.tasks.is_empty() && self.failed == 0
    }
}

/// The table on stdout. Plain text, aligned by character count so the Cyrillic
/// in failure notes does not push the columns out.
pub fn render_table(summary: &Summary) -> String {
    let header = ["TASK", "RESULT", "STOP", "TURNS", "TOOLS", "IN/OUT TOKENS", "COST", "TIME", "NOTE"];
    let mut rows: Vec<Vec<String>> = vec![header.iter().map(|s| s.to_string()).collect()];
    for t in &summary.tasks {
        rows.push(vec![
            t.task.clone(),
            if t.solved { "solved".into() } else { "FAILED".into() },
            t.stop_reason.clone(),
            t.turns.to_string(),
            t.tool_calls_total.to_string(),
            format!("{}/{}", t.tokens.input, t.tokens.output),
            format!("${:.4}", t.cost_usd),
            format!("{:.1}s", t.wall_ms as f64 / 1000.0),
            t.failure.clone().unwrap_or_default(),
        ]);
    }
    rows.push(vec![
        format!("{} solved, {} failed", summary.solved, summary.failed),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        format!("${:.4}", summary.cost_usd),
        format!("{:.1}s", summary.wall_ms as f64 / 1000.0),
        String::new(),
    ]);

    let columns = header.len();
    let widths: Vec<usize> = (0..columns)
        .map(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(c, cell)| {
                // The note is last and free-form: padding it would only add
                // trailing spaces to every line.
                if c == columns - 1 {
                    cell.clone()
                } else {
                    format!("{cell:<width$}", width = widths[c])
                }
            })
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
        if i == 0 {
            out.push_str(&"-".repeat(widths.iter().sum::<usize>() + 2 * (columns - 1)));
            out.push('\n');
        }
    }
    out
}

/// Writes `value` as pretty JSON, creating the parent directory.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{CommandReport, ExpectReport};

    fn passing() -> TaskResult {
        let mut t = TaskResult::new("fix-median", "anthropic", "m", "agent");
        t.stop_reason = "endTurn".into();
        t.checks = CheckReport {
            expects: vec![],
            command: Some(CommandReport {
                command: "cargo test".into(),
                cwd: ".".into(),
                exit_code: Some(0),
                timed_out: false,
                passed: true,
                output_tail: String::new(),
            }),
        };
        t.cost_usd = 0.1;
        t.wall_ms = 1500;
        t.judge();
        t
    }

    #[test]
    fn a_passing_check_solves_and_clears_nothing_else() {
        let t = passing();
        assert!(t.solved);
        assert_eq!(t.failure, None);
    }

    #[test]
    fn a_failing_expect_names_itself() {
        let mut t = TaskResult::new("rename", "anthropic", "m", "agent");
        t.checks.expects.push(ExpectReport {
            path: "src/a.rs".into(),
            kind: "contains",
            passed: false,
            detail: "src/a.rs does not contain \"fn b\"".into(),
        });
        t.judge();
        assert!(!t.solved);
        assert!(t.failure.unwrap().starts_with("expect: src/a.rs does not contain"));
    }

    #[test]
    fn a_setup_failure_is_kept_even_if_the_check_would_pass() {
        let mut t = TaskResult::new("x", "anthropic", "m", "agent");
        t.failure = Some("cannot copy the fixture".into());
        t.judge();
        assert!(!t.solved);
        assert_eq!(t.failure.as_deref(), Some("cannot copy the fixture"));
    }

    #[test]
    fn the_result_json_has_the_fields_a_comparison_reads() {
        let value = serde_json::to_value(passing()).unwrap();
        for key in [
            "task", "solved", "provider", "model", "mode", "stopReason", "turns",
            "toolCalls", "toolCallsTotal", "tokens", "costUsd", "wallMs",
            "permissionsDenied", "questionsDismissed", "errors", "checks", "failure",
        ] {
            assert!(value.get(key).is_some(), "missing `{key}` in {value}");
        }
        assert_eq!(value["tokens"]["cacheRead"], 0);
        assert_eq!(value["checks"]["command"]["exitCode"], 0);
    }

    #[test]
    fn the_summary_counts_and_an_empty_run_is_not_green() {
        let mut bad = TaskResult::new("b", "anthropic", "m", "agent");
        bad.judge();
        let summary = Summary::new(0, "anthropic", vec![passing(), bad]);
        assert_eq!(summary.solved, 1);
        assert_eq!(summary.failed, 1);
        assert!(!summary.all_solved());
        assert!((summary.cost_usd - 0.1).abs() < 1e-9);

        assert!(!Summary::new(0, "anthropic", vec![]).all_solved());
        assert!(Summary::new(0, "anthropic", vec![passing()]).all_solved());
    }

    #[test]
    fn the_table_has_a_row_per_task_and_a_total() {
        let mut bad = TaskResult::new("b-task", "anthropic", "m", "agent");
        bad.failure = Some("check exited 1".into());
        let text = render_table(&Summary::new(0, "anthropic", vec![passing(), bad]));
        assert!(text.contains("fix-median"));
        assert!(text.contains("FAILED"));
        assert!(text.contains("check exited 1"));
        assert!(text.contains("1 solved, 1 failed"));
    }
}

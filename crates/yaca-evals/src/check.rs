//! Deciding whether a run solved its task: the `expect` assertions and the
//! check command, both read from the final tree.
//!
//! The check runs through the platform shell because the tasks are written as
//! command lines (`cargo test --quiet`). It is the only part of a run that
//! executes something the fixture provides, so it runs in the copied tree and
//! never in the fixture itself.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::task::{Check, Expect};

/// Long enough for a cold `cargo test` of a small crate on a slow machine,
/// short enough that a hung check cannot stall the suite indefinitely.
const CHECK_TIMEOUT: Duration = Duration::from_secs(900);

/// How much of the check's output is kept in the result. The tail is where a
/// test failure says what went wrong; the head is usually compiler noise.
const OUTPUT_TAIL_CHARS: usize = 4_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExpectReport {
    pub path: String,
    /// `contains` or `absent`, as the task wrote it.
    pub kind: &'static str,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandReport {
    pub command: String,
    pub cwd: String,
    /// `None` when the process was killed or never started.
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub passed: bool,
    pub output_tail: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub expects: Vec<ExpectReport>,
    pub command: Option<CommandReport>,
}

impl CheckReport {
    /// Every assertion holds and the command, if any, exits 0. An empty report
    /// is not a pass: nothing was verified, and "nothing failed" must not be
    /// read as "solved". Task validation refuses such a task before it runs,
    /// so reaching this with nothing is a bug somewhere else, and it fails loud.
    pub fn passed(&self) -> bool {
        let verified = !self.expects.is_empty() || self.command.is_some();
        verified
            && self.expects.iter().all(|e| e.passed)
            && self.command.as_ref().is_none_or(|c| c.passed)
    }
}

/// Runs every assertion, then the command. Both always run, even after one
/// fails, so the report says everything that is wrong at once.
pub async fn run_check(root: &Path, expect: &[Expect], check: Option<&Check>) -> CheckReport {
    let expects = expect.iter().map(|e| evaluate_expect(root, e)).collect();
    let command = match check {
        Some(check) => Some(run_command(root, check, CHECK_TIMEOUT).await),
        None => None,
    };
    CheckReport { expects, command }
}

pub fn evaluate_expect(root: &Path, expect: &Expect) -> ExpectReport {
    match expect {
        Expect::Contains { path, text } => {
            let file = root.join(path);
            let (passed, detail) = match std::fs::read(&file) {
                Ok(bytes) => {
                    let body = String::from_utf8_lossy(&bytes);
                    if body.contains(text.as_str()) {
                        (true, "found".into())
                    } else {
                        (false, format!("{path} does not contain {text:?}"))
                    }
                }
                Err(e) => (false, format!("cannot read {path}: {e}")),
            };
            ExpectReport { path: path.clone(), kind: "contains", passed, detail }
        }
        Expect::Absent { path } => {
            // `symlink_metadata` so a dangling link still counts as present.
            let present = std::fs::symlink_metadata(root.join(path)).is_ok();
            let detail = if present { format!("{path} still exists") } else { "absent".into() };
            ExpectReport { path: path.clone(), kind: "absent", passed: !present, detail }
        }
    }
}

pub async fn run_command(root: &Path, check: &Check, timeout: Duration) -> CommandReport {
    let cwd = root.join(&check.cwd);
    let mut report = CommandReport {
        command: check.command.clone(),
        cwd: check.cwd.clone(),
        exit_code: None,
        timed_out: false,
        passed: false,
        output_tail: String::new(),
    };
    if !cwd.is_dir() {
        report.output_tail = format!("check cwd {} is not a directory", cwd.display());
        return report;
    }

    let mut cmd = shell(&check.command);
    cmd.current_dir(&cwd)
        .stdin(std::process::Stdio::null())
        // A check that times out must not outlive the run and keep writing into
        // a tree that is about to be removed.
        .kill_on_drop(true);

    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(Ok(output)) => {
            report.exit_code = output.status.code();
            report.passed = output.status.success();
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            report.output_tail = tail(&text, OUTPUT_TAIL_CHARS);
        }
        Ok(Err(e)) => report.output_tail = format!("cannot start the check: {e}"),
        Err(_) => {
            report.timed_out = true;
            report.output_tail = format!("check did not finish within {}s", timeout.as_secs());
        }
    }
    report
}

/// The platform shell, so a check is written the way a person would type it.
fn shell(command: &str) -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

/// The last `max` characters, cut on a character boundary.
pub fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let skip = count - max;
    format!("…{}", text.chars().skip(skip).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tree(label: &str) -> PathBuf {
        crate::workdir::create_unique(&std::env::temp_dir().join("yaca-evals-tests"), label)
            .unwrap()
    }

    fn check(command: &str, cwd: &str) -> Check {
        Check { command: command.into(), cwd: cwd.into() }
    }

    #[test]
    fn contains_passes_only_when_the_text_is_there() {
        let root = tree("contains");
        std::fs::write(root.join("a.rs"), "pub fn median() {}").unwrap();
        let yes = Expect::Contains { path: "a.rs".into(), text: "fn median".into() };
        let no = Expect::Contains { path: "a.rs".into(), text: "fn mean".into() };
        assert!(evaluate_expect(&root, &yes).passed);
        let miss = evaluate_expect(&root, &no);
        assert!(!miss.passed);
        assert!(miss.detail.contains("does not contain"));
    }

    #[test]
    fn contains_fails_on_a_missing_file_rather_than_passing() {
        let root = tree("contains-missing");
        let r = evaluate_expect(
            &root,
            &Expect::Contains { path: "nope.rs".into(), text: "x".into() },
        );
        assert!(!r.passed);
        assert!(r.detail.contains("cannot read"));
    }

    #[test]
    fn absent_passes_only_when_the_file_is_gone() {
        let root = tree("absent");
        std::fs::write(root.join("old.rs"), "").unwrap();
        let gone = Expect::Absent { path: "gone.rs".into() };
        let still = Expect::Absent { path: "old.rs".into() };
        assert!(evaluate_expect(&root, &gone).passed);
        assert!(!evaluate_expect(&root, &still).passed);
    }

    #[tokio::test]
    async fn exit_status_decides_the_command() {
        let root = tree("exit");
        let ok = run_command(&root, &check("exit 0", "."), Duration::from_secs(60)).await;
        assert!(ok.passed);
        assert_eq!(ok.exit_code, Some(0));
        let bad = run_command(&root, &check("exit 3", "."), Duration::from_secs(60)).await;
        assert!(!bad.passed);
        assert_eq!(bad.exit_code, Some(3));
    }

    #[tokio::test]
    async fn the_command_runs_in_its_cwd() {
        let root = tree("cwd");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/marker"), "").unwrap();
        let probe = if cfg!(windows) {
            "if exist marker (exit 0) else (exit 1)"
        } else {
            "test -f marker"
        };
        let inside = run_command(&root, &check(probe, "sub"), Duration::from_secs(60)).await;
        assert!(inside.passed, "{}", inside.output_tail);
        let outside = run_command(&root, &check(probe, "."), Duration::from_secs(60)).await;
        assert!(!outside.passed);
    }

    #[tokio::test]
    async fn a_missing_cwd_fails_without_running_anything() {
        let root = tree("cwd-missing");
        let r = run_command(&root, &check("exit 0", "nowhere"), Duration::from_secs(60)).await;
        assert!(!r.passed);
        assert!(r.output_tail.contains("not a directory"));
    }

    #[tokio::test]
    async fn output_is_captured_for_the_report() {
        let root = tree("output");
        let r = run_command(&root, &check("echo hello-from-check", "."), Duration::from_secs(60)).await;
        assert!(r.output_tail.contains("hello-from-check"), "{}", r.output_tail);
    }

    #[test]
    fn an_empty_report_is_not_a_pass() {
        assert!(!CheckReport::default().passed());
    }

    #[test]
    fn tail_keeps_the_end_and_respects_characters() {
        assert_eq!(tail("abc", 10), "abc");
        assert_eq!(tail("абвгд", 2), "…гд");
    }

    #[tokio::test]
    async fn run_check_reports_every_failure_not_just_the_first() {
        let root = tree("all");
        let expects = [Expect::Absent { path: "x".into() }];
        std::fs::write(root.join("x"), "").unwrap();
        let report = run_check(&root, &expects, Some(&check("exit 1", "."))).await;
        assert!(!report.passed());
        assert_eq!(report.expects.len(), 1);
        assert!(!report.expects[0].passed);
        assert!(!report.command.as_ref().unwrap().passed);
    }
}

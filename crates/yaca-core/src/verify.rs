//! Check after edit: the project's own verification command, run once after a
//! batch of calls that changed files.
//!
//! A model writes code it cannot always compile. A type error in the third file
//! of a batch surfaces today only when a human builds or the model happens to
//! run the build itself. A project already knows how to check itself, so it says
//! so once in `.yaca/settings.json` under `verify`, and the core runs that
//! command after a batch that changed files and hands the outcome to the model
//! as part of the result it reads next.
//!
//! Trust. The command is not a tool call and does not go through the permission
//! broker. The human wrote it in the project settings, the same file that holds
//! standing rules, and asking before every check would turn each edit into a
//! prompt. It runs with stdin closed, like every command (`process::shell`), so
//! it cannot prompt either.
//!
//! Once per batch. A batch of five edits is one build, and the outcome is
//! attached to the last change in the batch: the result the model reads last
//! before it decides what to do.
//!
//! This module knows nothing about turns, modes or callers. The wiring is in
//! `session::calls`, and everything here can be tested without a session.

use std::collections::VecDeque;

use tokio_util::sync::CancellationToken;

use crate::process::kill;
use crate::process::shell::{self, Shell};
use crate::settings::VerifySettings;
use crate::workspace::Workspace;

/// What the model reads of the output. The head says where the run started, the
/// tail says where it stopped; a compiler's first error and its closing summary
/// are the two parts worth the bytes, and the middle is a list of files.
const HEAD_BYTES: usize = 1_500;
const TAIL_BYTES: usize = 3_500;

/// Whether a successful call with this tool name changed a file, and so
/// earns a check.
///
/// Read from the name rather than from `ToolOutput::changed`: `DeleteDir`
/// reports a count, not a change, so the name is the only thing the core knows
/// it changed the tree.
pub fn changes_files(tool: &str) -> bool {
    matches!(
        tool,
        "Write" | "Edit" | "NotebookEdit" | "Rename" | "Delete" | "DeleteDir" | "Move"
    )
}

/// How a check ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed {
        code: Option<i32>,
        output: String,
    },
    /// Stopped by the timeout, and reported as that. A check that ran out of
    /// time has not passed, and the model must not be left to read silence as
    /// success.
    TimedOut {
        secs: u64,
        output: String,
    },
    /// The human stopped the turn while the check ran. The turn is over, so
    /// nothing is said about it to the model.
    Stopped,
    /// The check could not start or be waited for. Said as "not checked", so
    /// that the absence of a verdict is never read as a pass.
    Broken(String),
}

/// Runs the check and returns how it ended.
///
/// The process tree is killed when the check times out or is stopped (the same
/// `kill::tree` a background command uses): a `cargo` that leaves `rustc`
/// running would hold the target directory and make the next check hang.
pub async fn run(
    settings: &VerifySettings,
    ws: &Workspace,
    shell: Shell,
    cancel: &CancellationToken,
) -> Verdict {
    let cwd = match ws.resolve_existing(settings.cwd()) {
        Ok(dir) if dir.is_dir() => dir,
        Ok(_) => return Verdict::Broken(format!("{} is not a directory", settings.cwd())),
        Err(err) => return Verdict::Broken(err.to_string()),
    };

    let spawned = match shell::spawn(shell, settings.command(), &cwd) {
        Ok(spawned) => spawned,
        Err(err) => return Verdict::Broken(err.to_string()),
    };
    let shell::Spawned {
        mut child,
        pid,
        mut output,
    } = spawned;

    let limit = settings.timeout();
    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);

    let mut capture = Capture::default();
    let ending = loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break Ending::Stopped,
            _ = &mut deadline => break Ending::TimedOut,
            chunk = output.recv() => match chunk {
                Some(raw) => capture.push(&raw),
                None => break Ending::Done,
            },
        }
    };

    if !matches!(ending, Ending::Done) {
        if let Some(pid) = pid {
            let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
        }
        // Reaped although the answer may be discarded: an unwaited child stays
        // in the process table as a zombie.
        let _ = child.wait().await;
    }

    match ending {
        Ending::Stopped => Verdict::Stopped,
        Ending::TimedOut => Verdict::TimedOut {
            secs: limit.as_secs(),
            output: capture.render(),
        },
        Ending::Done => match child.wait().await {
            Ok(status) if status.success() => Verdict::Passed,
            Ok(status) => Verdict::Failed {
                code: status.code(),
                output: capture.render(),
            },
            Err(err) => Verdict::Broken(err.to_string()),
        },
    }
}

enum Ending {
    /// Both pipes reached EOF: the command has said everything it will say.
    Done,
    TimedOut,
    Stopped,
}

/// Keeps the head and the tail of a stream within a byte budget, and counts
/// what was dropped between them.
///
/// Bounded whatever the command prints: a check that writes gigabytes costs
/// the same memory as one that writes nothing.
#[derive(Default)]
struct Capture {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    /// Bytes dropped from the middle. Zero means the output is whole.
    cut: usize,
}

impl Capture {
    fn push(&mut self, bytes: &[u8]) {
        let room = HEAD_BYTES.saturating_sub(self.head.len());
        let (to_head, rest) = bytes.split_at(room.min(bytes.len()));
        self.head.extend_from_slice(to_head);
        self.tail.extend(rest);
        if self.tail.len() > TAIL_BYTES {
            let dropped = self.tail.len() - TAIL_BYTES;
            self.tail.drain(..dropped);
            self.cut += dropped;
        }
    }

    fn render(mut self) -> String {
        let text = if self.cut == 0 {
            let mut whole = self.head;
            whole.extend(self.tail);
            String::from_utf8_lossy(&whole).into_owned()
        } else {
            // Cut at line boundaries on both sides of the gap: half a line says
            // nothing, and the marker already says that something is missing.
            let mut head = self.head;
            if let Some(end) = head.iter().rposition(|&b| b == b'\n') {
                head.truncate(end);
            }
            let tail = self.tail.make_contiguous();
            let start = tail.iter().position(|&b| b == b'\n').map_or(0, |p| p + 1);
            format!(
                "{}\n[...{} bytes cut...]\n{}",
                String::from_utf8_lossy(&head),
                self.cut,
                String::from_utf8_lossy(&tail[start..]),
            )
        };
        text.replace("\r\n", "\n").trim_end().to_string()
    }
}

/// The line the human sees in the feed, English like the rest of the feed.
/// It goes into the expanded row of the file change that triggered the check.
pub fn for_human(command: &str, verdict: &Verdict) -> String {
    match verdict {
        Verdict::Passed => format!("Check \"{command}\": passed\n"),
        Verdict::Failed { code, output } => {
            let how = match code {
                Some(code) => format!("with exit code {code}"),
                None => "with a signal".to_string(),
            };
            format!("Check \"{command}\" finished {how}\n{}\n", body(output))
        }
        Verdict::TimedOut { secs, output } => format!(
            "Check \"{command}\" timed out after {secs}s and was stopped\n{}\n",
            body(output)
        ),
        Verdict::Stopped => format!("Check \"{command}\" stopped\n"),
        Verdict::Broken(why) => format!("Check \"{command}\" failed to start: {why}\n"),
    }
}

/// The text appended to the result the model reads, in English like the rest
/// of what it reads. `None` when there is nothing to say: a stopped check
/// belongs to a turn that is already over.
pub fn for_model(command: &str, verdict: &Verdict) -> Option<String> {
    Some(match verdict {
        Verdict::Passed => format!("\n\n[verify] `{command}` passed."),
        Verdict::Failed { code, output } => {
            let how = match code {
                Some(code) => format!("exit code {code}"),
                None => "a signal".to_string(),
            };
            format!(
                "\n\n[verify] `{command}` failed with {how}. Its output follows. \
                 The errors it reports come from the changes you just made; fix them \
                 before you go on.\n{}",
                body(output)
            )
        }
        Verdict::TimedOut { secs, output } => format!(
            "\n\n[verify] `{command}` did not finish within {secs} s and was stopped, so \
             it did not pass. Output so far:\n{}",
            body(output)
        ),
        Verdict::Broken(why) => {
            format!(
                "\n\n[verify] `{command}` could not be run ({why}), so the changes are unchecked."
            )
        }
        Verdict::Stopped => return None,
    })
}

fn body(output: &str) -> &str {
    if output.is_empty() {
        "(no output)"
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn short_output_is_kept_whole() {
        let mut capture = Capture::default();
        capture.push(b"one\r\n");
        capture.push(b"two\n");
        assert_eq!(capture.render(), "one\ntwo");
    }

    #[test]
    fn long_output_keeps_the_head_and_the_tail_and_says_what_was_cut() {
        let mut capture = Capture::default();
        for n in 0..5_000 {
            capture.push(format!("line {n:05}\n").as_bytes());
        }
        let text = capture.render();

        assert!(text.starts_with("line 00000\n"), "{text:.80}");
        assert!(text.ends_with("line 04999"), "{text:.80}");
        assert!(text.contains("bytes cut"), "the gap must be said");
        assert!(!text.contains("line 02500"), "the middle is what goes");
        assert!(text.len() <= HEAD_BYTES + TAIL_BYTES + 64, "{}", text.len());
        // The cut falls on whole lines: every kept line is complete.
        for line in text.lines().filter(|l| l.starts_with("line ")) {
            assert_eq!(line.len(), "line 00000".len(), "{line:?}");
        }
    }

    #[test]
    fn only_changes_that_write_files_earn_a_check() {
        for tool in ["Write", "Edit", "NotebookEdit", "Rename", "Delete", "DeleteDir", "Move"] {
            assert!(changes_files(tool), "{tool}");
        }
        for tool in ["Read", "Bash", "Grep", "Glob", "TodoWrite"] {
            assert!(!changes_files(tool), "{tool}");
        }
    }

    #[test]
    fn the_model_hears_about_a_pass_and_a_failure() {
        assert_eq!(
            for_model("cargo check", &Verdict::Passed).unwrap(),
            "\n\n[verify] `cargo check` passed."
        );
        let failed = for_model(
            "cargo check",
            &Verdict::Failed {
                code: Some(101),
                output: "error[E0308]".into(),
            },
        )
        .unwrap();
        assert!(failed.contains("exit code 101"), "{failed}");
        assert!(failed.contains("error[E0308]"), "{failed}");
    }

    #[test]
    fn a_check_that_timed_out_is_not_reported_as_passed() {
        let text = for_model(
            "cargo test",
            &Verdict::TimedOut {
                secs: 120,
                output: String::new(),
            },
        )
        .unwrap();
        assert!(text.contains("did not finish"), "{text}");
        assert!(text.contains("did not pass"), "{text}");
        assert!(!text.contains("passed"), "{text}");
    }

    #[test]
    fn a_stopped_check_says_nothing_to_the_model() {
        assert_eq!(for_model("cargo check", &Verdict::Stopped), None);
    }

    #[tokio::test]
    async fn a_check_stopped_before_it_starts_reports_stopped() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let settings = VerifySettings {
            command: "echo never".into(),
            ..VerifySettings::default()
        };
        let verdict = run(&settings, &ws, Shell::default(), &cancel).await;
        assert_eq!(verdict, Verdict::Stopped);
    }

    #[tokio::test]
    async fn a_directory_that_is_not_there_is_reported_not_skipped() {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        let settings = VerifySettings {
            command: "echo never".into(),
            cwd: Some("no-such-dir".into()),
            ..VerifySettings::default()
        };
        let verdict = run(&settings, &ws, Shell::default(), &CancellationToken::new()).await;
        assert!(matches!(verdict, Verdict::Broken(_)), "{verdict:?}");
    }
}

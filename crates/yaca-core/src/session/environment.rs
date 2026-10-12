//! What the model cannot know on its own: today's date and the state of the
//! working tree.
//!
//! Without a date a model takes the newest version it was trained on for the
//! latest one, and without the branch and the status it opens every session by
//! spending a turn on `git status`.
//!
//! This is a note on the first user message, not part of the system prompt:
//! the prompt is the cached prefix of every request (`prompt`), and a date in
//! it would void the cache at midnight while a status would void it on every
//! edit. In the first message it is written once, goes into the journal with
//! the message, and a reopened session reads the very same text back — so the
//! prefix never moves. The price is that it ages: the status is the tree as it
//! was when the session started, and the note says so.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::isolation::Isolation;

/// Past this the list is a wall of text the model has to read on every request
/// of the session; the count of the rest says enough.
const MAX_STATUS_LINES: usize = 40;

/// A slow repository must not hold up the first answer of the session.
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

/// The note, ready to go in front of the human's first message.
pub(super) async fn note(
    root: &Path,
    isolation: Option<&Isolation>,
    additional: &[String],
) -> String {
    render(
        local_date(),
        git_status(root).await.as_deref(),
        isolation,
        additional,
    )
}

/// Split from [`note`] so that the shape can be tested without a clock or a
/// repository.
fn render(
    date: Option<String>,
    status: Option<&str>,
    isolation: Option<&Isolation>,
    additional: &[String],
) -> String {
    let mut s = String::from("<environment_context>\n");
    if let Some(date) = date {
        s.push_str(&format!("Today's date: {date}\n"));
    }
    if let Some(status) = status {
        s.push_str(&git_section(status));
    }
    if !additional.is_empty() {
        s.push_str(&additional_section(additional));
    }
    if let Some(isolation) = isolation {
        s.push_str(&isolation.note());
    }
    s.push_str("</environment_context>");
    s
}

/// The folders beside the project. Model-facing, so English. Nothing is
/// written here when there are none, which keeps a single-root session's note
/// as it was.
fn additional_section(roots: &[String]) -> String {
    let mut s = String::from(
        "Additional folders in this session. Address files in them by their \
         absolute path: a relative path always means the project root. A \
         rollback does not cover them.\n",
    );
    for root in roots {
        s.push_str(&format!("- {root}\n"));
    }
    s
}

fn git_section(status: &str) -> String {
    let mut lines = status.lines();
    // `--branch` puts the branch first, as `## name...upstream [ahead 1]`.
    let branch = lines
        .next()
        .and_then(|l| l.strip_prefix("## "))
        .unwrap_or("unknown");
    // Yaca's own folder: it appears the moment the session opens, in every
    // project that does not ignore it, and a model told about it offers to
    // add it to `.gitignore`.
    let changes: Vec<&str> = lines.filter(|l| *l != "?? .yaca/").collect();

    let mut s = format!("Git branch: {branch}\n");
    if changes.is_empty() {
        s.push_str("Working tree: clean at the start of the session.\n");
        return s;
    }
    s.push_str(
        "Working tree at the start of the session (it does not update; run \
         `git status` for the current state):\n",
    );
    for line in changes.iter().take(MAX_STATUS_LINES) {
        s.push_str(line);
        s.push('\n');
    }
    if changes.len() > MAX_STATUS_LINES {
        s.push_str(&format!(
            "… and {} more\n",
            changes.len() - MAX_STATUS_LINES
        ));
    }
    s
}

/// `git status --short --branch` in the project root, or `None` where it is not
/// a repository, git is missing or it did not answer in time. The note is
/// then just the date: a missing status is not worth an error in the feed.
async fn git_status(root: &Path) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args([
        // Another process (the human's editor, `git gc`) may be using the
        // index; this is a read, and it must not take its lock or refresh it.
        "--no-optional-locks",
        // Else a Cyrillic file name arrives as an octal escape the model
        // then copies into a path.
        "-c",
        "core.quotepath=false",
        "status",
        "--short",
        "--branch",
    ])
    .current_dir(root)
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);

    let out = tokio::time::timeout(GIT_TIMEOUT, cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The date where the human is, not in UTC: a note saying "tomorrow" at ten
/// in the evening sends the model to the wrong file in a dated folder.
#[cfg(windows)]
fn local_date() -> Option<String> {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    // SAFETY: `GetLocalTime` only writes the struct it is given, and an
    // all-zero `SYSTEMTIME` is a valid value of it.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    Some(format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay))
}

#[cfg(unix)]
fn local_date() -> Option<String> {
    // SAFETY: `localtime_r` writes only to the `tm` it is given, and an
    // all-zero `tm` is a valid value of it. The time is read from the same
    // call that gets it, so there is no `time_t` to go stale.
    let tm = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return None;
        }
        tm
    };
    Some(format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_tree_is_said_to_be_clean() {
        let s = render(Some("2026-10-11".into()), Some("## master...origin/master\n"), None, &[]);
        assert!(s.contains("Today's date: 2026-10-11"));
        assert!(s.contains("Git branch: master...origin/master"));
        assert!(s.contains("clean"));
    }

    #[test]
    fn changes_are_listed_and_marked_as_a_snapshot() {
        let s = render(None, Some("## feat\n M src/a.rs\n?? b.txt\n"), None, &[]);
        assert!(!s.contains("Today's date"));
        assert!(s.contains("Git branch: feat\n"));
        assert!(s.contains(" M src/a.rs\n?? b.txt\n"));
        assert!(s.contains("does not update"));
    }

    #[test]
    fn the_session_folder_is_not_reported_as_a_change() {
        let s = render(None, Some("## main
?? .yaca/
"), None, &[]);
        assert!(s.contains("clean"), "{s}");
        let s = render(None, Some("## main
?? .yaca/
 M a.rs
"), None, &[]);
        assert!(!s.contains(".yaca"), "{s}");
    }

    #[test]
    fn a_long_status_is_cut_with_the_count_of_the_rest() {
        let mut status = String::from("## main\n");
        for i in 0..MAX_STATUS_LINES + 7 {
            status.push_str(&format!(" M f{i}.rs\n"));
        }
        let s = render(None, Some(&status), None, &[]);
        assert!(s.contains("… and 7 more"));
        assert!(!s.contains(&format!("f{}.rs", MAX_STATUS_LINES)));
    }

    #[test]
    fn outside_a_repository_only_the_date_is_left() {
        let s = render(Some("2026-10-11".into()), None, None, &[]);
        assert!(!s.contains("Git"));
        assert!(s.contains("2026-10-11"));
    }

    #[test]
    fn additional_folders_are_listed_by_absolute_path_and_only_when_there_are_some() {
        let roots = vec!["C:/other/lib".to_string()];
        let s = render(None, None, None, &roots);
        assert!(s.contains("Additional folders"), "{s}");
        assert!(s.contains("absolute path"), "{s}");
        assert!(s.contains("- C:/other/lib\n"), "{s}");
        // Nothing else changes: a single-root note has no such section at all.
        let plain = render(None, None, None, &[]);
        assert!(!plain.contains("Additional folders"), "{plain}");
    }

    #[test]
    fn an_isolated_chat_is_told_which_branch_is_its_own() {
        let iso = Isolation {
            branch: "yaca/ses_1".into(),
            path: "x".into(),
        };
        let s = render(None, None, Some(&iso), &[]);
        assert!(s.contains("`yaca/ses_1`"), "{s}");
    }

    #[test]
    fn the_local_date_looks_like_a_date() {
        let date = local_date().expect("the platform knows the time");
        let parts: Vec<&str> = date.split('-').collect();
        assert_eq!(parts.len(), 3, "{date}");
        assert_eq!(parts[0].len(), 4, "{date}");
        assert!(parts[0].parse::<u32>().unwrap() >= 2025, "{date}");
    }
}

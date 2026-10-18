//! The git a marketplace and a plugin are fetched with.
//!
//! The `git` program, as everywhere else in the core (`isolation`): it brings
//! the human's credentials, proxies and SSH keys, which a library would have to
//! be taught. Never asked anything: a prompt for a password would hang an
//! install that nobody is watching, so prompts are turned off and a repository
//! that needs one fails with git's own words.
//!
//! Shallow, one commit: a marketplace is read, not developed, and the history
//! of a plugin is not part of the plugin.

use std::path::Path;
use std::process::{Command, Output};

/// Runs git in `dir` and returns its stdout, or the first line of what it said
/// went wrong.
pub(super) fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = run(dir, args).map_err(|e| format!("git cannot be started: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("git exited with an error");
        Err(format!("git: {line}"))
    }
}

fn run(dir: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "")
        .stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
    }
    cmd.output()
}

/// `revision` of `url` checked out in `dir` (created, must not exist), with
/// one commit of history. `None` means the default branch. Returns the
/// commit's sha.
pub(super) fn fetch(url: &str, revision: Option<&str>, dir: &Path) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    git(dir, &["init", "--quiet"])?;
    // A sha is fetched by itself; servers that allow it (GitHub does) give
    // exactly that commit, and the shallow depth keeps it one commit.
    let what = revision.unwrap_or("HEAD");
    git(dir, &["fetch", "--quiet", "--depth", "1", url, what])?;
    git(dir, &["checkout", "--quiet", "--detach", "FETCH_HEAD"])?;
    git(dir, &["rev-parse", "HEAD"])
}

/// The commit `dir` is at, if it is a repository.
pub(super) fn head(dir: &Path) -> Option<String> {
    git(dir, &["rev-parse", "HEAD"]).ok()
}

/// Copies the tree at `from` to `to` (which must not exist), without `.git`.
/// Symbolic links are left out: what one points at may be outside the
/// package, and a link inside the copy would point at the download that is
/// about to be deleted.
pub(super) fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let read = std::fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?;
    for entry in read.flatten() {
        if entry.file_name() == ".git" {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &target).map_err(|e| format!("cannot copy {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Removes a directory the installer made, read-only files and all: git marks
/// its objects read-only on Windows, and `remove_dir_all` stops at the first.
pub(super) fn remove_tree(dir: &Path) {
    if !dir.exists() {
        return;
    }
    if std::fs::remove_dir_all(dir).is_ok() {
        return;
    }
    clear_read_only(dir);
    let _ = std::fs::remove_dir_all(dir);
}

fn clear_read_only(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if let Ok(meta) = entry.metadata() {
            let mut perms = meta.permissions();
            if perms.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = std::fs::set_permissions(&path, perms);
            }
            if meta.is_dir() {
                clear_read_only(&path);
            }
        }
    }
}

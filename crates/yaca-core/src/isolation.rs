//! A git worktree of one chat, so that two chats of a project do not share a
//! working tree.
//!
//! Without it every session of a project edits the same files and snapshots
//! into the same tree, and a rollback in one hands back a tree the other had
//! been changing. With it a chat works in `.yaca/worktrees/<session>` on its
//! own branch, `yaca/<session>`, and its checkpoints (§5.8) walk and restore
//! only that directory.
//!
//! Three decisions shape it.
//!
//! *It starts from `HEAD`.* What the human has not committed stays in the main
//! tree, and so do untracked files and whatever is not under version control
//! (`node_modules`). That is the price of a branch that can be merged with
//! plain git, and it is why isolation is something a caller asks for rather
//! than the default.
//!
//! *Where there is nothing to branch from, it fails.* A project that is not a
//! repository, or has no commit yet, cannot be isolated, and a session that
//! quietly fell back to the shared tree would be lying about the one thing it
//! was asked for.
//!
//! *The work is committed on the chat's branch at the end of each turn.* Taking
//! it back is then `git merge yaca/<session>` by whoever wants it; the core
//! does not merge, because a merge can conflict and a conflict needs a human.
//! The commits are authored by Yaca and skip the repository's hooks: they are a
//! save point on a throwaway branch, and a failing linter must not be what
//! stands between the chat and its work.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::CoreError;
use crate::id::SessionId;

/// Where a chat works, as the interface shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Isolation {
    /// The chat's branch. Merging it is how the work comes back.
    pub branch: String,
    /// The directory the chat's tools run in: the worktree, or the project's
    /// own folder inside it when the project is a subdirectory of the
    /// repository.
    pub path: String,
}

impl Isolation {
    /// A line for the model: it has to know which tree it stands in, or it
    /// goes looking for the main one and switches branches.
    pub(crate) fn note(&self) -> String {
        format!(
            "Isolated worktree: this chat works on its own branch `{}` in a separate \
             checkout. Do not switch branches or edit the main working tree. Your \
             changes are committed to this branch at the end of each turn.\n",
            self.branch
        )
    }
}

/// Where the worktree of `id` is.
fn worktree_of(home: &Path, id: &SessionId) -> PathBuf {
    crate::project::dir(home).join("worktrees").join(id.as_str())
}

fn branch_of(id: &SessionId) -> String {
    format!("yaca/{id}")
}

/// The worktree of session `id` in the project at `home`.
///
/// `create` is whether to make one when there is none. A session that already
/// has its worktree gets it back either way — resuming a chat that was
/// isolated has to land in the tree its work is in. `Ok(None)`: none, and none
/// asked for.
pub fn open(home: &Path, id: &SessionId, create: bool) -> Result<Option<Isolation>, CoreError> {
    let worktree = worktree_of(home, id);
    let exists = worktree.join(".git").exists();
    if !exists && !create {
        return Ok(None);
    }

    // Run from the project's folder: it may be a subdirectory of the
    // repository, and the prefix is how the chat lands in the same
    // subdirectory of its own checkout.
    let prefix = git(home, &["rev-parse", "--show-prefix"])
        .map_err(|_| CoreError::Config("проект не под git: изолировать чат нечем".into()))?;

    if !exists {
        // Relative to `home`, which is where git runs: a verbatim `\\?\` path
        // of a canonical root is not one every git build takes.
        let target = format!("{}/worktrees/{id}", crate::project::DIR);
        let branch = branch_of(id);

        git(home, &["rev-parse", "--verify", "--quiet", "HEAD"]).map_err(|_| {
            CoreError::Config("в проекте нет ни одного коммита: изолировать чат не от чего".into())
        })?;

        let branch_exists = git(
            home,
            &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")],
        )
        .is_ok();
        let made = if branch_exists {
            // The folder was removed by hand; git still remembers it.
            let _ = git(home, &["worktree", "prune"]);
            git(home, &["worktree", "add", "--quiet", &target, &branch])
        } else {
            git(home, &["worktree", "add", "--quiet", "-b", &branch, &target, "HEAD"])
        };
        made.map_err(|e| CoreError::Config(format!("не создать worktree чата: {e}")))?;
    }

    let tree = if prefix.is_empty() {
        worktree
    } else {
        worktree.join(prefix)
    };
    Ok(Some(Isolation {
        branch: branch_of(id),
        path: tree.to_string_lossy().into_owned(),
    }))
}

/// Commits what the chat changed in `tree`. `Ok(false)`: nothing to commit.
pub fn commit_all(tree: &Path, subject: &str) -> Result<bool, String> {
    // Yaca's own folder is not the chat's work, and `.gitignore` of the
    // project may not know about it.
    git(tree, &["add", "-A", "--", ".", ":(exclude).yaca"])?;

    let staged = run(tree, &["diff", "--cached", "--quiet"], false).map_err(|e| e.to_string())?;
    if staged.status.success() {
        return Ok(false);
    }

    let line: String = subject.split_whitespace().collect::<Vec<_>>().join(" ");
    let line: String = line.chars().take(72).collect();
    let message = if line.is_empty() {
        "yaca: правки чата".to_string()
    } else {
        format!("yaca: {line}")
    };
    let out = run(tree, &["commit", "--quiet", "--no-verify", "-m", &message], true)
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(true)
    } else {
        Err(first_line(&out))
    }
}

/// Stdout of a git command, trimmed; the first line of stderr when it failed.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = run(dir, args, false).map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(first_line(&out))
    }
}

fn first_line(out: &Output) -> String {
    let text = String::from_utf8_lossy(&out.stderr);
    let text = if text.trim().is_empty() {
        String::from_utf8_lossy(&out.stdout)
    } else {
        text
    };
    text.lines().next().unwrap_or("git failed").trim().to_string()
}

fn run(dir: &Path, args: &[&str], as_yaca: bool) -> std::io::Result<Output> {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null());
    if as_yaca {
        cmd.env("GIT_AUTHOR_NAME", "Yaca")
            .env("GIT_AUTHOR_EMAIL", "yaca@localhost")
            .env("GIT_COMMITTER_NAME", "Yaca")
            .env("GIT_COMMITTER_EMAIL", "yaca@localhost");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
    }
    cmd.output()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// A repository with one commit, committed as somebody who is not Yaca.
    fn repo() -> TempDir {
        let dir = TempDir::new();
        dir.write("a.txt", "one\n");
        for args in [
            vec!["init", "-q"],
            vec!["add", "-A"],
            vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "init"],
        ] {
            let ok = run(dir.path(), &args, false).unwrap().status.success();
            assert!(ok, "git {args:?}");
        }
        dir
    }

    #[test]
    fn a_chat_gets_a_checkout_of_its_own_on_its_own_branch() {
        let dir = repo();
        let id = SessionId::new();

        let iso = open(dir.path(), &id, true).unwrap().unwrap();
        assert_eq!(iso.branch, format!("yaca/{id}"));

        let tree = PathBuf::from(&iso.path);
        // `autocrlf` may have turned the checkout's line ends into CRLF.
        let seen = std::fs::read_to_string(tree.join("a.txt")).unwrap();
        assert_eq!(seen.replace("\r\n", "\n"), "one\n");

        std::fs::write(tree.join("a.txt"), "two\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "one\n",
            "the main tree is not the chat's"
        );
    }

    #[test]
    fn the_work_is_committed_to_the_chats_branch_and_nowhere_else() {
        let dir = repo();
        let id = SessionId::new();
        let iso = open(dir.path(), &id, true).unwrap().unwrap();
        let tree = PathBuf::from(&iso.path);

        assert!(!commit_all(&tree, "nothing yet").unwrap());

        std::fs::write(tree.join("b.txt"), "new\n").unwrap();
        // Yaca's own folder is never part of it.
        std::fs::create_dir_all(tree.join(".yaca")).unwrap();
        std::fs::write(tree.join(".yaca/log"), "x").unwrap();
        assert!(commit_all(&tree, "add b").unwrap());

        let listed = git(dir.path(), &["ls-tree", "-r", "--name-only", &iso.branch]).unwrap();
        assert!(listed.lines().any(|l| l == "b.txt"), "{listed}");
        assert!(!listed.contains(".yaca"), "{listed}");
        let main = git(dir.path(), &["log", "-1", "--format=%s"]).unwrap();
        assert_eq!(main, "init", "the project's own branch did not move");
    }

    #[test]
    fn a_resumed_chat_lands_in_the_checkout_it_left_even_unasked() {
        let dir = repo();
        let id = SessionId::new();
        let first = open(dir.path(), &id, true).unwrap().unwrap();

        let again = open(dir.path(), &id, false).unwrap().unwrap();
        assert_eq!(again, first);

        let stranger = SessionId::new();
        assert!(open(dir.path(), &stranger, false).unwrap().is_none());
    }

    #[test]
    fn a_checkout_removed_by_hand_is_made_again_on_the_same_branch() {
        let dir = repo();
        let id = SessionId::new();
        let iso = open(dir.path(), &id, true).unwrap().unwrap();
        let tree = PathBuf::from(&iso.path);
        std::fs::write(tree.join("b.txt"), "kept\n").unwrap();
        commit_all(&tree, "keep").unwrap();
        std::fs::remove_dir_all(&tree).unwrap();

        let back = open(dir.path(), &id, true).unwrap().unwrap();
        let seen = std::fs::read_to_string(Path::new(&back.path).join("b.txt")).unwrap();
        assert_eq!(seen.replace("\r\n", "\n"), "kept\n");
    }

    #[test]
    fn a_project_outside_git_is_refused_not_shared() {
        let dir = TempDir::new();
        dir.write("a.txt", "x\n");
        let err = open(dir.path(), &SessionId::new(), true).unwrap_err();
        assert!(err.to_string().contains("git"), "{err}");
    }

    #[test]
    fn a_repository_without_a_commit_is_refused() {
        let dir = TempDir::new();
        assert!(run(dir.path(), &["init", "-q"], false).unwrap().status.success());
        let err = open(dir.path(), &SessionId::new(), true).unwrap_err();
        assert!(err.to_string().contains("коммит"), "{err}");
    }

    #[test]
    fn a_project_inside_a_repository_lands_in_the_same_subdirectory() {
        let dir = TempDir::new();
        dir.write("app/a.txt", "one\n");
        for args in [
            vec!["init", "-q"],
            vec!["add", "-A"],
            vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "init"],
        ] {
            assert!(run(dir.path(), &args, false).unwrap().status.success());
        }
        let home = dir.path().join("app");

        let iso = open(&home, &SessionId::new(), true).unwrap().unwrap();
        assert!(Path::new(&iso.path).join("a.txt").exists(), "{}", iso.path);
    }
}

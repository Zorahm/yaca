//! Finding the git commands that §8 keeps behind the human's own word.
//!
//! Committing and pushing without an explicit command are forbidden in every
//! mode, bypass included. The core cannot forbid them outright, because the
//! human may ask for them; it can make sure every one of them is put to the
//! human first. That is what this module decides.
//!
//! The search is deliberately wide. A `git push` is found wherever a word is
//! `git` (or `git.exe`, or a path to either), in any simple command, after any
//! separator, inside `sh -c "…"`, inside `$(…)`, and behind env prefixes. A
//! word that looks like a command line of its own is read as one. A false
//! alarm costs one question; a missed push runs with nobody asked.
//!
//! The same section of §8 covers writes into git's own configuration, which
//! run programs when git reads them: see [`of_path`].
//!
//! What is not seen: a subcommand the shell produces from a variable (reported
//! as [`Invariant::GitUnresolved`] when it is visible), a git alias defined in
//! the configuration, and a script that runs git. Those are named in the
//! README as open, not papered over here.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::cmdline::{parse, Simple, Word, DIALECTS};
use super::Invariant;

/// How deep a word that looks like a command line is read. A string inside a
/// string inside a string is not something a person typed, and the bound keeps
/// a hostile line from costing unbounded work.
const MAX_DEPTH: u32 = 4;

/// Options of `git` that take a value in the next word. Without this list the
/// value of `-C` would be taken for the subcommand.
const VALUE_OPTIONS: [&str; 7] = [
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
];

/// The invariant this command runs into, if it runs into one.
pub fn of_command(command: &str) -> Option<Invariant> {
    scan(command, 0)
}

fn scan(line: &str, depth: u32) -> Option<Invariant> {
    for dialect in DIALECTS {
        for simple in &parse(line, dialect).commands {
            if let Some(found) = simple_hit(simple, depth) {
                return Some(found);
            }
        }
    }
    None
}

fn simple_hit(simple: &Simple, depth: u32) -> Option<Invariant> {
    for (idx, word) in simple.words.iter().enumerate() {
        if depth < MAX_DEPTH && looks_like_command_line(&word.text) {
            if let Some(found) = scan(&word.text, depth + 1) {
                return Some(found);
            }
        }
        if is_git(&word.text) {
            if let Some(found) = git_subcommand(&simple.words[idx + 1..]) {
                return Some(found);
            }
        }
    }

    // `$g push`: the program is not a name the parser can read, and an argument
    // that is a git verb sits next to it. Better a question than a silence.
    if let Some(first) = simple.words.first() {
        let unknown_program = first.hazard || first.text.is_empty();
        let verb_argument = simple
            .words
            .iter()
            .skip(1)
            .any(|w| matches!(w.text.as_str(), "push" | "commit"));
        if unknown_program && verb_argument {
            return Some(Invariant::GitUnresolved);
        }
    }
    None
}

/// A word with a space or a separator in it is, to a shell that re-reads it
/// (`sh -c`, `powershell -Command`, `eval`), a command line.
fn looks_like_command_line(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_whitespace() || ";|&(){}`".contains(c))
}

/// `git`, `git.exe`, `/usr/bin/git`, `C:\Program Files\Git\cmd\git.exe`.
fn is_git(text: &str) -> bool {
    let base = text.rsplit(['/', '\\']).next().unwrap_or(text);
    let lower = base.to_ascii_lowercase();
    lower == "git" || lower == "git.exe"
}

/// The subcommand of a `git` invocation, when it is commit or push. Global
/// options come first and may carry a value; the first other word is the verb.
fn git_subcommand(args: &[Word]) -> Option<Invariant> {
    let mut iter = args.iter();
    while let Some(word) = iter.next() {
        // A verb that the shell produces, or a glob that may name one.
        if word.hazard {
            return Some(Invariant::GitUnresolved);
        }
        let text = word.text.as_str();
        if text.starts_with('-') {
            if VALUE_OPTIONS.contains(&text) {
                iter.next();
            }
            continue;
        }
        return match text {
            "push" => Some(Invariant::GitPush),
            "commit" => Some(Invariant::GitCommit),
            _ => None,
        };
    }
    None
}

/// Links followed before the walk gives up. A chain longer than this is not a
/// layout a person made, and the check then falls back to the spelling.
const MAX_LINKS: u32 = 8;

/// The invariant a write to `path` runs into, if it is git's own configuration
/// (`Invariant::GitConfig`). `root` is the project root and `path` an absolute
/// path inside it, as `Workspace::resolve_new` gives.
///
/// Two spellings are checked, because the write follows links: the path as
/// the model named it, and the place it really lands. A link inside the
/// project that points into `.git` is a write to `.git` under another name.
///
/// Case and trailing dots or spaces are folded the way NTFS folds them: on
/// Windows `.GIT\config` and `.git.\config` are the repository's own
/// configuration, and git reads them as such.
pub fn of_path(root: &Path, path: &Path) -> Option<Invariant> {
    let lexical = components_below(root, path);
    let real = components_below(root, &real_path(path, MAX_LINKS));
    [lexical, real]
        .into_iter()
        .flatten()
        .any(|parts| is_git_config(&parts))
        .then_some(Invariant::GitConfig)
}

/// A `.git` anywhere on the way (a repository, a worktree's pointer file), or
/// a `.gitattributes` or `.gitmodules` file in any directory.
fn is_git_config(parts: &[String]) -> bool {
    parts.iter().any(|part| fold_name(part) == ".git")
        || parts.last().is_some_and(|last| {
            matches!(fold_name(last).as_str(), ".gitattributes" | ".gitmodules")
        })
}

/// The name as NTFS compares it: lowercase, without the trailing dots and
/// spaces Win32 drops. A no-op on other systems for every name git uses.
fn fold_name(name: &str) -> String {
    name.trim_end_matches(['.', ' ']).to_ascii_lowercase()
}

/// The parts of `path` below `root`, or `None` when it is not below it.
fn components_below(root: &Path, path: &Path) -> Option<Vec<String>> {
    let rel = path.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .filter_map(|c| c.as_os_str().to_str().map(str::to_owned))
            .collect(),
    )
}

/// Where `path` really lands once the links on the way are followed. On
/// Windows this also expands an 8.3 short name such as `GIT~1`, which
/// `canonicalize` resolves to the long name, so a short spelling of `.git`
/// does not get past the check either. A link that does not exist yet is
/// followed by its text, as a write through it would be. Anything that cannot
/// be resolved stays as it was spelled.
fn real_path(path: &Path, links_left: u32) -> PathBuf {
    // The parts below the first existing ancestor, innermost last.
    let mut rest: Vec<OsString> = Vec::new();
    let mut anchor = path.to_path_buf();
    loop {
        match std::fs::symlink_metadata(&anchor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let (Ok(target), Some(parent)) = (std::fs::read_link(&anchor), anchor.parent())
                else {
                    return path.to_path_buf();
                };
                if links_left == 0 {
                    return path.to_path_buf();
                }
                let target = if target.is_absolute() {
                    target
                } else {
                    parent.join(target)
                };
                let joined = rest.iter().rev().fold(target, |acc, part| acc.join(part));
                return real_path(&lexical(&joined), links_left - 1);
            }
            Ok(_) => {
                let base = std::fs::canonicalize(&anchor).unwrap_or(anchor);
                return rest.iter().rev().fold(base, |acc, part| acc.join(part));
            }
            Err(_) => {
                let (Some(parent), Some(name)) = (anchor.parent(), anchor.file_name()) else {
                    return path.to_path_buf();
                };
                rest.push(name.to_os_string());
                anchor = parent.to_path_buf();
            }
        }
    }
}

/// Folds `.` and `..` without touching the disk, for a link target that does
/// not exist yet.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_or_commit(command: &str) -> bool {
        matches!(
            of_command(command),
            Some(Invariant::GitPush | Invariant::GitCommit)
        )
    }

    #[test]
    fn plain_push_and_commit_are_found() {
        assert_eq!(of_command("git push"), Some(Invariant::GitPush));
        assert_eq!(of_command("git push origin main"), Some(Invariant::GitPush));
        assert_eq!(of_command("git commit -m fix"), Some(Invariant::GitCommit));
    }

    #[test]
    fn global_options_do_not_hide_the_verb() {
        assert_eq!(of_command("git -C ../other push"), Some(Invariant::GitPush));
        assert_eq!(
            of_command("git -c user.name=x commit -m m"),
            Some(Invariant::GitCommit)
        );
        assert_eq!(
            of_command("git --no-pager --git-dir=.git push"),
            Some(Invariant::GitPush)
        );
        assert_eq!(
            of_command("git --work-tree ../w push"),
            Some(Invariant::GitPush)
        );
    }

    #[test]
    fn a_push_after_any_separator_is_found() {
        for line in [
            "ls && git push",
            "ls; git push",
            "ls || git push",
            "cat x | git push",
            "ls & git push",
            "ls\ngit push",
            "(git push)",
            "{ git push; }",
            "echo $(git push)",
            "echo `git push`",
            "ls &git push",
        ] {
            assert!(push_or_commit(line), "{line}");
        }
    }

    #[test]
    fn a_push_inside_a_shell_string_is_found() {
        for line in [
            "sh -c \"git push\"",
            "bash -lc 'cd x && git push'",
            "powershell -Command \"git push\"",
            "pwsh -c git commit -m x",
            "cmd /c git push",
            "eval \"git push\"",
            "xargs git push",
            "env GIT_TRACE=1 git push",
            "FOO=1 git push",
            "nohup git push &",
        ] {
            assert!(push_or_commit(line), "{line}");
        }
    }

    #[test]
    fn quoting_and_paths_do_not_hide_the_program() {
        for line in [
            "\"git\" push",
            "'git' push",
            "g''it push",
            "git.exe push",
            "/usr/bin/git push",
            "C:\\Program Files\\Git\\cmd\\git.exe push",
            "& 'C:\\Program Files\\Git\\cmd\\git.exe' push",
            "{git push}",
        ] {
            assert!(push_or_commit(line), "{line}");
        }
    }

    #[test]
    fn a_push_is_found_under_powershell_rules_too() {
        // The backslash escapes the quote in POSIX and opens a string in
        // PowerShell; the push is outside the string under PowerShell.
        assert!(push_or_commit("echo \\\" ; git push ; echo \\\""));
    }

    #[test]
    fn other_git_verbs_are_not_invariants() {
        for line in [
            "git status",
            "git log --oneline",
            "git diff HEAD",
            "git pull",
            "git add .",
            "git stash",
            "git branch --list",
            "git checkout -b x",
            "git remote -v",
        ] {
            assert_eq!(of_command(line), None, "{line}");
        }
        assert_eq!(of_command("cargo test"), None);
        assert_eq!(of_command("echo pushing the button"), None);
    }

    #[test]
    fn a_verb_the_shell_produces_is_unresolved_not_missed() {
        assert_eq!(of_command("git $verb"), Some(Invariant::GitUnresolved));
        assert_eq!(of_command("git pu*"), Some(Invariant::GitUnresolved));
        assert_eq!(of_command("$g push"), Some(Invariant::GitUnresolved));
    }

    #[test]
    fn a_quoted_mention_is_asked_about_on_purpose() {
        // Conservative on purpose: the parser cannot tell a mention in a search
        // pattern from a command, and a question about a grep is cheap.
        assert!(push_or_commit("rg 'git push' docs"));
    }
}

#[cfg(test)]
mod git_config_tests {
    use super::*;
    use crate::testutil::TempDir;

    fn config_of(dir: &TempDir, rel: &str) -> Option<Invariant> {
        of_path(dir.path(), &dir.path().join(rel))
    }

    #[test]
    fn git_directory_and_pointer_files_are_git_config() {
        let dir = TempDir::new();
        dir.write(".git/config", "[core]\n");
        for rel in [
            ".git/config",
            ".git/hooks/pre-commit",
            ".git/info/attributes",
            // A submodule or a linked worktree keeps its own `.git`.
            "vendor/lib/.git/config",
            // A worktree's `.git` is a file, not a directory.
            ".git",
        ] {
            assert_eq!(config_of(&dir, rel), Some(Invariant::GitConfig), "{rel}");
        }
    }

    #[test]
    fn attribute_and_module_files_are_git_config_wherever_they_are() {
        let dir = TempDir::new();
        for rel in [
            ".gitattributes",
            "docs/.gitattributes",
            ".gitmodules",
            "a/b/.gitmodules",
        ] {
            assert_eq!(config_of(&dir, rel), Some(Invariant::GitConfig), "{rel}");
        }
    }

    #[test]
    fn the_name_is_compared_the_way_ntfs_compares_it() {
        let dir = TempDir::new();
        for rel in [
            ".GIT/config",
            ".git./config",
            ".Git /hooks/x",
            ".GitAttributes",
        ] {
            assert_eq!(config_of(&dir, rel), Some(Invariant::GitConfig), "{rel}");
        }
    }

    #[test]
    fn ordinary_git_adjacent_names_are_not_git_config() {
        let dir = TempDir::new();
        for rel in [
            "src/main.rs",
            ".gitignore",
            ".github/workflows/ci.yml",
            "git/config",
            "docs/git.md",
            // A file that merely mentions git in its name, not a git directory.
            ".gitattributes.bak/notes",
        ] {
            assert_eq!(config_of(&dir, rel), None, "{rel}");
        }
    }

    #[test]
    fn a_link_that_points_into_git_is_git_config_under_its_own_name() {
        let dir = TempDir::new();
        dir.write(".git/config", "[core]\n");
        let target = dir.path().join(".git/config");
        let link = dir.path().join("notes.txt");
        make_file_link(&target, &link).expect("the test host can make a file link");
        assert_eq!(config_of(&dir, "notes.txt"), Some(Invariant::GitConfig));
    }

    #[test]
    fn a_directory_link_into_git_is_git_config_for_what_is_written_through_it() {
        let dir = TempDir::new();
        dir.write(".git/config", "[core]\n");
        let link = dir.path().join("cfg");
        make_dir_link(&dir.path().join(".git"), &link).expect("the test host can make a dir link");
        // The name is `cfg/config`, the write lands in `.git/config`.
        assert_eq!(config_of(&dir, "cfg/config"), Some(Invariant::GitConfig));
        assert_eq!(config_of(&dir, "cfg/new-file"), Some(Invariant::GitConfig));
    }

    #[test]
    fn an_ordinary_link_is_not_git_config() {
        let dir = TempDir::new();
        dir.write("src/lib.rs", "fn a() {}\n");
        make_file_link(&dir.path().join("src/lib.rs"), &dir.path().join("alias.rs"))
            .expect("the test host can make a file link");
        assert_eq!(config_of(&dir, "alias.rs"), None);
    }

    #[test]
    fn a_dangling_link_is_followed_by_its_text() {
        // Writing through it creates the file it names, so the text decides.
        let dir = TempDir::new();
        dir.mkdir(".git");
        let link = dir.path().join("later.txt");
        make_file_link(&dir.path().join(".git/config"), &link)
            .expect("the test host can make a file link");
        assert_eq!(config_of(&dir, "later.txt"), Some(Invariant::GitConfig));
    }

    /// Links are the case under test, so a host that cannot make one fails
    /// the test rather than letting it pass without checking anything.
    fn make_file_link(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_file(target, link);
    }

    fn make_dir_link(target: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link);
        #[cfg(windows)]
        return std::os::windows::fs::symlink_dir(target, link);
    }
}

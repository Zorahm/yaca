//! Commands that change nothing, which Yaca runs without asking (§7.1, §8).
//!
//! The core cannot tell `cargo tree` from `rm -rf` by looking at a name, so
//! this is an allow-list, and the default is "not read-only". A command is
//! read-only only when every simple command in it is a listed program whose
//! arguments cannot make it write. Anything the parser is unsure of is not
//! read-only: a redirection, an expansion, a glob, a quote that never closes,
//! a program named by a path (that path could be a script the model just
//! wrote), and a program shadowed by a file of the same name in the project. A
//! wrong "no" costs one question; a wrong "yes" runs a write unasked.
//!
//! Left out on purpose: `cargo` (it writes `Cargo.lock` and `target/`), `sed`
//! and `sort` (their flags write files), `tee`, `xargs`, and everything that
//! runs another program. `git` is listed with its read-only subcommands only,
//! and no global options, because `-c` and `-C` reach configuration that runs
//! programs and other repositories.
//!
//! A read-only `git` still reads the configuration on disk, and that
//! configuration can run programs (`core.fsmonitor`, textconv and diff
//! drivers). This classifier does not look at it, and does not need to. A
//! write through the file tools into `.git/`, or to `.gitattributes` or
//! `.gitmodules`, is an invariant ([`Invariant::GitConfig`](super::Invariant::GitConfig))
//! asked in every mode. A shell command that writes there is not read-only
//! (it redirects or runs a program), so it is asked as a command. Either way
//! the configuration is never put in place unasked.

use std::path::Path;

use super::cmdline::{parse, Simple, Word, DIALECTS};

/// Programs whose arguments do not change what they do.
const PLAIN_READERS: [&str; 19] = [
    "pwd",
    "echo",
    "cat",
    "type",
    "head",
    "tail",
    "wc",
    "ls",
    "dir",
    "gci",
    "get-childitem",
    "gc",
    "get-content",
    "get-location",
    "gl",
    "select-string",
    "sls",
    "findstr",
    "grep",
];

/// `find` actions that run something or write something.
const FIND_ACTIONS: [&str; 5] = ["-exec", "-ok", "-delete", "-fprint", "-fls"];

/// `rg` options that run a program named in the argument.
const RG_EXECUTES: [&str; 3] = ["--pre", "--pre-glob", "--hostname-bin"];

/// `git` subcommands that only read: no option of theirs writes a file.
const GIT_READERS: [&str; 9] = [
    "status",
    "rev-parse",
    "ls-files",
    "describe",
    "blame",
    "shortlog",
    "log",
    "show",
    "diff",
];

/// Extensions a Windows shell would run a bare name from.
const RUNNABLE_EXTENSIONS: [&str; 6] = ["", ".exe", ".bat", ".cmd", ".com", ".ps1"];

/// Whether the command, run in one of `dirs`, changes nothing. `dirs` are the
/// directories a program name could be resolved in before `PATH`: the working
/// directory and the project root.
pub fn is_read_only_in(command: &str, dirs: &[&Path]) -> bool {
    DIALECTS.iter().all(|&dialect| {
        let line = parse(command, dialect);
        line.closed
            && line
                .commands
                .iter()
                .all(|simple| simple_is_read_only(simple) && !shadowed(simple, dirs))
    })
}

fn simple_is_read_only(simple: &Simple) -> bool {
    if simple.hazard {
        return false;
    }
    let Some(first) = simple.words.first() else {
        return true;
    };
    let Some(name) = program_name(&first.text) else {
        return false;
    };
    let args = &simple.words[1..];
    match name.as_str() {
        "git" => git_is_read_only(args),
        "find" => !args
            .iter()
            .any(|a| FIND_ACTIONS.iter().any(|action| a.text.starts_with(action))),
        "rg" => !args.iter().any(|a| {
            RG_EXECUTES
                .iter()
                .any(|opt| a.text == *opt || a.text.starts_with(&format!("{opt}=")))
        }),
        other => PLAIN_READERS.contains(&other),
    }
}

/// The name a shell would look up, lowercased, without `.exe`. A name with a
/// separator in it is a path, and a path is not something to vouch for.
fn program_name(text: &str) -> Option<String> {
    if text.contains(['/', '\\']) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    Some(lower.strip_suffix(".exe").unwrap_or(&lower).to_string())
}

/// A file in one of the directories that shares the program's name would be
/// run instead of the program. Only for names the classifier vouches for.
fn shadowed(simple: &Simple, dirs: &[&Path]) -> bool {
    let Some(name) = simple.words.first().and_then(|w| program_name(&w.text)) else {
        return false;
    };
    dirs.iter().any(|dir| {
        RUNNABLE_EXTENSIONS
            .iter()
            .any(|ext| dir.join(format!("{name}{ext}")).exists())
    })
}

fn git_is_read_only(args: &[Word]) -> bool {
    let Some(verb) = args.first() else {
        return false;
    };
    // A global option is `-c` or `-C` or `--exec-path`, and each one reaches
    // configuration or a repository this check has not looked at.
    if verb.text.starts_with('-') {
        return false;
    }
    let rest = &args[1..];
    match verb.text.as_str() {
        verb if GIT_READERS.contains(&verb) => no_output_option(rest),
        "grep" => {
            no_output_option(rest)
                && !rest.iter().any(|a| {
                    a.text.starts_with("-O") || a.text.starts_with("--open-files-in-pager")
                })
        }
        "branch" => branch_only_lists(rest),
        "remote" => match rest {
            [] => true,
            [flag] => flag.text == "-v" || flag.text == "--verbose",
            _ => false,
        },
        _ => false,
    }
}

/// `--output` writes the result to a file; on `log`, `show` and `diff` it is
/// the only way they write at all.
fn no_output_option(args: &[Word]) -> bool {
    !args.iter().any(|a| a.text.starts_with("--output"))
}

/// `git branch` creates, renames and deletes unless it is told to list. Only
/// the list forms are read-only, and a name of a branch is never one of them.
fn branch_only_lists(args: &[Word]) -> bool {
    const FLAGS: [&str; 15] = [
        "--list",
        "-l",
        "-a",
        "--all",
        "-r",
        "--remotes",
        "-v",
        "-vv",
        "--verbose",
        "--show-current",
        "--color",
        "--no-color",
        "--column",
        "--no-column",
        "--ignore-case",
    ];
    const PREFIXES: [&str; 4] = ["--color=", "--column=", "--format=", "--sort="];
    const COMMIT_FILTERS: [&str; 4] = ["--contains", "--no-contains", "--merged", "--no-merged"];

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.text.as_str();
        if FLAGS.contains(&text) || PREFIXES.iter().any(|p| text.starts_with(p)) {
            continue;
        }
        if COMMIT_FILTERS.contains(&text) {
            // The commit it filters by is optional. A flag in its place is not
            // that commit, and would be read as a flag of its own.
            if let Some(value) = iter.next() {
                if value.text.starts_with('-') {
                    return false;
                }
            }
            continue;
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// The classifier with no project to look in.
    fn ro(command: &str) -> bool {
        is_read_only_in(command, &[])
    }

    #[test]
    fn everyday_reads_are_read_only() {
        for command in [
            "git status",
            "git status --short --branch",
            "git log --oneline -5",
            "git diff HEAD~1",
            "git diff --stat -- src",
            "git show HEAD:README.md",
            "git branch --list",
            "git branch -a",
            "git branch --show-current",
            "git branch --contains HEAD",
            "git rev-parse HEAD",
            "git ls-files",
            "git remote -v",
            "git grep TODO",
            "git log --format=%H",
            "ls",
            "ls -la src",
            "dir",
            "pwd",
            "echo hello world",
            "cat README.md",
            "type Cargo.toml",
            "head -n 20 src/main.rs",
            "tail -n 5 log.txt",
            "wc -l src/main.rs",
            "rg TODO crates",
            "rg --pretty needle",
            "grep -rn fn src",
            "find . -name '*.rs'",
            "Get-ChildItem -Recurse src",
            "Get-Content Cargo.toml",
            "cat README.md | rg TODO",
            "git status && git log -1",
            "echo 'a > b'",
            "echo \"a; b\"",
            "echo ''",
            "echo \"\"",
        ] {
            assert!(ro(command), "should be read-only: {command}");
        }
    }

    #[test]
    fn writes_are_not_read_only() {
        for command in [
            "rm -rf target",
            "rm x",
            "git commit -m x",
            "git push",
            "npm ci",
            "cargo check",
            "cargo test",
            "cargo tree",
            "sed -i s/a/b/ x",
            "sort -o out in",
            "tee out",
            "ls | tee out",
            "touch x",
            "mkdir x",
            "mv a b",
            "cp a b",
            "curl https://example.com",
        ] {
            assert!(!ro(command), "should not be read-only: {command}");
        }
    }

    #[test]
    fn redirection_is_a_write() {
        for command in [
            "echo hi > README.md",
            "echo hi >> README.md",
            "cat a >b",
            "echo hi 2>&1",
            "ls >/dev/null",
            "git log > out.txt",
            "cat < input",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn chains_are_read_only_only_if_every_part_is() {
        for command in [
            "git status && rm -rf x",
            "git status; rm x",
            "git status || rm x",
            "git status & rm x",
            "cat x | sh",
            "cat x | xargs rm",
            "ls\nrm x",
            "ls\r\nrm x",
            "(rm x)",
            "{ rm x; }",
            "& rm x",
            "git status ;rm x",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn command_substitution_is_not_read_only() {
        for command in [
            "echo $(rm x)",
            "echo `rm x`",
            "echo \"$(rm x)\"",
            "echo \"`rm x`\"",
            "cat $(which git)",
            "ls $HOME",
            "echo $env:PATH",
            "$env:X = 1",
            "cat \"$HOME/x\"",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn globs_and_braces_are_not_read_only() {
        // A brace list turns into several words, and the extra word is a
        // command of its own that the classifier does not know.
        for command in [
            "git log {--output=x,}",
            "cat {a,b}",
            "ls *.rs",
            "cat src/?.rs",
            "git diff -- src/[ab].rs",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn env_prefixes_and_assignments_are_not_read_only() {
        for command in [
            "GIT_PAGER=sh git log",
            "X=1 cat x",
            "PAGER=cat git status",
            "FOO=bar ls",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn a_path_to_a_program_is_not_vouched_for() {
        for command in [
            "./git status",
            "bin/git status",
            "..\\git.exe status",
            "/usr/bin/git status",
            "C:\\Program Files\\Git\\cmd\\git.exe status",
            "C:\\bin\\git.exe status",
            "~/bin/ls",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn git_global_options_are_not_read_only() {
        // Each of these reaches configuration that runs a program, or another
        // repository whose configuration nobody checked.
        for command in [
            "git -c core.pager=sh log",
            "git -c core.fsmonitor=evil status",
            "git -C ../other status",
            "git --exec-path=/tmp/x status",
            "git --git-dir=/other/.git status",
            "git --no-pager status",
            "git -c alias.st=!sh status",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn git_output_options_are_not_read_only() {
        for command in [
            "git diff --output=out.patch",
            "git diff --output out.patch",
            "git log --output=log.txt",
            "git log --output log.txt",
            "git show --output=x HEAD",
            "git diff --output-indicator-new=+ HEAD",
            "git grep -O TODO",
            "git grep --open-files-in-pager=vim TODO",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn git_verbs_that_write_are_not_read_only() {
        for command in [
            "git add .",
            "git checkout main",
            "git reset --hard",
            "git stash",
            "git pull",
            "git fetch",
            "git tag v1",
            "git config user.name x",
            "git remote add origin url",
            "git remote remove origin",
            "git clean -fdx",
            "git merge other",
            "git rebase main",
            "git apply x.patch",
            "git",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn git_branch_that_creates_or_deletes_is_not_read_only() {
        for command in [
            "git branch new-feature",
            "git branch -d old",
            "git branch -D old",
            "git branch -m old new",
            "git branch --list -D old",
            "git branch -av",
            "git branch --contains -d x",
            "git branch -f main HEAD~1",
            "git branch --set-upstream-to origin/main",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn git_remote_with_arguments_is_not_read_only() {
        assert!(!ro("git remote show origin"));
        assert!(!ro("git remote set-url origin x"));
    }

    #[test]
    fn find_actions_that_write_or_run_are_not_read_only() {
        for command in [
            "find . -name x -exec rm {} \\;",
            "find . -name x -execdir rm {} +",
            "find . -name x -ok rm {} \\;",
            "find . -name x -delete",
            "find . -fprint out.txt",
            "find . -fls out.txt",
        ] {
            assert!(!ro(command), "{command}");
        }
        assert!(ro("find . -name '*.rs' -newer Cargo.toml"));
    }

    #[test]
    fn rg_options_that_run_a_program_are_not_read_only() {
        for command in [
            "rg --pre=sh TODO",
            "rg --pre sh TODO",
            "rg --pre-glob='*' --pre=sh TODO",
            "rg --hostname-bin=sh TODO",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn powershell_pipelines_with_code_blocks_are_not_read_only() {
        for command in [
            "Get-ChildItem | Where-Object { $_.Length -gt 1 }",
            "Get-ChildItem | ForEach-Object { Remove-Item $_ }",
            "Get-Content x | Invoke-Expression",
            "Get-Content x | iex",
            "gci | % { rm $_ }",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn quoting_does_not_hide_a_second_command() {
        for command in [
            "echo \"a\"; rm x",
            "echo 'a'; rm x",
            "echo \"a\" && rm x",
            "echo \"a; rm x",
            "echo 'a; rm x",
            "echo \"a\\\"; rm x; echo \\\"\"",
            "echo \\\"; rm x; echo \\\"",
            "echo `\"; rm x; echo `\"",
            "git status `; rm x",
        ] {
            assert!(!ro(command), "{command}");
        }
    }

    #[test]
    fn an_unclosed_quote_is_not_read_only() {
        assert!(!ro("echo \"abc"));
        assert!(!ro("cat 'x"));
        assert!(!ro("git log --format=\"%h"));
    }

    #[test]
    fn a_file_named_like_a_reader_in_the_project_shadows_it() {
        let dir = TempDir::new();
        dir.write("cat.bat", "@echo off\r\n");
        assert!(!is_read_only_in("cat README.md", &[dir.path()]));
        assert!(is_read_only_in("cat README.md", &[]));
    }

    #[test]
    fn a_shadowing_file_in_an_unrelated_directory_changes_nothing() {
        let project = TempDir::new();
        let elsewhere = TempDir::new();
        elsewhere.write("git", "#!/bin/sh\n");
        assert!(is_read_only_in("git status", &[project.path()]));
    }
}

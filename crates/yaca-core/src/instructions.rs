//! The project's standing instructions for the agent: `AGENTS.md`, or
//! `CLAUDE.md` when that is the only one (§10), at the project root and in the
//! directories below it.
//!
//! The core reads the files and puts them in the system prompt rather than
//! telling the model to go and look. An instruction the model has to remember
//! to fetch is one a weak model never sees. The prompt is also the one place
//! that survives compaction (§5.11): a first message carrying the file would
//! be summarized away along with the rest.
//!
//! `AGENTS.md` is looked for first because it is the name every coding agent
//! reads, not only Claude. Where it is a link to `CLAUDE.md` — the way §10
//! expects a project to keep the two — the text comes from the target, so
//! there is one file either way.
//!
//! A link is followed to what it points at. So is the text file git writes in
//! a link's place when `core.symlinks` is off, which is the default on
//! Windows: a single line holding the target's path. Taking that line for the
//! instructions would hand the model the word `CLAUDE.md` and nothing else.
//!
//! Only files inside the project are read. The text reaches the model without
//! anyone being asked, so a link out of the tree — to `~/.ssh`, say — would be
//! a way past the boundary every tool keeps.
//!
//! # Nested files
//!
//! By the `AGENTS.md` convention a file in a subdirectory refines the rules for
//! its own part of the tree. All of them are read when the session starts and
//! travel in the prompt next to the root file, each labelled with the directory
//! it governs; the prompt says the nearer file wins. They are not fetched
//! lazily when the model first touches a directory: that would put text into
//! the middle of the conversation, where compaction drops it, and would move
//! the cached prefix of every request that follows.
//!
//! What that costs is bounded, because the prompt is paid for on every turn:
//! at most [`NESTED_MAX_FILES`] files, [`NESTED_MAX_DEPTH`] levels down, a
//! [`NESTED_FILE_BYTES`] head of each and [`NESTED_TOTAL_BYTES`] altogether.
//! A file past a limit is still listed by path, so the model knows it exists
//! and can open it, rather than finding out from nothing.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::workspace::Workspace;

/// Looked for at the project root, and in every directory below it, in this
/// order.
pub const NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// How much of the root file goes into the prompt. It rides along with every
/// request, so a runaway file is paid for on every turn, or pushes the
/// conversation out of the context altogether. What is cut is said to be cut.
const MAX_BYTES: usize = 64 * 1024;

/// The same, for one nested file. Smaller: there may be many, and each only
/// matters for part of the tree.
const NESTED_FILE_BYTES: usize = 16 * 1024;

/// What all nested files together may add to the prompt.
const NESTED_TOTAL_BYTES: usize = 64 * 1024;

/// How many directories are consulted. A tree with more has a convention
/// problem of its own, and the prompt is no place to solve it.
const NESTED_MAX_FILES: usize = 32;

/// Levels below the root that are walked. Deeper files are not looked for:
/// nobody keeps rules nine directories down, and the walk stays cheap on a
/// large tree.
const NESTED_MAX_DEPTH: usize = 8;

/// Directories never entered even where nothing ignores them. A project
/// without a `.gitignore` is ordinary, and walking `node_modules` to find
/// rules that are not there is a stall at the start of every session.
const SKIPPED_DIRS: [&str; 5] = ["node_modules", "target", "dist", "build", "vendor"];

/// How many link placeholders are followed in a row. Two files naming each
/// other would otherwise be followed forever.
const MAX_HOPS: usize = 4;

/// One instruction file, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFile {
    /// The file the text came from, relative to the project root, with links
    /// followed: the file to read again or edit is this one.
    pub path: String,
    /// Empty when the file was past the budget and only its path is kept.
    pub text: String,
    /// `text` is only the head of the file, or the file was left out
    /// altogether.
    pub truncated: bool,
}

/// An instruction file below the root, and the part of the tree it governs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nested {
    /// The directory, relative to the project root, as the model writes paths.
    pub dir: String,
    pub file: RuleFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instructions {
    /// `None` when only subdirectories keep instructions.
    pub root: Option<RuleFile>,
    /// In path order, so that a parent comes before its children and the prompt
    /// is the same on every start.
    pub nested: Vec<Nested>,
}

/// The project's instructions, or `None` when it keeps none.
///
/// A file that cannot be read counts as absent, and the next name is tried.
/// There is nobody to tell at this point: the session is only starting, and
/// refusing to start it over an unreadable `AGENTS.md` would be worse than
/// going without.
pub fn load(ws: &Workspace) -> Option<Instructions> {
    let root = NAMES.iter().find_map(|name| read(ws, name, MAX_BYTES));
    let nested = nested(ws);
    if root.is_none() && nested.is_empty() {
        return None;
    }
    Some(Instructions { root, nested })
}

/// Instruction files below the root, nearest the root first.
fn nested(ws: &Workspace) -> Vec<Nested> {
    let mut found = Vec::new();
    let mut budget = NESTED_TOTAL_BYTES;

    for dir in directories(ws).into_iter().take(NESTED_MAX_FILES) {
        // The cap is what is left, so one large file cannot starve the rest of
        // the tree of any rules at all.
        let cap = NESTED_FILE_BYTES.min(budget);
        let Some(file) = NAMES
            .iter()
            .find_map(|name| read(ws, &format!("{dir}/{name}"), cap))
        else {
            continue;
        };
        budget = budget.saturating_sub(file.text.len());
        found.push(Nested { dir, file });
    }
    found
}

/// Directories below the root holding an instruction file, as project-relative
/// paths in a fixed order.
///
/// Only the primary root is walked. Instructions in an additional root are not
/// read: they are another project's, and reading them would put that project's
/// rules into this one's prompt (README, multi-root).
fn directories(ws: &Workspace) -> BTreeSet<String> {
    let walk = ignore::WalkBuilder::new(ws.root())
        .hidden(true)
        // Same as the read tools: a project without `.git` still has a
        // `.gitignore` worth honouring.
        .require_git(false)
        .max_depth(Some(NESTED_MAX_DEPTH + 1))
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            !(is_dir && SKIPPED_DIRS.iter().any(|d| entry.file_name() == *d))
        })
        .build();

    walk.flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter(|e| NAMES.iter().any(|n| e.file_name() == *n))
        .filter_map(|e| {
            let parent = e.path().parent()?;
            (parent != ws.root()).then(|| ws.display(parent))
        })
        .collect()
}

/// Reads `name` (a path relative to the root) up to `cap` bytes. A zero cap
/// still finds the file and reports it as left out.
fn read(ws: &Workspace, name: &str, cap: usize) -> Option<RuleFile> {
    // Bounded like any tool's path, which covers a real link out of the tree.
    let mut path = ws.resolve_existing(name).ok()?;
    for _ in 0..MAX_HOPS {
        let (text, truncated) = head(&path, cap)?;
        match placeholder_target(&path, &text) {
            // The placeholder is held to the same boundary as a real link.
            Some(target) => path = ws.resolve_existing(target.to_str()?).ok()?,
            None => {
                return Some(RuleFile {
                    path: ws.display(&path),
                    text,
                    truncated,
                })
            }
        }
    }
    None
}

/// The first `cap` bytes of a file as text, and whether there was more.
fn head(path: &Path, cap: usize) -> Option<(String, bool)> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let truncated = bytes.len() > cap;
    bytes.truncate(cap);

    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        // The cap fell inside a character: keep the whole ones before it.
        Err(err) if truncated && err.utf8_error().error_len().is_none() => {
            let valid = err.utf8_error().valid_up_to();
            let mut bytes = err.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).expect("valid_up_to() ends a valid UTF-8 prefix")
        }
        Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
    };
    Some((text, truncated))
}

/// Where a link placeholder points, if this text is one.
///
/// Git writes the target's path and nothing else. Any file that is a single
/// line naming an existing file is taken as a link: either it is the
/// placeholder, or someone wrote the same thing by hand, and in neither case is
/// the name itself worth giving the model.
fn placeholder_target(path: &Path, text: &str) -> Option<PathBuf> {
    let line = text.trim();
    if line.is_empty() || line.contains('\n') {
        return None;
    }
    let target = path.parent()?.join(line);
    target.is_file().then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn load_from(dir: &TempDir) -> Option<Instructions> {
        load(&Workspace::open(dir.path()).unwrap())
    }

    /// The root file, which most of the tests below are about.
    fn root_of(dir: &TempDir) -> Option<RuleFile> {
        load_from(dir).and_then(|i| i.root)
    }

    fn nested_of(dir: &TempDir) -> Vec<Nested> {
        load_from(dir).map(|i| i.nested).unwrap_or_default()
    }

    #[test]
    fn agents_md_is_preferred_over_claude_md() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "from agents");
        dir.write("CLAUDE.md", "from claude");

        let found = root_of(&dir).unwrap();
        assert_eq!(found.path, "AGENTS.md");
        assert_eq!(found.text, "from agents");
    }

    #[test]
    fn claude_md_is_read_when_it_is_the_only_one() {
        let dir = TempDir::new();
        dir.write("CLAUDE.md", "from claude");

        let found = root_of(&dir).unwrap();
        assert_eq!(found.path, "CLAUDE.md");
        assert_eq!(found.text, "from claude");
    }

    #[test]
    fn a_project_without_either_has_none() {
        assert_eq!(load_from(&TempDir::new()), None);
    }

    #[test]
    fn a_git_link_placeholder_is_followed_to_its_target() {
        // What a checkout with `core.symlinks=false` leaves on disk: the
        // target's path, with no newline after it.
        let dir = TempDir::new();
        dir.write("AGENTS.md", "CLAUDE.md");
        dir.write("CLAUDE.md", "the real rules\nsecond line");

        let found = root_of(&dir).unwrap();
        assert_eq!(found.path, "CLAUDE.md");
        assert_eq!(found.text, "the real rules\nsecond line");
    }

    #[test]
    fn a_real_link_is_followed_to_its_target() {
        let dir = TempDir::new();
        dir.write("CLAUDE.md", "the real rules");
        if !symlink(&dir.path().join("CLAUDE.md"), &dir.path().join("AGENTS.md")) {
            // Windows without Developer Mode refuses to create links, and
            // then there is no link to follow; the placeholder test above
            // covers what such a machine gets instead.
            return;
        }

        let found = root_of(&dir).unwrap();
        assert_eq!(found.path, "CLAUDE.md");
        assert_eq!(found.text, "the real rules");
    }

    #[test]
    fn a_placeholder_out_of_the_project_is_not_followed() {
        let outside = TempDir::new();
        outside.write("secret.md", "not for the model");
        let dir = TempDir::new();
        let target = outside.path().join("secret.md");
        dir.write("AGENTS.md", target.to_str().unwrap());
        dir.write("CLAUDE.md", "the project's own rules");

        let found = root_of(&dir).unwrap();
        assert_eq!(found.text, "the project's own rules");
    }

    #[test]
    fn a_one_line_instruction_is_not_mistaken_for_a_link() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "Answer in English.\n");

        let found = root_of(&dir).unwrap();
        assert_eq!(found.text, "Answer in English.\n");
    }

    #[test]
    fn placeholders_naming_each_other_give_nothing() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "CLAUDE.md");
        dir.write("CLAUDE.md", "AGENTS.md");

        assert_eq!(load_from(&dir), None);
    }

    #[test]
    fn a_huge_file_is_cut_on_a_character_boundary() {
        let dir = TempDir::new();
        // Two-byte characters with an odd cap would split one in half.
        dir.write("AGENTS.md", &format!("x{}", "я".repeat(MAX_BYTES)));

        let found = root_of(&dir).unwrap();
        assert!(found.truncated);
        assert!(found.text.len() <= MAX_BYTES);
        assert!(found.text.len() >= MAX_BYTES - 1);
        assert!(!found.text.contains('\u{FFFD}'));
    }

    #[test]
    fn a_file_under_the_cap_is_whole() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "short");

        assert!(!root_of(&dir).unwrap().truncated);
    }

    #[test]
    fn nested_files_are_read_with_the_directory_they_govern() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "root rules");
        dir.write("crates/core/AGENTS.md", "core rules");
        dir.write("web/CLAUDE.md", "web rules");

        let found = load_from(&dir).unwrap();
        assert_eq!(found.root.unwrap().text, "root rules");

        let dirs: Vec<_> = found.nested.iter().map(|n| n.dir.as_str()).collect();
        assert_eq!(dirs, ["crates/core", "web"]);
        assert_eq!(found.nested[0].file.path, "crates/core/AGENTS.md");
        assert_eq!(found.nested[0].file.text, "core rules");
        assert_eq!(found.nested[1].file.path, "web/CLAUDE.md");
    }

    #[test]
    fn a_project_with_only_nested_files_still_has_instructions() {
        let dir = TempDir::new();
        dir.write("packages/a/AGENTS.md", "a rules");

        let found = load_from(&dir).unwrap();
        assert!(found.root.is_none());
        assert_eq!(found.nested.len(), 1);
    }

    #[test]
    fn a_parent_comes_before_its_children_and_the_order_is_fixed() {
        let dir = TempDir::new();
        dir.write("b/AGENTS.md", "b");
        dir.write("a/inner/AGENTS.md", "a inner");
        dir.write("a/AGENTS.md", "a");

        let dirs: Vec<_> = nested_of(&dir).into_iter().map(|n| n.dir).collect();
        assert_eq!(dirs, ["a", "a/inner", "b"]);
        assert_eq!(dirs, nested_of(&dir).into_iter().map(|n| n.dir).collect::<Vec<_>>());
    }

    #[test]
    fn agents_md_wins_over_claude_md_in_the_same_directory() {
        let dir = TempDir::new();
        dir.write("pkg/AGENTS.md", "agents");
        dir.write("pkg/CLAUDE.md", "claude");

        let found = nested_of(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file.text, "agents");
    }

    #[test]
    fn a_nested_placeholder_is_followed_inside_its_directory() {
        let dir = TempDir::new();
        dir.write("pkg/AGENTS.md", "CLAUDE.md");
        dir.write("pkg/CLAUDE.md", "pkg rules");

        let found = nested_of(&dir);
        assert_eq!(found[0].file.path, "pkg/CLAUDE.md");
        assert_eq!(found[0].file.text, "pkg rules");
    }

    #[test]
    fn ignored_and_heavy_directories_are_not_entered() {
        let dir = TempDir::new();
        dir.write(".gitignore", "generated/\n");
        dir.write("generated/AGENTS.md", "not ours");
        dir.write("node_modules/dep/AGENTS.md", "not ours");
        dir.write("target/AGENTS.md", "not ours");
        dir.write(".hidden/AGENTS.md", "not ours");
        dir.write("src/AGENTS.md", "ours");

        let dirs: Vec<_> = nested_of(&dir).into_iter().map(|n| n.dir).collect();
        assert_eq!(dirs, ["src"]);
    }

    #[test]
    fn files_below_the_depth_limit_are_not_looked_for() {
        let dir = TempDir::new();
        let deep = ["d"; NESTED_MAX_DEPTH + 1].join("/");
        dir.write(&format!("{deep}/AGENTS.md"), "too deep");
        let ok = ["d"; NESTED_MAX_DEPTH].join("/");
        dir.write(&format!("{ok}/AGENTS.md"), "just deep enough");

        let found = nested_of(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dir, ok);
    }

    #[test]
    fn a_nested_file_is_cut_to_its_own_cap_and_says_so() {
        let dir = TempDir::new();
        dir.write("pkg/AGENTS.md", &"x".repeat(NESTED_FILE_BYTES + 10));

        let found = nested_of(&dir);
        assert!(found[0].file.truncated);
        assert_eq!(found[0].file.text.len(), NESTED_FILE_BYTES);
    }

    #[test]
    fn the_shared_budget_runs_out_and_later_files_are_listed_without_text() {
        let dir = TempDir::new();
        // Five full-size files outrun the total of four; zero-padded names
        // keep the order the test relies on.
        for n in 0..5 {
            dir.write(
                &format!("p{n}/AGENTS.md"),
                &"x".repeat(NESTED_FILE_BYTES),
            );
        }

        let found = nested_of(&dir);
        assert_eq!(found.len(), 5, "an unread file is still listed");
        let used: usize = found.iter().map(|n| n.file.text.len()).sum();
        assert!(used <= NESTED_TOTAL_BYTES);
        let last = &found[4].file;
        assert!(last.text.is_empty() && last.truncated, "{last:?}");
        assert_eq!(last.path, "p4/AGENTS.md");
    }

    #[test]
    fn the_number_of_files_is_bounded() {
        let dir = TempDir::new();
        for n in 0..NESTED_MAX_FILES + 5 {
            dir.write(&format!("d{n:03}/AGENTS.md"), "r");
        }
        assert_eq!(nested_of(&dir).len(), NESTED_MAX_FILES);
    }

    /// Whether the OS agreed to create the link.
    fn symlink(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(target, link);
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(target, link);
        made.is_ok()
    }
}

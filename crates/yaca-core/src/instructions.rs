//! The project's standing instructions for the agent: `AGENTS.md`, or
//! `CLAUDE.md` when that is the only one (§10).
//!
//! The core reads the file and puts it in the system prompt rather than
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

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::workspace::Workspace;

/// Looked for at the project root, in this order.
pub const NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// How much of the file goes into the prompt. It rides along with every
/// request, so a runaway file is paid for on every turn, or pushes the
/// conversation out of the context altogether. What is cut is said to be cut.
const MAX_BYTES: usize = 64 * 1024;

/// How many link placeholders are followed in a row. Two files naming each
/// other would otherwise be followed forever.
const MAX_HOPS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instructions {
    /// The file the text came from, relative to the project root, with links
    /// followed: the file to read again or edit is this one.
    pub path: String,
    pub text: String,
    /// `text` is only the head of the file, [`MAX_BYTES`] of it.
    pub truncated: bool,
}

/// The project's instructions, or `None` when it keeps none.
///
/// A file that cannot be read counts as absent, and the next name is tried.
/// There is nobody to tell at this point: the session is only starting, and
/// refusing to start it over an unreadable `AGENTS.md` would be worse than
/// going without.
pub fn load(ws: &Workspace) -> Option<Instructions> {
    NAMES.iter().find_map(|name| read(ws, name))
}

fn read(ws: &Workspace, name: &str) -> Option<Instructions> {
    // Bounded like any tool's path, which covers a real link out of the tree.
    let mut path = ws.resolve_existing(name).ok()?;
    for _ in 0..MAX_HOPS {
        let (text, truncated) = head(&path)?;
        match placeholder_target(&path, &text) {
            // The placeholder is held to the same boundary as a real link.
            Some(target) => path = ws.resolve_existing(target.to_str()?).ok()?,
            None => {
                return Some(Instructions {
                    path: ws.display(&path),
                    text,
                    truncated,
                })
            }
        }
    }
    None
}

/// The first [`MAX_BYTES`] of a file as text, and whether there was more.
fn head(path: &Path) -> Option<(String, bool)> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let truncated = bytes.len() > MAX_BYTES;
    bytes.truncate(MAX_BYTES);

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

    #[test]
    fn agents_md_is_preferred_over_claude_md() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "from agents");
        dir.write("CLAUDE.md", "from claude");

        let found = load_from(&dir).unwrap();
        assert_eq!(found.path, "AGENTS.md");
        assert_eq!(found.text, "from agents");
    }

    #[test]
    fn claude_md_is_read_when_it_is_the_only_one() {
        let dir = TempDir::new();
        dir.write("CLAUDE.md", "from claude");

        let found = load_from(&dir).unwrap();
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

        let found = load_from(&dir).unwrap();
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

        let found = load_from(&dir).unwrap();
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

        let found = load_from(&dir).unwrap();
        assert_eq!(found.text, "the project's own rules");
    }

    #[test]
    fn a_one_line_instruction_is_not_mistaken_for_a_link() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "Answer in English.\n");

        let found = load_from(&dir).unwrap();
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

        let found = load_from(&dir).unwrap();
        assert!(found.truncated);
        assert!(found.text.len() <= MAX_BYTES);
        assert!(found.text.len() >= MAX_BYTES - 1);
        assert!(!found.text.contains('\u{FFFD}'));
    }

    #[test]
    fn a_file_under_the_cap_is_whole() {
        let dir = TempDir::new();
        dir.write("AGENTS.md", "short");

        assert!(!load_from(&dir).unwrap().truncated);
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

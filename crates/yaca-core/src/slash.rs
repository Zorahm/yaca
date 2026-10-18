//! The project's own slash commands (§7.3): `.yaca/commands/*.md`.
//!
//! A file is a prompt kept under a name. `.yaca/commands/review.md` becomes
//! `/review`, and what the human types is a name while what the model gets is
//! the text behind it — the same split §7.2 already makes for attachments. The
//! feed and the journal keep the typed line, so a reopened session shows
//! `/review` and not the wall of text it stood for.
//!
//! The directory is read per message rather than once at spawn. A command
//! written while the session is open should work in that session: the read
//! costs one `read_dir` against a model call, and a cache here would mean
//! editing a file and being told, without a word, that nothing changed.
//!
//! The built-in commands of §7.3 — `/compact`, `/model`, `/clear` — are not
//! here. They are not prompts: they are things the interface does, and the core
//! already has a [`Command`](crate::Command) for each one it can do.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where the files live, under the project's `.yaca/`.
pub const DIR: &str = "commands";

/// Where what followed the name goes, if the body asks for it.
pub const ARGUMENTS: &str = "$ARGUMENTS";

/// A prompt the project keeps under a name (§7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommand {
    /// What is typed after the slash — the file stem.
    pub name: String,
    /// The dimmed second column of the menu. Empty when the file gives none.
    pub description: String,
    /// The prompt itself, with the frontmatter taken off.
    pub body: String,
}

/// Every command of a project, by name.
///
/// A file that cannot be read is left out rather than reported: the list feeds
/// a menu, and a menu is not the place to learn about a permission problem in a
/// directory the human may not even know exists. A file that *can* be read
/// always yields a command — there is no such thing as a prompt that does not
/// parse.
pub fn load(root: &Path) -> Vec<SlashCommand> {
    let dir = crate::project::dir(root).join(DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out: Vec<SlashCommand> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "md" {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            if name.is_empty() || !name.chars().all(is_name_char) {
                return None;
            }
            Some(parse(name, &std::fs::read_to_string(&path).ok()?))
        })
        .collect();

    // Alphabetical: the menu shows them in this order, and directory order is
    // whatever the filesystem felt like.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The prompt behind a typed line, or `None` when the line is an ordinary
/// message — including a `/name` no file answers to.
///
/// An unknown name goes to the model as written. The alternative is refusing to
/// send it, and a human who typed `/usr/bin/env is missing` deserves an answer
/// rather than a lecture about slash commands.
pub fn expand(root: &Path, text: &str) -> Option<String> {
    let (name, args) = split(text)?;
    let command = load(root).into_iter().find(|c| c.name == name)?;
    Some(fill(&command.body, args))
}

/// [`expand`] over a list already loaded: the plugins' commands.
pub fn expand_in(commands: &[SlashCommand], text: &str) -> Option<String> {
    let (name, args) = split(text)?;
    let command = commands.iter().find(|c| c.name == name)?;
    Some(fill(&command.body, args))
}

/// Splits a typed line into a command name and what followed it.
///
/// `None` when the line is not a command: no leading slash, an empty name, or a
/// name with something other than letters, digits, `-` and `_` in it. That last
/// rule is what keeps a pasted path — `/usr/share/doc` — from being read as a
/// command with a very strange name.
pub fn split(text: &str) -> Option<(&str, &str)> {
    let line = text.strip_prefix('/')?;
    let end = line
        .find(|c: char| c.is_whitespace())
        .unwrap_or(line.len());
    let (name, rest) = line.split_at(end);
    if name.is_empty() || !name.chars().all(is_name_char) {
        return None;
    }
    Some((name, rest.trim()))
}

/// `:` joins a plugin's name to its command's (`/tools:review`), and `.` is
/// allowed in plugin names.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

/// A name a typed `/name` can reach.
pub(crate) fn is_command_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_name_char)
}

/// Puts the arguments where the body asks for them.
///
/// A body that never mentions [`ARGUMENTS`] still gets them, appended: the
/// alternative is dropping what the human typed on the floor, which is the one
/// outcome nobody wants.
pub(crate) fn fill(body: &str, args: &str) -> String {
    if body.contains(ARGUMENTS) {
        return body.replace(ARGUMENTS, args);
    }
    if args.is_empty() {
        return body.to_string();
    }
    format!("{}\n\n{args}", body.trim_end())
}

/// Reads the optional frontmatter off the top of a file.
///
/// Only `description` is read, and only as a plain `key: value` line. This is
/// not YAML and does not pretend to be: a command file is a prompt with a label
/// on it, and a parser that accepted anchors and block scalars would be
/// promising a format nothing here can keep.
pub(crate) fn parse(name: String, text: &str) -> SlashCommand {
    let (description, body) = match split_frontmatter(text) {
        Some((header, body)) => (header_value(header, "description").unwrap_or_default(), body),
        None => (String::new(), text),
    };

    SlashCommand {
        name,
        description,
        body: body.trim().to_string(),
    }
}

/// Splits a file into its frontmatter block and the body after it, or `None`
/// when there is no frontmatter — no leading `---`, or one never closed.
///
/// Shared with skills (`skills::frontmatter`), whose files carry the same header.
pub(crate) fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---")?;
    let rest = rest.strip_prefix('\n').or_else(|| rest.strip_prefix("\r\n"))?;
    let end = find_fence(rest)?;
    Some((&rest[..end.0], &rest[end.1..]))
}

/// The value of one `key: value` line in a frontmatter block. The last such
/// line wins, as it always has.
pub(crate) fn header_value(header: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    let mut found = None;
    for line in header.lines() {
        if let Some(value) = line.trim().strip_prefix(&prefix) {
            found = Some(value.trim().trim_matches('"').to_string());
        }
    }
    found
}

/// Finds the closing `---`, answering where the frontmatter ends and where the
/// body begins.
fn find_fence(text: &str) -> Option<(usize, usize)> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((at, at + line.len()));
        }
        at += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn a_line_that_is_not_a_command_is_not_split() {
        assert_eq!(split("fix date parsing"), None);
        assert_eq!(split("/"), None);
        assert_eq!(split("/ "), None);
        // A pasted path is a message, not a command with a strange name.
        assert_eq!(split("/usr/share/doc somewhere there"), None);
    }

    #[test]
    fn a_command_splits_off_its_arguments() {
        assert_eq!(split("/review"), Some(("review", "")));
        assert_eq!(split("/review  src/main.rs "), Some(("review", "src/main.rs")));
        assert_eq!(split("/fix-tests\nand more"), Some(("fix-tests", "and more")));
    }

    #[test]
    fn a_file_becomes_a_command_named_after_it() {
        let dir = TempDir::new();
        dir.write(".yaca/commands/review.md", "Review changes.\n");

        let loaded = load(dir.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "review");
        assert_eq!(loaded[0].description, "");
        assert_eq!(loaded[0].body, "Review changes.");
    }

    #[test]
    fn the_frontmatter_gives_the_menu_its_second_column() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/commands/review.md",
            "---\ndescription: Review changes\n---\nReview changes.\n",
        );

        let loaded = load(dir.path());
        assert_eq!(loaded[0].description, "Review changes");
        assert_eq!(loaded[0].body, "Review changes.");
    }

    #[test]
    fn an_unclosed_frontmatter_stays_part_of_the_prompt() {
        let dir = TempDir::new();
        dir.write(".yaca/commands/odd.md", "---\ndescription: endless\n");

        // Cutting at a fence that never comes would swallow the whole file and
        // leave a command that sends nothing.
        let loaded = load(dir.path());
        assert!(loaded[0].body.starts_with("---"), "{:?}", loaded[0].body);
        assert_eq!(loaded[0].description, "");
    }

    #[test]
    fn commands_come_out_in_the_order_the_menu_shows_them() {
        let dir = TempDir::new();
        dir.write(".yaca/commands/review.md", "one");
        dir.write(".yaca/commands/audit.md", "two");
        dir.write(".yaca/commands/notes.txt", "not a command");

        let names: Vec<_> = load(dir.path()).into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["audit", "review"]);
    }

    #[test]
    fn a_project_without_the_directory_has_no_commands() {
        let dir = TempDir::new();
        assert!(load(dir.path()).is_empty());
    }

    #[test]
    fn arguments_go_where_the_body_asks_for_them() {
        let dir = TempDir::new();
        dir.write(
            ".yaca/commands/review.md",
            "Review $ARGUMENTS and tell what is wrong.",
        );

        assert_eq!(
            expand(dir.path(), "/review src/main.rs").as_deref(),
            Some("Review src/main.rs and tell what is wrong.")
        );
    }

    #[test]
    fn arguments_a_body_never_mentions_are_appended_rather_than_dropped() {
        let dir = TempDir::new();
        dir.write(".yaca/commands/review.md", "Review changes.");

        assert_eq!(
            expand(dir.path(), "/review tests only").as_deref(),
            Some("Review changes.\n\ntests only")
        );
        assert_eq!(
            expand(dir.path(), "/review").as_deref(),
            Some("Review changes.")
        );
    }

    #[test]
    fn a_name_no_file_answers_to_is_left_alone() {
        let dir = TempDir::new();
        dir.write(".yaca/commands/review.md", "Review changes.");

        // It goes to the model as written; refusing to send it would be worse.
        assert_eq!(expand(dir.path(), "/unknown something"), None);
        assert_eq!(expand(dir.path(), "regular message"), None);
    }
}

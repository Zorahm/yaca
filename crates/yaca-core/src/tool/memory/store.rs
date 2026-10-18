//! The notes themselves: where they live under `.yaca/memory/`, which names
//! they may take, the index that lists them, and the index the prompt carries.
//!
//! The index is derived, never edited. A model that rewrote `MEMORY.md` by
//! hand would corrupt it, so every write and delete here regenerates it from
//! the notes' own front matter, sorted by file name. The same inputs give the
//! same bytes, and the index is never a second source of truth to drift from
//! the notes.
//!
//! The prompt reads the index once, when a session starts (see
//! `session::prompt`). Notes written during the session are readable through
//! the tool at once, but the cached prefix must not move mid-session, so they
//! reach the prompt only at the next start.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::error::ToolError;
use crate::project;
use crate::workspace::Workspace;

/// Notes live in a subdirectory of the project's `.yaca`, next to the settings
/// and the session log: one directory holds everything Yaca keeps for a project.
pub const NOTES_DIR: &str = "memory";
pub const INDEX: &str = "MEMORY.md";

/// The index goes into every request, so it is capped the way the instruction
/// files are. The cap is generous for a list of one-line hooks; it exists so a
/// long list cannot take over the prompt.
pub const MAX_INDEX_BYTES: usize = 16 * 1024;
const MAX_NOTE_BYTES: usize = 32 * 1024;
const MAX_HOOK_CHARS: usize = 160;

/// Names a note may not take. `memory` would collide with the index on a
/// case-insensitive filesystem, which Windows is; the device names are special
/// there whatever the extension says.
const RESERVED: &[&str] = &[
    "memory", "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
    "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// The index as the prompt will carry it. `truncated` says the file was longer
/// than the cap and only its beginning is here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIndex {
    pub text: String,
    pub truncated: bool,
}

/// A note's name is checked before any path is built from it, so the name
/// itself carries the boundary: lowercase letters, digits and hyphens, then
/// `.md`. Nothing that can express `..`, a separator or an absolute path
/// survives that.
pub fn is_note_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    !stem.is_empty()
        && stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !RESERVED.contains(&stem)
}

fn check_name(name: &str) -> Result<(), ToolError> {
    if is_note_name(name) {
        Ok(())
    } else {
        Err(ToolError::rejected(format!(
            "{name} is an invalid note name: lowercase ASCII letters, digits, hyphen, with .md suffix; \
             reserved names include memory, con, nul, etc."
        )))
    }
}

/// Path of a note relative to the project root, in the form the workspace
/// checks take.
fn rel(name: &str) -> String {
    format!("{}/{NOTES_DIR}/{name}", project::DIR)
}

fn dir_of(ws: &Workspace) -> PathBuf {
    ws.home().join(project::DIR).join(NOTES_DIR)
}

/// Collapses a value to one line. A hook or title that contained a newline
/// could close the front matter early and smuggle keys into the note.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Creates or replaces a note. Returns whether it existed before.
pub fn write(
    ws: &Workspace,
    name: &str,
    title: Option<&str>,
    hook: &str,
    body: &str,
) -> Result<bool, ToolError> {
    check_name(name)?;

    let hook = one_line(hook);
    if hook.is_empty() {
        return Err(ToolError::failed(
            "hook is required: one sentence describing the note",
        ));
    }
    if hook.chars().count() > MAX_HOOK_CHARS {
        return Err(ToolError::failed(format!(
            "hook exceeds {MAX_HOOK_CHARS} characters — please shorten it"
        )));
    }
    if body.len() > MAX_NOTE_BYTES {
        return Err(ToolError::failed(format!(
            "note exceeds {MAX_NOTE_BYTES} bytes — please split it into multiple notes"
        )));
    }

    let stem = name.trim_end_matches(".md");
    let title = title
        .map(one_line)
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| stem.to_string());

    let path = ws.resolve_new(&rel(name))?;
    let existed = path.is_file();
    let dir = dir_of(ws);
    fs::create_dir_all(&dir)?;
    let text = format!("---\nname: {title}\ndescription: {hook}\n---\n\n{}\n", body.trim_end());
    fs::write(&path, text)?;
    rebuild_index(&dir)?;
    Ok(existed)
}

/// The note's full text, front matter included.
pub fn read(ws: &Workspace, name: &str) -> Result<String, ToolError> {
    check_name(name)?;
    let path = ws.resolve_new(&rel(name))?;
    if !path.is_file() {
        return Err(ToolError::failed(format!("note {name} does not exist")));
    }
    Ok(fs::read_to_string(&path)?)
}

pub fn delete(ws: &Workspace, name: &str) -> Result<(), ToolError> {
    check_name(name)?;
    let path = ws.resolve_new(&rel(name))?;
    if !path.is_file() {
        return Err(ToolError::failed(format!("note {name} does not exist")));
    }
    fs::remove_file(&path)?;
    rebuild_index(&dir_of(ws))?;
    Ok(())
}

struct Note {
    file: String,
    title: String,
    hook: Option<String>,
}

/// Every note in the directory, sorted by file name. A file that is not a
/// note by name is ignored, so the index itself never lists itself.
fn notes(dir: &Path) -> io::Result<Vec<Note>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut notes = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file = entry.file_name().to_string_lossy().into_owned();
        if !is_note_name(&file) || !entry.file_type()?.is_file() {
            continue;
        }
        // Lossy, so a note with a stray byte still gets listed rather than
        // vanishing from the index without a trace.
        let text = String::from_utf8_lossy(&fs::read(entry.path())?).into_owned();
        let (title, hook) = front_matter(&text);
        let stem = file.trim_end_matches(".md").to_string();
        notes.push(Note {
            title: title.unwrap_or(stem),
            hook,
            file,
        });
    }
    notes.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(notes)
}

/// `name` and `description` from a leading `---` block. Anything else, or a
/// block never closed, counts as no front matter: the note is then listed by
/// its file name alone.
fn front_matter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, None);
    }
    let (mut title, mut hook) = (None, None);
    for line in lines {
        let line = line.trim_end();
        if line == "---" {
            return (non_empty(title), non_empty(hook));
        }
        if let Some(value) = line.strip_prefix("name:") {
            title = Some(one_line(value));
        } else if let Some(value) = line.strip_prefix("description:") {
            hook = Some(one_line(value));
        }
    }
    (None, None)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

fn index_line(note: &Note) -> String {
    // Brackets and parentheses in a title would break the link syntax.
    let title = note.title.replace(|c| "[]()".contains(c), "");
    match &note.hook {
        Some(hook) => format!("- [{title}]({}) — {hook}", note.file),
        None => format!("- [{title}]({})", note.file),
    }
}

/// Regenerates `MEMORY.md` from the notes. With no notes left the file goes
/// too: an index saying nothing would still be a section in every prompt.
fn rebuild_index(dir: &Path) -> io::Result<()> {
    let notes = notes(dir)?;
    let index = dir.join(INDEX);
    if notes.is_empty() {
        return match fs::remove_file(&index) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        };
    }
    let mut text = String::new();
    for note in &notes {
        text.push_str(&index_line(note));
        text.push('\n');
    }
    fs::write(index, text)
}

/// The index for the prompt, or `None` when the project has no notes. A
/// project that never used the feature has no directory at all, and must get
/// exactly the prompt it got before.
pub fn load(ws: &Workspace) -> Option<MemoryIndex> {
    let path = ws
        .resolve_existing(&format!("{}/{NOTES_DIR}/{INDEX}", project::DIR))
        .ok()?;
    let (text, truncated) = head(&path, MAX_INDEX_BYTES)?;
    if text.trim().is_empty() {
        return None;
    }
    Some(MemoryIndex { text, truncated })
}

/// The first `cap` bytes as text, and whether there was more. A cut that lands
/// inside a character keeps the whole characters before it, as the instruction
/// files do.
pub(crate) fn head(path: &Path, cap: usize) -> Option<(String, bool)> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let truncated = bytes.len() > cap;
    bytes.truncate(cap);
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    fn project() -> (TempDir, Workspace) {
        let dir = TempDir::new();
        let ws = Workspace::open(dir.path()).unwrap();
        (dir, ws)
    }

    fn index_of(dir: &TempDir) -> String {
        std::fs::read_to_string(dir.path().join(".yaca/memory/MEMORY.md")).unwrap()
    }

    #[test]
    fn write_read_delete_round_trip() {
        let (dir, ws) = project();

        assert!(!write(&ws, "build-quirks.md", Some("Build quirks"), "why the build is slow", "Use -j1 on CI.").unwrap());
        let text = read(&ws, "build-quirks.md").unwrap();
        assert!(text.starts_with("---\nname: Build quirks\ndescription: why the build is slow\n---\n"));
        assert!(text.contains("Use -j1 on CI."));

        // Replacing reports that it existed before.
        assert!(write(&ws, "build-quirks.md", None, "why the build is slow", "Use -j2.").unwrap());
        assert!(read(&ws, "build-quirks.md").unwrap().contains("Use -j2."));

        delete(&ws, "build-quirks.md").unwrap();
        assert!(!dir.path().join(".yaca/memory/build-quirks.md").exists());
        assert!(read(&ws, "build-quirks.md").is_err());
        assert!(delete(&ws, "build-quirks.md").is_err());
    }

    #[test]
    fn the_index_is_regenerated_sorted_with_title_and_hook() {
        let (dir, ws) = project();
        write(&ws, "zeta.md", Some("Zeta"), "last one", "z").unwrap();
        write(&ws, "alpha.md", None, "first one", "a").unwrap();
        write(&ws, "mid.md", Some("Middle"), "x", "m").unwrap();

        assert_eq!(
            index_of(&dir),
            "- [alpha](alpha.md) — first one\n- [Middle](mid.md) — x\n- [Zeta](zeta.md) — last one\n"
        );
    }

    #[test]
    fn the_index_follows_the_notes_not_the_other_way_round() {
        // A hand edit to the index is overwritten on the next write: the notes
        // are the source, the index is derived.
        let (dir, ws) = project();
        write(&ws, "a.md", None, "one", "x").unwrap();
        std::fs::write(dir.path().join(".yaca/memory/MEMORY.md"), "garbage").unwrap();
        write(&ws, "b.md", None, "two", "y").unwrap();
        assert_eq!(
            index_of(&dir),
            "- [a](a.md) — one\n- [b](b.md) — two\n"
        );
    }

    #[test]
    fn deleting_the_last_note_removes_the_index() {
        let (dir, ws) = project();
        write(&ws, "only.md", None, "h", "b").unwrap();
        assert!(dir.path().join(".yaca/memory/MEMORY.md").is_file());
        delete(&ws, "only.md").unwrap();
        assert!(!dir.path().join(".yaca/memory/MEMORY.md").exists());
    }

    #[test]
    fn a_hook_cannot_break_out_of_the_front_matter() {
        let (dir, ws) = project();
        write(&ws, "x.md", None, "fine\n---\nname: evil", "body").unwrap();
        let text = read(&ws, "x.md").unwrap();
        assert!(text.contains("description: fine --- name: evil\n---\n"), "{text}");
        assert_eq!(index_of(&dir), "- [x](x.md) — fine --- name: evil\n");
    }

    #[test]
    fn a_hook_is_required_and_bounded() {
        let (_dir, ws) = project();
        assert!(write(&ws, "x.md", None, "   ", "body").is_err());
        let long = "h".repeat(MAX_HOOK_CHARS + 1);
        assert!(write(&ws, "x.md", None, &long, "body").is_err());
    }

    #[test]
    fn names_that_could_escape_or_collide_are_rejected() {
        let (dir, ws) = project();
        let bad = [
            "../x.md",
            "..\\x.md",
            "a/b.md",
            "a\\b.md",
            "/abs.md",
            "C:x.md",
            "x.md.md",
            "X.md",
            "MEMORY.md",
            "memory.md",
            "con.md",
            "nul.md",
            "com1.md",
            "a.txt",
            ".md",
            "x",
            "sp ace.md",
            "",
        ];
        for name in bad {
            let err = write(&ws, name, None, "h", "b").unwrap_err();
            assert!(matches!(err, ToolError::Rejected(_)), "{name:?} -> {err:?}");
            assert!(read(&ws, name).is_err(), "{name:?}");
        }
        // Nothing was written anywhere, not even the directory.
        assert!(!dir.path().join(".yaca").exists());
    }

    #[test]
    fn a_name_that_is_only_a_link_out_of_the_project_is_refused() {
        // A symlink named like the notes directory, pointing outside, must be
        // caught by the workspace's canonical check, not by the name check.
        let (dir, ws) = project();
        let outside = TempDir::new();
        std::fs::create_dir_all(dir.path().join(".yaca")).unwrap();
        if !symlink_dir(outside.path(), &dir.path().join(".yaca/memory")) {
            return; // Creating symlinks needs a privilege some Windows setups lack.
        }
        let err = write(&ws, "x.md", None, "h", "b").unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
        assert!(!outside.path().join("x.md").exists());
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[test]
    fn an_absent_directory_has_no_index_for_the_prompt() {
        let (_dir, ws) = project();
        assert_eq!(load(&ws), None);
    }

    #[test]
    fn an_index_is_loaded_whole_when_it_fits() {
        let (_dir, ws) = project();
        write(&ws, "a.md", None, "one", "x").unwrap();
        let index = load(&ws).unwrap();
        assert_eq!(index.text, "- [a](a.md) — one\n");
        assert!(!index.truncated);
    }

    #[test]
    fn a_long_index_is_cut_at_the_cap_and_says_so() {
        let (dir, ws) = project();
        std::fs::create_dir_all(dir.path().join(".yaca/memory")).unwrap();
        // Written past the cap by hand: the tool never writes one this long,
        // but a hand edit or a long list of notes can.
        std::fs::write(
            dir.path().join(".yaca/memory/MEMORY.md"),
            "- [x](x.md) — ".to_string() + &"é".repeat(MAX_INDEX_BYTES),
        )
        .unwrap();
        let index = load(&ws).unwrap();
        assert!(index.truncated);
        assert!(index.text.len() <= MAX_INDEX_BYTES);
        // The cut fell inside a two-byte character: the split one is dropped,
        // the whole ones before it are kept.
        assert!(index.text.starts_with("- [x](x.md) — é"));
    }

    #[test]
    fn files_that_are_not_notes_are_left_out_of_the_index() {
        let (dir, ws) = project();
        write(&ws, "real.md", None, "yes", "x").unwrap();
        std::fs::write(dir.path().join(".yaca/memory/notes.txt"), "no").unwrap();
        std::fs::write(dir.path().join(".yaca/memory/Upper.md"), "no").unwrap();
        // Rebuild, so the stray files would show if they were listed.
        write(&ws, "real2.md", None, "also", "y").unwrap();
        assert_eq!(index_of(&dir), "- [real](real.md) — yes\n- [real2](real2.md) — also\n");
    }

    #[test]
    fn a_note_without_front_matter_is_listed_by_its_file_name() {
        let (dir, ws) = project();
        write(&ws, "fine.md", None, "h", "x").unwrap();
        std::fs::write(dir.path().join(".yaca/memory/plain.md"), "just text\n").unwrap();
        write(&ws, "other.md", None, "o", "y").unwrap();
        assert_eq!(
            index_of(&dir),
            "- [fine](fine.md) — h\n- [other](other.md) — o\n- [plain](plain.md)\n"
        );
    }

    #[test]
    fn the_index_is_identical_however_the_notes_were_written() {
        let (dir_a, ws_a) = project();
        write(&ws_a, "b.md", None, "two", "x").unwrap();
        write(&ws_a, "a.md", None, "one", "y").unwrap();

        let (dir_b, ws_b) = project();
        write(&ws_b, "a.md", None, "one", "y").unwrap();
        write(&ws_b, "b.md", None, "two", "x").unwrap();

        assert_eq!(index_of(&dir_a), index_of(&dir_b));
    }
}

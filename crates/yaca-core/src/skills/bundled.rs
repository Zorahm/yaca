//! The office skills Yaca ships with: Word, Excel, PowerPoint and PDF.
//!
//! A chat about documents is only half done without them. What a model gets
//! wrong about an office file is not the library — it is the shape of the
//! problem: that a `.docx` is a ZIP, that a formula has to be written as a
//! formula, that a deck built from the default template keeps its theme. These
//! are instruction files written for that, and they are small enough to ship
//! inside the binary.
//!
//! They are installed into the human's skills folder, not read from the
//! binary: the `Skill` tool lists the files beside `SKILL.md` and reads them
//! by path, and a skill whose directory is nowhere cannot hand over its
//! scripts. That also makes them visible — and editable — like any other skill
//! on the machine.
//!
//! An installed file is rewritten when the bundled text changes and left alone
//! otherwise, so a skill added by a later version arrives without an update
//! script and the folder is not touched on every start. A project skill of
//! the same name overrides the bundled copy in that project.

use std::path::Path;

/// The version of the bundled set. A file written by an older version is
/// replaced; the same version is not written again.
const VERSION: &str = "1.2.0";

/// The marker naming the version installed in a folder.
const MARKER: &str = ".yaca-bundled";

const LICENSE: &str = "ANTHROPIC-LICENSE.txt";

/// One file of a bundled skill, relative to its directory.
struct File {
    path: &'static str,
    text: &'static str,
}

struct Bundled {
    name: &'static str,
    files: &'static [File],
}

/// The skills and their files.
const BUNDLED: &[Bundled] = &[
    Bundled {
        name: "docx",
        files: &[
            File {
                path: "SKILL.md",
                text: include_str!("../../bundled/skills/docx/SKILL.md"),
            },
            File {
                path: "scripts/replace.py",
                text: include_str!("../../bundled/skills/docx/scripts/replace.py"),
            },
        ],
    },
    Bundled {
        name: "xlsx",
        files: &[File {
            path: "SKILL.md",
            text: include_str!("../../bundled/skills/xlsx/SKILL.md"),
        }],
    },
    Bundled {
        name: "pptx",
        files: &[File {
            path: "SKILL.md",
            text: include_str!("../../bundled/skills/pptx/SKILL.md"),
        }],
    },
    Bundled {
        name: "pdf",
        files: &[File {
            path: "SKILL.md",
            text: include_str!("../../bundled/skills/pdf/SKILL.md"),
        }],
    },
];

/// Writes the bundled skills into `skills_dir` (the human's skills folder),
/// and answers how many files it wrote.
///
/// A folder that cannot be written is not an error the session should die of:
/// the skills are an addition, and a session without them is the session Yaca
/// had before. The caller decides what to say; `Err` carries the reason.
pub fn install(skills_dir: &Path) -> Result<usize, String> {
    let marker = skills_dir.join(MARKER);
    if std::fs::read_to_string(&marker).is_ok_and(|text| text.trim() == VERSION) {
        return Ok(0);
    }

    let mut written = usize::from(install_file(
        &skills_dir.join(LICENSE),
        include_str!("../../bundled/skills/ANTHROPIC-LICENSE.txt"),
    )?);
    for skill in BUNDLED {
        for file in skill.files {
            let path = skills_dir.join(skill.name).join(file.path);
            written += usize::from(install_file(&path, file.text)?);
        }
    }
    std::fs::write(&marker, VERSION)
        .map_err(|e| format!("could not write {}: {e}", marker.display()))?;
    Ok(written)
}

fn install_file(path: &Path, text: &str) -> Result<bool, String> {
    // Keep the timestamp when only the checkout's line endings changed.
    if std::fs::read_to_string(path).is_ok_and(|current| same_text(&current, text)) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(true)
}

/// The names of the bundled skills, for a message that says what arrived.
pub fn names() -> Vec<&'static str> {
    BUNDLED.iter().map(|skill| skill.name).collect()
}

/// Two texts with the same content, whatever each one uses for a line break.
fn same_text(a: &str, b: &str) -> bool {
    a == b || a.replace("\r\n", "\n") == b.replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// The folder of the human's own skills, as the applications pass it.
    fn skills_dir(dir: &TempDir) -> std::path::PathBuf {
        dir.path().join("skills")
    }

    #[test]
    fn the_office_skills_are_written_with_their_scripts() {
        let dir = TempDir::new();
        let folder = skills_dir(&dir);

        assert_eq!(install(&folder).unwrap(), 6);
        let docx = std::fs::read_to_string(folder.join("docx/SKILL.md")).unwrap();
        assert!(docx.replace("\r\n", "\n").starts_with("---\nname: docx\n"));
        assert!(folder.join("docx/scripts/replace.py").is_file());
        assert!(folder.join("xlsx/SKILL.md").is_file());
        assert!(folder.join("pptx/SKILL.md").is_file());
        assert!(folder.join("pdf/SKILL.md").is_file());
        assert!(folder.join(LICENSE).is_file());
        assert_eq!(names(), ["docx", "xlsx", "pptx", "pdf"]);
    }

    #[test]
    fn a_second_run_writes_nothing() {
        let dir = TempDir::new();
        let folder = skills_dir(&dir);
        install(&folder).unwrap();
        assert_eq!(install(&folder).unwrap(), 0);
    }

    #[test]
    fn a_file_from_an_older_version_is_replaced() {
        let dir = TempDir::new();
        let folder = skills_dir(&dir);
        std::fs::create_dir_all(folder.join("docx")).unwrap();
        std::fs::write(folder.join("docx/SKILL.md"), "old text").unwrap();
        std::fs::write(folder.join(MARKER), "1.1.0").unwrap();

        assert!(install(&folder).unwrap() > 0);
        let docx = std::fs::read_to_string(folder.join("docx/SKILL.md")).unwrap();
        assert!(docx.contains("name: docx"));
        assert!(folder.join("pdf/SKILL.md").is_file());
    }

    #[test]
    fn the_skills_load_through_the_catalog() {
        // The point of installing them: the `Skill` tool finds them by name.
        let dir = TempDir::new();
        let folder = skills_dir(&dir);
        install(&folder).unwrap();

        let catalog = crate::skills::SkillRoots::new(dir.path(), vec![folder]).scan();
        for name in ["docx", "xlsx", "pptx", "pdf"] {
            assert!(catalog.get(name).is_some(), "{name} missing");
        }
        let listing = catalog.listing().unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "docx"));
    }
}

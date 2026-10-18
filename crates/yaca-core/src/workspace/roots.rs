//! The set of folders a workspace works in, and the path forms that go with it.
//!
//! One primary root (the project, or a chat's worktree) and zero or more
//! additional roots. Everything here is about forms and sets; the boundary rule
//! itself lives in `workspace.rs` and asks [`in_root_form`] before it compares.
//!
//! Windows detail that the rest of the core depends on: `canonicalize` returns
//! a verbatim path (`\\?\D:\proj`) and `Path::starts_with` does not treat it as
//! equal to `D:\proj`. The primary root has always been stored verbatim, so
//! absolute paths typed by a model are brought into that form before they are
//! compared, and additional roots are stored the same way. Shown to a person or
//! a model, the verbatim prefix is removed again ([`plain`]).

use std::path::{Component, Path, PathBuf, Prefix};

use crate::error::CoreError;

/// Whether `path` starts with a verbatim (`\\?\`) prefix.
fn is_verbatim(path: &Path) -> bool {
    matches!(path.components().next(), Some(Component::Prefix(p)) if p.kind().is_verbatim())
}

/// `path` in the same form as `like`: a drive-letter path becomes verbatim when
/// `like` is verbatim, so the two can be compared. Anything else is returned
/// as it is; on other systems no path has a prefix and this is a no-op.
pub(crate) fn in_root_form(path: &Path, like: &Path) -> PathBuf {
    let disk = matches!(
        path.components().next(),
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_))
    );
    if disk && is_verbatim(like) && !is_verbatim(path) {
        // Verbatim paths are not normalised, so the separators are made plain
        // here; a `/` left in would not match the `\` that `canonicalize` wrote.
        let text = path.display().to_string().replace('/', r"\");
        PathBuf::from(format!(r"\\?\{text}"))
    } else {
        path.to_path_buf()
    }
}

/// How a path is shown: the verbatim prefix taken off, forward slashes.
///
/// Used for the additional roots, which the model and the person see whole
/// (their absolute form). A relative form could be taken for a file in the
/// primary root, which is why it is never shortened.
pub(crate) fn plain(path: &Path) -> String {
    let text = path.display().to_string();
    let text = match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => text.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(text),
    };
    text.replace('\\', "/")
}

/// Checks the additional roots a session asked for, and returns the ones that
/// stay, canonical and without redundancy.
///
/// Dropped without error: a folder equal to the primary root or inside it (the
/// primary root already covers it), a duplicate, and a folder inside another
/// additional root. Refused: a folder that is not a directory, and one that
/// contains the primary root, because granting it would widen the boundary to
/// the parent of the project without anyone saying so.
pub(crate) fn validate(primary: &Path, given: &[PathBuf]) -> Result<Vec<PathBuf>, CoreError> {
    let mut kept: Vec<PathBuf> = Vec::new();
    for raw in given {
        let canon = std::fs::canonicalize(raw).map_err(|e| {
            CoreError::Config(format!(
                "failed to open additional root {}: {e}",
                raw.display()
            ))
        })?;
        if !canon.is_dir() {
            return Err(CoreError::Config(format!(
                "{} is not a directory",
                canon.display()
            )));
        }
        if canon.starts_with(primary) {
            continue;
        }
        if primary.starts_with(&canon) {
            return Err(CoreError::Config(format!(
                "{} contains the project itself {}: boundaries would become wider than the project",
                canon.display(),
                primary.display()
            )));
        }
        if !kept.contains(&canon) {
            kept.push(canon);
        }
    }

    let all = kept.clone();
    kept.retain(|root| !all.iter().any(|other| other != root && root.starts_with(other)));
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_form_drops_the_verbatim_prefix_and_uses_forward_slashes() {
        assert_eq!(plain(Path::new(r"\\?\D:\work\src")), "D:/work/src");
        assert_eq!(plain(Path::new(r"\\?\UNC\srv\share\x")), "//srv/share/x");
        assert_eq!(plain(Path::new("/already/plain")), "/already/plain");
    }

    #[cfg(windows)]
    #[test]
    fn a_drive_path_takes_the_verbatim_form_of_its_root() {
        let like = Path::new(r"\\?\D:\project");
        assert_eq!(
            in_root_form(Path::new("D:/other/a.txt"), like),
            PathBuf::from(r"\\?\D:\other\a.txt")
        );
        // Already verbatim, or no verbatim root to match: left alone.
        assert_eq!(in_root_form(Path::new(r"\\?\D:\x"), like), PathBuf::from(r"\\?\D:\x"));
        assert_eq!(
            in_root_form(Path::new(r"D:\x"), Path::new(r"D:\project")),
            PathBuf::from(r"D:\x")
        );
    }
}

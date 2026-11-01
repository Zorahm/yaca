//! A command's starting directory is independent of the file tools' boundaries.
//!
//! Commands already run with the user's filesystem access. The project supplies
//! a default and a base for relative paths, not a sandbox for the shell.

use std::path::PathBuf;

use crate::error::ToolError;
use crate::workspace::Workspace;

pub(super) fn resolve(ws: &Workspace, raw: &str) -> Result<PathBuf, ToolError> {
    let dir = std::fs::canonicalize(ws.root().join(raw))
        .map_err(|e| ToolError::failed(format!("cannot open working directory {raw}: {e}")))?;
    if !dir.is_dir() {
        return Err(ToolError::failed(format!("{raw} is not a directory")));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn absolute_and_relative_paths_can_leave_the_project() {
        let dir = TempDir::new();
        dir.mkdir("project");
        dir.mkdir("tools");
        let ws = Workspace::open(dir.path().join("project")).unwrap();
        let expected = std::fs::canonicalize(dir.path().join("tools")).unwrap();
        assert_eq!(resolve(&ws, "../tools").unwrap(), expected);
        assert_eq!(resolve(&ws, &expected.to_string_lossy()).unwrap(), expected);
    }

    #[test]
    fn a_missing_directory_or_a_file_is_a_failure() {
        let dir = TempDir::new();
        dir.write("file.txt", "data");
        let ws = Workspace::open(dir.path()).unwrap();
        for raw in ["missing", "file.txt"] {
            assert!(matches!(resolve(&ws, raw), Err(ToolError::Failed { .. })));
        }
    }
}

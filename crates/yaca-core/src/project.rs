//! Where a project keeps its own things (§9): settings, the session journals,
//! the shadow store and its slash commands, all in one folder at its root.
//!
//! One function computes the path, so the name is written once, and the folder
//! the edoC build left behind is brought over the first time anyone asks —
//! see [`crate::legacy`].

use std::path::{Path, PathBuf};

use crate::legacy;

/// The folder's name at the project root.
pub const DIR: &str = ".yaca";

/// The project's folder, moved over from its old name on first use.
pub fn dir(root: &Path) -> PathBuf {
    legacy::adopt_dir(&root.join(legacy::PROJECT_DIR), &root.join(DIR))
}

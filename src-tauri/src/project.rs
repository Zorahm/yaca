//! One project's own settings: `.yaca/settings.json` (§10).
//!
//! The shape belongs to the core ([`yaca_core::Settings`]) and so does the
//! writing, so this file is only the door the settings window knocks on. Two
//! commands and nothing else: a second description of the file here would be
//! a second thing to keep in step.
//!
//! **These settings describe how a session starts.** The core reads the file
//! at `spawn` and keeps what it read, so a mode or a rule changed here reaches
//! the next chat, not the one already running — which is why §10 calls it
//! "default mode". Reaching into a live session is what the composer's
//! `⇧⇥` is for.

use std::path::PathBuf;

use yaca_core::Settings;

/// The same canonical spelling every other path in the shell goes through.
fn root(path: &str) -> Result<PathBuf, String> {
    crate::project_path(path)
}

/// What the project would start a session with, as it is on disk.
///
/// A file that does not parse comes back as an error rather than as the
/// defaults: the settings window must not show an empty rule list to a human
/// whose rules are merely mistyped (§0).
#[tauri::command]
pub fn project_settings(project: String) -> Result<Settings, String> {
    Settings::load(&root(&project)?).map_err(|e| e.to_string())
}

/// Writes the file whole.
///
/// Whole, not field by field: the window holds the settings it just read, and
/// a patch API would have to answer what happens when two windows patch at
/// once. One project, one settings window, one write.
#[tauri::command]
pub fn save_project_settings(project: String, settings: Settings) -> Result<(), String> {
    settings
        .save(&root(&project)?)
        .map_err(|e| e.to_string())
}

/// Directories the snapshot walk never enters, whatever the project says
/// (§5.8, §10).
///
/// Asked of the core rather than written down in the window: the list is the
/// core's, and a copy in the frontend would drift from it on the first
/// addition.
#[tauri::command]
pub fn shadow_excluded_dirs() -> Vec<&'static str> {
    yaca_core::checkpoint::scan::excluded_dirs().to_vec()
}

/// Which instructions file the project actually uses, relative to its root
/// (§10), or `None` when it keeps none.
///
/// The path, never the text: the settings window says which file is in force
/// and offers to open it, and pumping 64 KB of prompt through the bridge to
/// show one line would be paying for a thing nobody reads here.
#[tauri::command]
pub fn project_instructions(project: String) -> Result<Option<String>, String> {
    let ws = yaca_core::Workspace::open(root(&project)?).map_err(|e| e.to_string())?;
    // The root file only: nested ones govern their own directories and are
    // not "the" instructions file of the project.
    Ok(yaca_core::instructions::load(&ws)
        .and_then(|found| found.root)
        .map(|root| root.path))
}

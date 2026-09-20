//! What the build called edoC left behind, and how it is brought over.
//!
//! The application was renamed to Yaca, and with it every place that holds
//! the human's things: the settings directory, the keychain service, the
//! folder at each project's root, and the window's webview profile. Each is
//! moved on first contact rather than read in place, so there is one copy of
//! it and nothing to keep in step. The moves live in the core, so whichever of
//! the window and `yaca-cli` starts first makes them; the webview profile is
//! the shell's, and it calls [`adopt_dir`] from here.
//!
//! Everything about the old name is here, so it can go in one piece once no
//! installation has anything left under it.

use std::path::{Path, PathBuf};

/// The settings directory and the keychain service before the rename.
pub const NAME: &str = "edoC";

/// The folder at a project's root before the rename.
pub const PROJECT_DIR: &str = ".edoc";

/// The window's identifier before the rename. The webview keeps its profile —
/// `localStorage` with the composer drafts (§7.4) — in a directory named after
/// it.
pub const IDENTIFIER: &str = "com.edoc.desktop";

/// Moves `old` to `new` if there is something to move, and says where the
/// data is now.
///
/// Only while `new` does not exist: a directory there means Yaca has been used
/// already, and what it wrote is newer than anything under the old name.
///
/// A move that fails leaves the data where it was, and that is where it is
/// used from until a later call manages the move. A fresh, empty `new` beside
/// it would be a second version of the truth: providers and projects gone
/// from one, sessions written into the other.
pub fn adopt_dir(old: &Path, new: &Path) -> PathBuf {
    if new.exists() || !old.is_dir() {
        return new.to_path_buf();
    }
    match std::fs::rename(old, new) {
        Ok(()) => new.to_path_buf(),
        // The window and the CLI can start together; the other one moved it.
        Err(_) if new.exists() => new.to_path_buf(),
        Err(e) => {
            tracing::warn!("не перенести {} в {}: {e}", old.display(), new.display());
            old.to_path_buf()
        }
    }
}

/// Moves a provider's key from the old keychain service to the new one.
///
/// The old key is deleted only once the new one reads back. A successful write
/// is not enough: the Windows Credential Manager loses a write now and then
/// when several land at once — measured at 4 of 480 from eight threads, none
/// of 200 in sequence — and deleting on the strength of such a write would
/// leave the key nowhere. [`crate::secrets`] serializes the process's own
/// calls; the read-back covers the window and the CLI writing together.
///
/// A key already under the new name wins — it was typed after the rename —
/// and the old one is only dropped.
///
/// Failures are not errors, as nowhere in reading the keychain: the key stays
/// where it was and the next call tries again.
pub(crate) fn adopt_key(old_service: &str, new_service: &str, provider: &str) {
    let (Ok(old), Ok(new)) = (
        keyring::Entry::new(old_service, provider),
        keyring::Entry::new(new_service, provider),
    ) else {
        return;
    };
    let Ok(key) = old.get_password() else {
        return;
    };
    match new.get_password() {
        Ok(_) => {}
        Err(keyring::Error::NoEntry) => {
            if let Err(e) = new.set_password(&key) {
                tracing::warn!(provider, "ключ не перенесён под новое имя: {e}");
                return;
            }
            if new.get_password().ok().as_deref() != Some(key.as_str()) {
                tracing::warn!(provider, "ключ под новым именем не читается, старый оставлен");
                return;
            }
        }
        // Unreadable is not absent: writing over it could replace a newer key.
        Err(_) => return,
    }
    let _ = old.delete_credential();
}

/// Canonicalizes old tool names from previous builds to new ones.
///
/// When older versions wrote standing permission rules (`.yaca/settings.json`)
/// or session journals (`.yaca/sessions/`), they used lowercase snake_case
/// identifiers (`read_file`, `bash`, etc.). The core maps them to their
/// PascalCase identifiers on read so disk data continues to work.
pub fn canonical_tool_name(name: &str) -> &str {
    match name {
        "read_file" => "Read",
        "grep" => "Grep",
        "glob" => "Glob",
        "write_file" => "Write",
        "edit_file" => "Edit",
        "delete_file" => "Delete",
        "move_file" => "Move",
        "bash" => "Bash",
        "bash_output" => "BashOutput",
        "web_fetch" => "WebFetch",
        "todo_write" => "TodoWrite",
        "exit_plan_mode" => "ExitPlanMode",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn old_tool_names_map_to_new_ones() {
        assert_eq!(canonical_tool_name("read_file"), "Read");
        assert_eq!(canonical_tool_name("grep"), "Grep");
        assert_eq!(canonical_tool_name("glob"), "Glob");
        assert_eq!(canonical_tool_name("write_file"), "Write");
        assert_eq!(canonical_tool_name("edit_file"), "Edit");
        assert_eq!(canonical_tool_name("delete_file"), "Delete");
        assert_eq!(canonical_tool_name("move_file"), "Move");
        assert_eq!(canonical_tool_name("bash"), "Bash");
        assert_eq!(canonical_tool_name("bash_output"), "BashOutput");
        assert_eq!(canonical_tool_name("web_fetch"), "WebFetch");
        assert_eq!(canonical_tool_name("todo_write"), "TodoWrite");
        assert_eq!(canonical_tool_name("exit_plan_mode"), "ExitPlanMode");
        assert_eq!(canonical_tool_name("Read"), "Read");
        assert_eq!(canonical_tool_name("custom"), "custom");
    }

    #[test]
    fn the_old_directory_moves_whole() {
        let base = TempDir::new();
        base.write("edoC/providers.json", r#"{"list":[]}"#);
        base.write("edoC/settings.json", r#"{"projects":[]}"#);
        let new = base.path().join("Yaca");

        let at = adopt_dir(&base.path().join("edoC"), &new);

        assert_eq!(at, new);
        assert!(new.join("providers.json").is_file());
        assert!(new.join("settings.json").is_file());
        // One copy, not two that drift apart.
        assert!(!base.path().join("edoC").exists());
    }

    #[test]
    fn a_new_directory_that_exists_is_left_alone_and_so_is_the_old() {
        let base = TempDir::new();
        base.write("edoC/providers.json", "old");
        base.write("Yaca/providers.json", "new");
        let new = base.path().join("Yaca");

        let at = adopt_dir(&base.path().join("edoC"), &new);

        // What Yaca wrote is newer than anything under the old name.
        assert_eq!(at, new);
        let kept = std::fs::read_to_string(new.join("providers.json")).unwrap();
        assert_eq!(kept, "new");
        assert!(base.path().join("edoC/providers.json").is_file());
    }

    #[test]
    fn nothing_to_move_creates_nothing() {
        let base = TempDir::new();
        let new = base.path().join("Yaca");

        let at = adopt_dir(&base.path().join("edoC"), &new);

        assert_eq!(at, new);
        assert!(!new.exists());
    }

    #[test]
    fn a_move_that_fails_leaves_the_data_where_it_was_and_uses_it_there() {
        let base = TempDir::new();
        base.write("edoC/providers.json", "old");
        let old = base.path().join("edoC");
        // A parent that does not exist: the rename cannot succeed.
        let new = base.path().join("missing").join("Yaca");

        let at = adopt_dir(&old, &new);

        // An empty new folder beside the old one would be a second truth.
        assert_eq!(at, old);
        assert!(old.join("providers.json").is_file());
        assert!(!new.exists());
    }

    /// The real keychain, under throwaway service names: a mock would test
    /// the mock. Windows only, because the Credential Manager is always there;
    /// a Linux runner without a secret service has nothing to move keys in.
    #[cfg(windows)]
    mod keychain {
        use super::*;

        use std::sync::MutexGuard;

        const PROVIDER: &str = "legacy-test";

        /// Two service names nobody else uses, emptied on drop even if the
        /// test fails halfway.
        ///
        /// Holds the process's keychain lock for the whole test: parallel
        /// tests writing at once lose entries the same way parallel callers
        /// do (see [`adopt_key`]), and a test that fails for that reason tests
        /// the Credential Manager, not the move.
        struct Services {
            old: String,
            new: String,
            // Declared last so it is released after `drop` has cleaned up.
            _serial: MutexGuard<'static, ()>,
        }

        impl Services {
            fn new() -> Self {
                let tag = uuid::Uuid::new_v4().simple().to_string();
                Services {
                    old: format!("yaca-test-old-{tag}"),
                    new: format!("yaca-test-new-{tag}"),
                    _serial: crate::secrets::serial(),
                }
            }

            fn entry(service: &str) -> keyring::Entry {
                keyring::Entry::new(service, PROVIDER).unwrap()
            }

            fn put(service: &str, key: &str) {
                Self::entry(service).set_password(key).unwrap();
            }

            fn get(service: &str) -> Option<String> {
                Self::entry(service).get_password().ok()
            }
        }

        impl Drop for Services {
            fn drop(&mut self) {
                let _ = Self::entry(&self.old).delete_credential();
                let _ = Self::entry(&self.new).delete_credential();
            }
        }

        #[test]
        fn a_key_under_the_old_name_moves_to_the_new() {
            let s = Services::new();
            Services::put(&s.old, "sk-old");

            adopt_key(&s.old, &s.new, PROVIDER);

            assert_eq!(Services::get(&s.new).as_deref(), Some("sk-old"));
            assert_eq!(Services::get(&s.old), None);
        }

        #[test]
        fn a_key_already_under_the_new_name_wins() {
            let s = Services::new();
            Services::put(&s.old, "sk-old");
            Services::put(&s.new, "sk-typed-later");

            adopt_key(&s.old, &s.new, PROVIDER);

            assert_eq!(Services::get(&s.new).as_deref(), Some("sk-typed-later"));
            // The stale one does not stay behind to be found later.
            assert_eq!(Services::get(&s.old), None);
        }

        #[test]
        fn no_old_key_writes_nothing() {
            let s = Services::new();

            adopt_key(&s.old, &s.new, PROVIDER);

            assert_eq!(Services::get(&s.new), None);
        }
    }
}

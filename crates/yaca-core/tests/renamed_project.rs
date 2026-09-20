//! A project written by the edoC build opens in Yaca with everything it had (§9).
//!
//! The folder at the project root was `.edoc` before the rename. It is moved to
//! `.yaca` the first time the core looks, and these tests check through the
//! public surface that what was kept there — settings, sessions, slash
//! commands — is found afterwards.

mod common;

use common::TempProject;
use yaca_core::{slash, Mode, SessionConfig, SessionStore};

#[test]
fn an_old_project_folder_moves_and_everything_in_it_is_found() {
    let project = TempProject::new();
    project.write(".edoc/settings.json", r#"{"mode":"auto"}"#);
    // A session is listed by its feed journal; the index sits beside it.
    project.write(".edoc/sessions/ses_old.jsonl", "");
    project.write(
        ".edoc/sessions/ses_old.meta.json",
        r#"{"id":"ses_old","title":"slugify","createdAt":1,"updatedAt":1,"model":"ornith"}"#,
    );
    project.write(".edoc/commands/review.md", "Просмотри изменения.");

    let config = SessionConfig::open(project.path()).unwrap();

    assert_eq!(config.mode, Mode::Auto);
    assert_eq!(SessionStore::open(project.path()).list().len(), 1);
    assert_eq!(slash::load(project.path()).len(), 1);
    // One folder, not two that drift apart.
    assert!(project.path().join(".yaca/settings.json").is_file());
    assert!(!project.path().join(".edoc").exists());
}

#[test]
fn a_project_already_on_the_new_name_is_read_from_it() {
    let project = TempProject::new();
    project.write(".yaca/settings.json", r#"{"mode":"auto"}"#);
    project.write(".edoc/settings.json", r#"{"mode":"plan"}"#);

    let config = SessionConfig::open(project.path()).unwrap();

    // What Yaca wrote is newer than anything under the old name.
    assert_eq!(config.mode, Mode::Auto);
    assert!(project.path().join(".edoc/settings.json").is_file());
}

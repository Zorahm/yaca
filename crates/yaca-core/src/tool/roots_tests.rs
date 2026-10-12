//! Tools across several roots, on a real filesystem.
//!
//! Each test makes two directories of its own: a primary root (the project)
//! and an additional root beside it. The tools are called the way the model
//! calls them, with an absolute path for the second root, in the form the
//! environment note shows (no verbatim prefix, forward slashes).

use std::path::{Path, PathBuf};

use serde_json::json;

use super::bash::Bash;
use super::delete_dir::DeleteDir;
use super::edit_file::EditFile;
use super::glob::Glob;
use super::grep::Grep;
use super::read_file::ReadFile;
use super::tests_support::ctx_on;
use super::write_file::WriteFile;
use super::{Tool, ToolCtx};
use crate::permission::ScopeKey;
use crate::testutil::TempDir;
use crate::workspace::{roots, Workspace};

/// The project, and one additional root beside it.
fn two_roots() -> (TempDir, TempDir, Workspace) {
    let main = TempDir::new();
    let extra = TempDir::new();
    let ws = Workspace::open(main.path())
        .unwrap()
        .with_roots(&[extra.path().to_path_buf()])
        .unwrap();
    (main, extra, ws)
}

/// How the model names a file in `root`: absolute, without `\\?\`.
fn model_path(root: &Path, rel: &str) -> String {
    roots::plain(&root.join(rel))
}

fn context(ws: Workspace) -> ToolCtx {
    ctx_on(ws).0
}

#[tokio::test]
async fn reads_edits_and_writes_a_file_in_the_additional_root() {
    let (main, extra, ws) = two_roots();
    main.write("a.txt", "primary\n");
    let notes = extra.write("notes/todo.txt", "one\n");
    let ctx = context(ws);
    let path = model_path(extra.path(), "notes/todo.txt");

    let read = ReadFile
        .run(&ctx, json!({ "path": path }))
        .await
        .expect("a file in an additional root is readable by absolute path");
    assert!(read.text.contains("one"), "{}", read.text);

    EditFile
        .run(
            &ctx,
            json!({ "path": path, "old_string": "one", "new_string": "two" }),
        )
        .await
        .expect("an edit after a read lands in the additional root");
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "two\n");

    let created = model_path(extra.path(), "new/fresh.txt");
    WriteFile
        .run(&ctx, json!({ "path": created, "content": "fresh\n" }))
        .await
        .expect("a new file in the additional root is written");
    assert_eq!(
        std::fs::read_to_string(extra.path().join("new/fresh.txt")).unwrap(),
        "fresh\n"
    );

    // A relative path still means the project, and only the project.
    WriteFile
        .run(&ctx, json!({ "path": "b.txt", "content": "x\n" }))
        .await
        .unwrap();
    assert!(main.path().join("b.txt").exists());
    assert!(!extra.path().join("b.txt").exists());
}

#[tokio::test]
async fn an_edit_in_the_additional_root_needs_a_read_first_like_anywhere_else() {
    let (_main, extra, ws) = two_roots();
    extra.write("n.txt", "x\n");
    let ctx = context(ws);
    let err = EditFile
        .run(
            &ctx,
            json!({
                "path": model_path(extra.path(), "n.txt"),
                "old_string": "x",
                "new_string": "y",
            }),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("не прочитан"), "{err}");
}

#[tokio::test]
async fn glob_and_grep_find_files_in_both_roots() {
    let (main, extra, ws) = two_roots();
    main.write("src/a.rs", "fn main() {}\n");
    extra.write("src/b.rs", "fn helper() {}\n");
    let ctx = context(ws);

    let found = Glob
        .run(&ctx, json!({ "pattern": "src/**/*.rs" }))
        .await
        .unwrap()
        .text;
    assert!(found.contains("src/a.rs"), "{found}");
    // Shown by its absolute path: a relative `src/b.rs` would look like the
    // project's file.
    let shown_b = model_path(extra.path(), "src/b.rs");
    assert!(found.contains(&shown_b), "{found}");

    let grepped = Grep::new()
        .run(&ctx, json!({ "pattern": "fn ", "mode": "files" }))
        .await
        .unwrap()
        .text;
    assert!(grepped.contains("src/a.rs"), "{grepped}");
    assert!(grepped.contains(&shown_b), "{grepped}");

    // With a path, the search is confined to that root's folder.
    let only_extra = Grep::new()
        .run(
            &ctx,
            json!({ "pattern": "fn ", "mode": "files", "path": model_path(extra.path(), "src") }),
        )
        .await
        .unwrap()
        .text;
    assert!(only_extra.contains(&shown_b), "{only_extra}");
    assert!(!only_extra.contains("src/a.rs"), "{only_extra}");
}

#[tokio::test]
async fn a_glob_pattern_is_matched_inside_the_root_that_holds_the_file() {
    let (main, extra, ws) = two_roots();
    main.write("docs/readme.md", "x\n");
    extra.write("docs/guide.md", "x\n");
    let ctx = context(ws);

    // `docs/*.md` names the docs of each root; it does not see a root's
    // absolute name as a folder called after the drive.
    let found = Glob
        .run(&ctx, json!({ "pattern": "docs/*.md" }))
        .await
        .unwrap()
        .text;
    assert!(found.contains("docs/readme.md"), "{found}");
    assert!(found.contains(&model_path(extra.path(), "docs/guide.md")), "{found}");
}

#[tokio::test]
async fn delete_dir_refuses_the_additional_root_itself() {
    let (_main, extra, ws) = two_roots();
    extra.write("keep.txt", "x\n");
    let ctx = context(ws);
    let input = json!({ "path": roots::plain(extra.path()) });

    let err = DeleteDir.run(&ctx, input.clone()).await.unwrap_err();
    assert!(err.to_string().contains("дополнительный корень"), "{err}");
    assert!(extra.path().join("keep.txt").exists(), "the root and its files stay");

    // Refused before any question: there is nothing to ask about.
    assert!(DeleteDir.permission(&ctx.ws, &input).is_none());
}

#[tokio::test]
async fn a_write_in_the_additional_root_is_reported_as_not_covered_by_rollback() {
    let (main, extra, ws) = two_roots();
    main.write("a.txt", "x\n");
    extra.write("n.txt", "x\n");

    let elsewhere = WriteFile
        .permission(&ws, &json!({ "path": model_path(extra.path(), "new.txt"), "content": "" }))
        .expect("a write is a question");
    let lost = elsewhere.unsaved.expect("the checkpoint never saw this root");
    assert_eq!(lost.roots, vec![roots::plain(extra.path())]);

    let inside = WriteFile
        .permission(&ws, &json!({ "path": "new.txt", "content": "" }))
        .expect("a write is a question");
    assert_eq!(inside.unsaved, None);

    // An edit reaches the same root through the same question.
    let edit = EditFile
        .permission(
            &ws,
            &json!({ "path": model_path(extra.path(), "n.txt"), "old_string": "x", "new_string": "y" }),
        )
        .unwrap();
    assert_eq!(edit.unsaved.unwrap().roots, vec![roots::plain(extra.path())]);
}

#[tokio::test]
async fn a_rule_for_the_project_does_not_match_the_same_path_in_the_other_root() {
    let (main, extra, ws) = two_roots();
    main.write("src/a.rs", "x\n");
    extra.write("src/a.rs", "x\n");
    let in_extra = EditFile
        .permission(
            &ws,
            &json!({ "path": model_path(extra.path(), "src/a.rs"), "old_string": "x", "new_string": "y" }),
        )
        .unwrap();
    let in_main = EditFile
        .permission(&ws, &json!({ "path": "src/a.rs", "old_string": "x", "new_string": "y" }))
        .unwrap();

    // The offered scopes for the other root are absolute; the project's are not.
    let offered = in_extra.scopes(ws.root());
    let absolute_dir = offered
        .iter()
        .find_map(|scope| match scope {
            ScopeKey::Directory { path } => Some(path.clone()),
            _ => None,
        })
        .expect("a directory rule is offered");
    assert_eq!(absolute_dir, model_path(extra.path(), "src"));

    // The project's relative rule covers the project and nothing else.
    let relative = ScopeKey::Directory { path: "src".into() };
    assert!(in_main.covered_by(&relative, ws.root()));
    assert!(!in_extra.covered_by(&relative, ws.root()));

    // The other root's absolute rule covers its own files, not the project's.
    let absolute = ScopeKey::Directory { path: absolute_dir };
    assert!(in_extra.covered_by(&absolute, ws.root()));
    assert!(!in_main.covered_by(&absolute, ws.root()));
}

#[tokio::test]
async fn a_command_in_the_additional_root_is_reported_with_its_root() {
    let (_main, extra, ws) = two_roots();
    extra.write("notes.txt", "x\n");
    let cwd = roots::plain(extra.path());

    // Read-only commands run without a question, wherever they run.
    let read = Bash.permission(&ws, &json!({ "command": "cat notes.txt", "cwd": cwd }));
    assert!(read.is_none(), "{read:?}");

    let build = Bash
        .permission(&ws, &json!({ "command": "make build", "cwd": cwd }))
        .expect("a command that may write is a question");
    assert_eq!(build.unsaved.unwrap().roots, vec![cwd.clone()]);

    // A command in the project is reported with the same additional roots: a
    // shell line in the project can still write into them.
    let in_project = Bash
        .permission(&ws, &json!({ "command": "make build" }))
        .expect("a question");
    assert_eq!(in_project.unsaved.unwrap().roots, vec![cwd]);
}

#[tokio::test]
async fn a_command_with_no_additional_root_is_not_reported_at_all() {
    let main = TempDir::new();
    let ws = Workspace::open(main.path()).unwrap();
    let need = Bash
        .permission(&ws, &json!({ "command": "make build" }))
        .expect("a question");
    assert_eq!(need.unsaved, None);
}

#[test]
fn roots_lists_the_primary_root_first_then_the_additional_ones() {
    let (main, extra, ws) = two_roots();
    let seen: Vec<PathBuf> = ws.roots().iter().map(|p| p.to_path_buf()).collect();
    assert_eq!(seen, vec![main.path().to_path_buf(), extra.path().to_path_buf()]);
}

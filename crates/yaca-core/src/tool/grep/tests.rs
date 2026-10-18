//! Tests of the tool as the model calls it: matching, context, the walk.
//! Pages and multiline have theirs in `paging`.

use super::*;
use crate::testutil::TempDir;
use crate::tool::tests_support::ctx_for;

/// The walk is stopped by the clock, and it says so in red: a search that
/// did not finish did not find "nothing", it found nothing *yet* (§5.9).
#[tokio::test]
async fn a_search_that_overruns_is_stopped_and_says_so() {
    let dir = TempDir::new();
    dir.write(
        "a.rs",
        "find me
",
    );
    let (ctx, _ws) = ctx_for(&dir);

    let tool = Grep {
        deadline: Duration::ZERO,
    };
    let err = tool
        .run(&ctx, json!({ "pattern": "find" }))
        .await
        .unwrap_err();

    assert!(matches!(err, ToolError::Failed { .. }), "got {err:?}");
    assert!(err.to_string().contains("timed out"), "{err}");
    // The model gets the way out as well, not only the fact.
    assert!(
        err.for_model().contains("subdirectory"),
        "{}",
        err.for_model()
    );
}
#[tokio::test]
async fn finds_lines_with_numbers() {
    let dir = TempDir::new();
    dir.write("src/a.rs", "let x = 1;\nlet clipboard = 2;\n");
    dir.write("src/b.ts", "const clipboard = 3;\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "clipboard" }))
        .await
        .unwrap();

    assert!(out.text.contains("src/a.rs:2: let clipboard = 2;"));
    assert!(out.text.contains("src/b.ts:1:"));
    assert_eq!(out.meta.items, Some(2));
}

#[tokio::test]
async fn path_pattern_narrows_search() {
    let dir = TempDir::new();
    dir.write("src/a.rs", "clipboard\n");
    dir.write("src/b.ts", "clipboard\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "clipboard", "glob": "**/*.rs" }))
        .await
        .unwrap();

    assert!(out.text.contains("src/a.rs"));
    assert!(!out.text.contains("b.ts"));
}

#[tokio::test]
async fn files_mode_returns_only_paths() {
    let dir = TempDir::new();
    dir.write("a.rs", "needle\nneedle\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "needle", "mode": "files" }))
        .await
        .unwrap();

    assert_eq!(out.text.trim(), "a.rs");
    assert_eq!(out.meta.items, Some(1));
}

#[tokio::test]
async fn ignore_applies_outside_git_repository_too() {
    let dir = TempDir::new();
    dir.write(
        ".gitignore",
        "target
node_modules
",
    );
    dir.write(
        "target/debug/build.rs",
        "needle
",
    );
    dir.write(
        "node_modules/pkg/index.js",
        "needle
",
    );
    dir.write(
        "src/a.rs",
        "needle
",
    );
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "needle" }))
        .await
        .unwrap();

    assert!(out.text.contains("src/a.rs"), "got: {}", out.text);
    assert!(!out.text.contains("target"), "got: {}", out.text);
    assert!(!out.text.contains("node_modules"), "got: {}", out.text);
}

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

#[tokio::test]
async fn context_lines_surround_a_match_and_are_marked_apart() {
    let dir = TempDir::new();
    dir.write("a.rs", "one\ntwo\nfound\nfour\nfive\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "found", "context": 1 }))
        .await
        .unwrap();

    // ripgrep's marks: `:` on the match, `-` on the lines around it.
    assert_eq!(out.text, "a.rs-2- two\na.rs:3: found\na.rs-4- four\n");
    assert_eq!(out.meta.items, Some(1), "context lines are not matches");
}

#[tokio::test]
async fn before_and_after_are_separate_and_override_context() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(9));
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(
            &ctx,
            json!({ "pattern": "^line 5$", "context": 3, "before": 2, "after": 0 }),
        )
        .await
        .unwrap();

    assert_eq!(out.text, "a.rs-3- line 3\na.rs-4- line 4\na.rs:5: line 5\n");
}

#[tokio::test]
async fn overlapping_windows_merge_and_distant_ones_are_split() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(12));
    let (ctx, _ws) = ctx_for(&dir);

    // Matches on 2, 4 and 10: the first two windows share line 3.
    let out = Grep::new()
        .run(
            &ctx,
            json!({ "pattern": "^line (2|4|10)$", "context": 1 }),
        )
        .await
        .unwrap();

    assert_eq!(
        out.text,
        "a.rs-1- line 1\na.rs:2: line 2\na.rs-3- line 3\na.rs:4: line 4\na.rs-5- line 5\n\
         --\n\
         a.rs-9- line 9\na.rs:10: line 10\na.rs-11- line 11\n"
    );
    assert_eq!(out.meta.items, Some(3));
}

#[tokio::test]
async fn groups_in_different_files_are_split_too() {
    let dir = TempDir::new();
    dir.write("a.rs", "x\nfound\n");
    dir.write("b.rs", "found\ny\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "found", "context": 1 }))
        .await
        .unwrap();

    assert_eq!(out.text, "a.rs-1- x\na.rs:2: found\n--\nb.rs:1: found\nb.rs-2- y\n");
}

#[tokio::test]
async fn wide_context_is_cut_to_a_budget_and_counted_in_matches() {
    let dir = TempDir::new();
    let wide = "w".repeat(300);
    let text: String = (0..2_000)
        .map(|i| if i % 10 == 0 { "hit\n".to_string() } else { format!("{wide}\n") })
        .collect();
    dir.write("a.rs", &text);
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "^hit$", "context": 9 }))
        .await
        .unwrap();

    // Two hundred matches with nine lines of context each way would be
    // well over a megabyte.
    assert!(out.text.len() < 110_000, "{} bytes", out.text.len());
    assert!(out.meta.truncated);
    // The footer counts matches, not the lines around them.
    let footer = out.text.lines().last().unwrap();
    assert!(footer.contains("of 200 matches"), "{footer}");
}

#[tokio::test]
async fn absence_of_matches_is_stated() {
    let dir = TempDir::new();
    dir.write("a.rs", "nothing\n");
    let (ctx, _ws) = ctx_for(&dir);

    let out = Grep::new()
        .run(&ctx, json!({ "pattern": "^none$" }))
        .await
        .unwrap();
    assert!(out.text.contains("no matches found"));
}

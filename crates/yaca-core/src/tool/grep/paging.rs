//! Pages of results and multiline matching, through the tool's own input.

use serde_json::json;

use super::Grep;
use crate::testutil::TempDir;
use crate::tool::tests_support::ctx_for;
use crate::tool::Tool;

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("hit {i}\n")).collect()
}

async fn search(dir: &TempDir, input: serde_json::Value) -> crate::tool::ToolOutput {
    let (ctx, _ws) = ctx_for(dir);
    Grep::new().run(&ctx, input).await.unwrap()
}

#[tokio::test]
async fn a_page_says_where_the_next_one_starts() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(10));

    let out = search(&dir, json!({ "pattern": "hit", "head_limit": 4 })).await;

    assert!(out.text.contains("a.rs:4: hit 4"), "{}", out.text);
    assert!(!out.text.contains("hit 5"), "{}", out.text);
    assert!(
        out.text.contains("showing 1–4 of 10 matches in 1 files; continue: offset=4"),
        "{}",
        out.text
    );
    assert!(out.meta.truncated);
    assert_eq!(out.meta.items, Some(10));
}

#[tokio::test]
async fn the_offset_from_the_footer_continues_without_a_gap_or_a_repeat() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(10));

    let out = search(&dir, json!({ "pattern": "hit", "head_limit": 4, "offset": 4 })).await;

    assert!(out.text.starts_with("a.rs:5: hit 5\n"), "{}", out.text);
    assert!(out.text.contains("a.rs:8: hit 8"), "{}", out.text);
    assert!(!out.text.contains("hit 9"), "{}", out.text);
    assert!(out.text.contains("showing 5–8 of 10"), "{}", out.text);
    assert!(out.text.contains("offset=8"), "{}", out.text);
}

#[tokio::test]
async fn the_last_page_has_no_continuation() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(10));

    let out = search(&dir, json!({ "pattern": "hit", "head_limit": 4, "offset": 8 })).await;

    assert!(out.text.contains("hit 10"), "{}", out.text);
    assert!(out.text.contains("showing 9–10 of 10"), "{}", out.text);
    assert!(!out.text.contains("continue"), "{}", out.text);
    assert!(!out.meta.truncated);
}

#[tokio::test]
async fn pages_run_across_files_in_path_order() {
    let dir = TempDir::new();
    dir.write("a.rs", "hit a1\nhit a2\nhit a3\n");
    dir.write("b.rs", "hit b1\nhit b2\n");

    let first = search(&dir, json!({ "pattern": "hit", "head_limit": 2 })).await;
    assert!(first.text.contains("a.rs:1:") && first.text.contains("a.rs:2:"));
    assert!(!first.text.contains("a3"));

    let second = search(&dir, json!({ "pattern": "hit", "head_limit": 2, "offset": 2 })).await;
    assert!(second.text.contains("a.rs:3: hit a3"), "{}", second.text);
    assert!(second.text.contains("b.rs:1: hit b1"), "{}", second.text);
    assert!(!second.text.contains("b2"), "{}", second.text);
    assert!(second.text.contains("in 2 files"), "{}", second.text);
}

#[tokio::test]
async fn an_offset_past_the_end_says_how_many_there_are() {
    let dir = TempDir::new();
    dir.write("a.rs", &numbered(3));

    let out = search(&dir, json!({ "pattern": "hit", "offset": 50 })).await;

    assert!(out.text.contains("is out of range: 3 matches total"), "{}", out.text);
    assert!(!out.text.contains("a.rs:"), "{}", out.text);
}

#[tokio::test]
async fn context_pages_do_not_repeat_a_match_as_context() {
    let dir = TempDir::new();
    dir.write("a.rs", "hit 1\nhit 2\nhit 3\nhit 4\n");

    let out = search(
        &dir,
        json!({ "pattern": "hit", "context": 1, "head_limit": 2, "offset": 2 }),
    )
    .await;

    // Match 2 sits in the context of match 3 and keeps its mark.
    assert!(out.text.starts_with("a.rs:2: hit 2\na.rs:3: hit 3\na.rs:4: hit 4\n"), "{}", out.text);
}

#[tokio::test]
async fn files_mode_pages_too() {
    let dir = TempDir::new();
    for name in ["a", "b", "c", "d"] {
        dir.write(&format!("{name}.rs"), "hit\n");
    }

    let first = search(&dir, json!({ "pattern": "hit", "mode": "files", "head_limit": 3 })).await;
    assert_eq!(first.text.lines().take(3).collect::<Vec<_>>(), ["a.rs", "b.rs", "c.rs"]);
    assert!(first.text.contains("of 4 files; continue: offset=3"), "{}", first.text);

    let rest = search(
        &dir,
        json!({ "pattern": "hit", "mode": "files", "head_limit": 3, "offset": 3 }),
    )
    .await;
    assert!(rest.text.starts_with("d.rs\n"), "{}", rest.text);
    assert!(!rest.text.contains("continue"), "{}", rest.text);
}

#[tokio::test]
async fn a_pattern_cannot_span_lines_unless_asked() {
    let dir = TempDir::new();
    dir.write("a.rs", "fn main() {\n    run();\n}\n");

    let out = search(&dir, json!({ "pattern": r"main\(\) \{\n\s+run" })).await;
    assert!(out.text.contains("no matches found"), "{}", out.text);
}

#[tokio::test]
async fn multiline_matches_across_lines_and_shows_every_line_of_the_match() {
    let dir = TempDir::new();
    dir.write("a.rs", "fn main() {\n    run();\n}\n\nfn other() {}\n");

    let out = search(
        &dir,
        json!({ "pattern": r"fn main\(\) \{.*?\n\}", "multiline": true }),
    )
    .await;

    assert_eq!(out.text, "a.rs:1: fn main() {\na.rs:2:     run();\na.rs:3: }\n");
    assert_eq!(out.meta.items, Some(1), "one match, however many lines");
}

#[tokio::test]
async fn multiline_context_surrounds_the_whole_match() {
    let dir = TempDir::new();
    dir.write("a.rs", "top\nstart\nmiddle\nend\nbottom\n");

    let out = search(
        &dir,
        json!({ "pattern": "start.*end", "multiline": true, "context": 1 }),
    )
    .await;

    assert_eq!(
        out.text,
        "a.rs-1- top\na.rs:2: start\na.rs:3: middle\na.rs:4: end\na.rs-5- bottom\n"
    );
}

#[tokio::test]
async fn multiline_keeps_line_anchors() {
    let dir = TempDir::new();
    dir.write("a.rs", "alpha\nbeta\nalpha beta\n");

    let out = search(&dir, json!({ "pattern": "^beta$", "multiline": true })).await;

    assert_eq!(out.text, "a.rs:2: beta\n");
}

#[tokio::test]
async fn a_match_ending_on_a_newline_does_not_claim_the_next_line() {
    let dir = TempDir::new();
    dir.write("a.rs", "one\ntwo\nthree\n");

    let out = search(&dir, json!({ "pattern": r"one\n", "multiline": true })).await;

    assert_eq!(out.text, "a.rs:1: one\n");
}

#[tokio::test]
async fn multiline_matches_are_paged_by_match() {
    let dir = TempDir::new();
    dir.write("a.rs", "<a>\n1\n</a>\n<a>\n2\n</a>\n<a>\n3\n</a>\n");

    let out = search(
        &dir,
        json!({ "pattern": r"<a>.*?</a>", "multiline": true, "head_limit": 1, "offset": 1 }),
    )
    .await;

    assert_eq!(out.text.lines().next(), Some("a.rs:4: <a>"), "{}", out.text);
    assert!(out.text.contains("a.rs:6: </a>"), "{}", out.text);
    assert!(!out.text.contains("a.rs:7:"), "{}", out.text);
    assert!(out.text.contains("showing 2–2 of 3 matches"), "{}", out.text);
    assert!(out.text.contains("offset=2"), "{}", out.text);
}

#[tokio::test]
async fn files_mode_honours_multiline() {
    let dir = TempDir::new();
    dir.write("a.rs", "start\nend\n");
    dir.write("b.rs", "start\n\nend\n");

    let out = search(
        &dir,
        json!({ "pattern": r"start\nend", "multiline": true, "mode": "files" }),
    )
    .await;

    assert_eq!(out.text.trim(), "a.rs");
}

#[tokio::test]
async fn context_that_fills_the_budget_before_a_match_says_so() {
    let dir = TempDir::new();
    let wide = "w".repeat(300);
    let mut text = String::new();
    for _ in 0..600 {
        text.push_str(&wide);
        text.push('\n');
    }
    text.push_str("hit\n");
    dir.write("a.rs", &text);

    let out = search(&dir, json!({ "pattern": "^hit$", "before": 600 })).await;

    assert!(out.text.contains("decrease context"), "{}", out.text);
    assert!(!out.text.contains("continue"), "{}", out.text);
}

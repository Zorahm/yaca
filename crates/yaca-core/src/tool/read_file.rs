//! Reading a file. The caption is written by the model (§5.4.1): the interface does not know why it was read.
//!
//! Reads through the set of uncommitted changes. Otherwise an agent that just edited
//! a file would re-read it, see the old content, and start arguing with itself.

use async_trait::async_trait;
use serde_json::{json, Value};

use std::path::{Path, PathBuf};

use super::{files, notebook, pdf};
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;

/// Cap for a single call. More only with explicit `offset`/`limit`:
/// otherwise a single `read_file` consumes the context window.
const DEFAULT_LIMIT: usize = 2_000;
/// Long lines are truncated: a single-line minified bundle can
/// blow out the entire window.
const MAX_LINE_CHARS: usize = 2_000;
/// How much of the file one call hands the model, in bytes, line numbers
/// included. The two limits above do not bound it: two thousand lines of a
/// wide CSV are two megabytes, more than a whole context window from one call,
/// and it stays in the window for every request after. Sized so that the
/// default window of ordinary code — about forty characters a line — still
/// arrives whole: roughly 25k tokens.
const MAX_OUTPUT_BYTES: usize = 100_000;

pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "Read"
    }

    fn description(&self) -> &'static str {
        "Read a project file. Returns numbered lines. \
         For large files, use offset and limit. \
         A PDF comes back as its text, page by page: one of up to 10 pages is read \
         whole, a longer one needs `pages` (at most 20 per call). A scanned page has \
         no text to return. A Jupyter notebook (.ipynb) comes back as its cells, each \
         with its number, id and outputs; offset and limit then count cells."
    }

    fn properties(&self) -> Value {
        json!({
            "path": {
                "type": "string",
                "description": "Path relative to the project root.",
            },
            "offset": {
                "type": "integer",
                "minimum": 1,
                "description": "Line number to start from, 1-indexed.",
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "description": "Number of lines to read.",
            },
            "pages": {
                "type": "string",
                "description": "PDF only: the pages to read, e.g. \"3\" or \"3-7\".",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Reading modifies nothing and asks for nothing.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;

        // An image read as text is two megabytes of noise in the window, and
        // "binary file" would leave the model with nowhere to go next.
        if ctx.ws.resolve_new(raw_path).is_ok_and(|abs| crate::picture::is_picture(&abs)) {
            return Err(ToolError::failed_with(
                format!("{raw_path} is an image, not text"),
                "This file is an image. Look at it with ReadImage.",
            ));
        }

        if let Ok(abs) = ctx.ws.resolve_new(raw_path) {
            if is_pdf(&abs) {
                return read_pdf(ctx, abs, &input).await;
            }
        }

        // Reading moves to a blocking executor: a large file on a network
        // drive would otherwise stall the entire session — along with the event
        // channel and cancellation, — and in a §5.4 batch would block adjacent
        // reads that should run together.
        let ws = ctx.ws.clone();
        let raw = raw_path.to_string();
        let target = tokio::task::spawn_blocking(move || files::look_in(&ws, &raw))
            .await
            .map_err(|_| ToolError::failed("reading file did not complete"))??;

        if target.abs.is_dir() {
            return Err(ToolError::failed(format!("{raw_path} is a directory, not a file")));
        }
        let Some(text) = target.current else {
            return Err(ToolError::failed(format!("not found: {raw_path}")));
        };

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(DEFAULT_LIMIT as u64) as usize;

        if notebook::is_notebook(&target.abs) {
            let shown = notebook::render(&text, &target.rel, offset, limit, MAX_OUTPUT_BYTES)?;
            remember(ctx, &target.abs);
            let mut out = shown.text;
            let truncated = shown.end < shown.total;
            if truncated || offset > 1 {
                out.push_str(&format!(
                    "[showing cells {offset}–{} of {}]\n",
                    shown.end, shown.total
                ));
            }
            return Ok(ToolOutput::new(out)
                .items(shown.total as u32)
                .truncated(truncated));
        }

        let all: Vec<&str> = text.lines().collect();
        let total = all.len();
        let start = offset - 1;

        if start >= total && total > 0 {
            return Err(ToolError::failed(format!(
                "file has {total} lines, requested line {offset}"
            )));
        }

        let window = (start + limit).min(total);
        let mut out = String::new();
        // The last line shown, which the budget may put before the window's
        // end. The first line always fits: a clipped line is a few kilobytes.
        let mut end = start;
        for (i, line) in all[start..window].iter().enumerate() {
            ctx.check_cancelled()?;
            let n = start + i + 1;
            let shown: String = if line.chars().count() > MAX_LINE_CHARS {
                line.chars().take(MAX_LINE_CHARS).collect::<String>() + " …[line truncated]"
            } else {
                (*line).to_string()
            };
            let row = format!("{n:>6}\t{shown}\n");
            if out.len() + row.len() > MAX_OUTPUT_BYTES {
                break;
            }
            out.push_str(&row);
            end = n;
        }

        remember(ctx, &target.abs);

        let truncated = end < total;
        if truncated {
            out.push_str(&format!(
                "\n[showing lines {}–{} of {total}]\n",
                start + 1,
                end
            ));
        }
        if total == 0 {
            out.push_str("[file is empty]\n");
        }

        Ok(ToolOutput::new(out)
            .items(total as u32)
            .truncated(truncated))
    }
}

/// File read — registry remembered the fingerprint (§8, read-before-edit).
/// For a file that only exists in the change set, there is no fingerprint:
/// there will be nothing to compare it against, and that is correct.
fn remember(ctx: &ToolCtx, abs: &Path) {
    if ctx.ws.fingerprint(abs).is_some() {
        ctx.ws.mark_read(abs);
    } else {
        ctx.ws.mark_authored(abs);
    }
}

fn is_pdf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// A PDF's text, by page. Not marked as read: there is no text in the file
/// itself an edit could anchor on.
async fn read_pdf(ctx: &ToolCtx, abs: PathBuf, input: &Value) -> Result<ToolOutput, ToolError> {
    let shown = ctx.ws.display(&abs);
    let pages = input["pages"].as_str().map(pdf::parse_pages).transpose()?;
    if abs.is_dir() {
        return Err(ToolError::failed(format!("{shown} is a directory, not a file")));
    }
    let name = shown.clone();
    let read = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&abs).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ToolError::failed(format!("not found: {name}")),
            _ => ToolError::failed(e.to_string()),
        })?;
        pdf::read(&bytes, &name, pages)
    })
    .await
    .map_err(|_| ToolError::failed("reading the PDF did not complete"))??;

    let mut text = read.text;
    let partial = read.shown < read.total;
    if partial {
        text.push_str(&format!("[{} of {} pages shown]\n", read.shown, read.total));
    }
    if !read.empty.is_empty() && read.empty.len() as u32 == read.shown {
        text.push_str(
            "[no page here has a text layer: this looks like a scanned document, \
             and its text cannot be read]\n",
        );
    }
    if text.len() > MAX_OUTPUT_BYTES {
        let mut end = MAX_OUTPUT_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[text cut: ask for fewer pages]\n");
    }
    Ok(ToolOutput::new(text).items(read.total).truncated(partial))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn numbers_lines_and_marks_read() {
        let dir = TempDir::new();
        dir.write("a.rs", "first\nsecond\nthird\n");
        let (ctx, ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "a.rs" })).await.unwrap();

        assert!(out.text.contains("     1\tfirst"));
        assert!(out.text.contains("     3\tthird"));
        assert_eq!(out.meta.items, Some(3));
        assert!(!out.meta.truncated);
        assert!(ws.was_read(&ws.resolve_existing("a.rs").unwrap()));
    }

    #[tokio::test]
    async fn window_by_offset_and_limit() {
        let dir = TempDir::new();
        dir.write("a.rs", "1\n2\n3\n4\n5\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile
            .run(&ctx, json!({ "path": "a.rs", "offset": 2, "limit": 2 }))
            .await
            .unwrap();

        assert!(out.text.contains("     2\t2"));
        assert!(out.text.contains("     3\t3"));
        assert!(!out.text.contains("\t4"));
        assert!(out.meta.truncated);
    }

    #[tokio::test]
    async fn a_wide_file_is_cut_to_a_budget_and_says_where() {
        let dir = TempDir::new();
        let line = "x".repeat(1_000);
        dir.write("wide.csv", &format!("{line}\n").repeat(2_000));
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "wide.csv" })).await.unwrap();

        // Two thousand lines of a thousand characters is two megabytes — more
        // than a whole context window, from one call.
        assert!(out.text.len() <= MAX_OUTPUT_BYTES + 100, "{} bytes", out.text.len());
        assert!(out.meta.truncated);
        assert!(!out.text.contains("  2000\t"));
        // The footer is how the model finds out where to go on from.
        assert!(out.text.contains("of 2000]"), "{:?}", out.text.lines().last());
    }

    #[tokio::test]
    async fn an_ordinary_file_of_the_default_window_still_fits_whole() {
        let dir = TempDir::new();
        let line = format!("    let value = compute(input, &config);{}", " ");
        dir.write("a.rs", &format!("{line}\n").repeat(2_000));
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "a.rs" })).await.unwrap();

        assert!(!out.meta.truncated);
        assert!(out.text.contains("  2000\t"));
    }

    #[tokio::test]
    async fn leaving_project_is_a_rejection_not_a_failure() {
        let dir = TempDir::new();
        dir.write("a.rs", "x");
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "../outside.rs" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn binary_file_is_rejected_clearly() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("bin"), [0u8, 1, 2, 3]).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "bin" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("binary"));
    }

    #[tokio::test]
    async fn an_image_is_sent_to_the_tool_that_can_show_it() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), crate::picture::tests::png(8, 8)).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "shot.png" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains("ReadImage"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn missing_file_is_clear_refusal() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "no-such.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn a_pdf_is_read_as_text_by_page() {
        let dir = TempDir::new();
        std::fs::write(
            dir.path().join("spec.pdf"),
            crate::tool::pdf::tests::pdf(&["The first page says hello", "The second one says bye"]),
        )
        .unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "spec.pdf" })).await.unwrap();
        assert!(out.text.contains("--- page 1 of 2 ---"), "{}", out.text);
        assert!(out.text.contains("says bye"), "{}", out.text);
        assert_eq!(out.meta.items, Some(2));

        let one = ReadFile
            .run(&ctx, json!({ "path": "spec.pdf", "pages": "2" }))
            .await
            .unwrap();
        assert!(!one.text.contains("hello"));
        assert!(one.text.contains("[1 of 2 pages shown]"), "{}", one.text);
        assert!(one.meta.truncated);
    }

    #[tokio::test]
    async fn a_notebook_is_read_as_cells_and_can_then_be_edited() {
        let dir = TempDir::new();
        let path = dir.write(
            "a.ipynb",
            r#"{"cells":[{"cell_type":"code","id":"c1","metadata":{},"outputs":[],"source":["x = 1"]}],"metadata":{},"nbformat":4,"nbformat_minor":5}"#,
        );
        let (ctx, ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "a.ipynb" })).await.unwrap();
        assert!(out.text.starts_with("[cell 1 · code · id c1]\nx = 1\n"), "{}", out.text);
        assert_eq!(out.meta.items, Some(1));
        assert!(ws.was_read(&path));
    }
}

//! Renaming a symbol through the language server.
//!
//! A rename by text is a guess: `parse` the method and `parse` the local
//! variable in the next function share a name and nothing else, and a
//! replace-all across files cannot tell them apart. The server can. It answers
//! `textDocument/rename` with every edit the rename takes, and this tool
//! applies them.
//!
//! All or nothing, like `Edit`. Every file is read and every edit is placed
//! before the first byte is written; an edit that lands outside its file, two
//! that overlap, a file outside the project or one a rollback could not bring
//! back stop the whole rename with nothing changed.
//!
//! Renaming files is not done here. A server renaming a module may also want
//! to move the file that holds it; the client tells it that it cannot, and a
//! server that asks anyway is refused, with the reason, rather than half obeyed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files::{self, read_text};
use super::{Edited, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::{ChangeKind, FileFact};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::lsp::format::locate;
use crate::lsp::LspError;
use crate::permission::{invariant, unsaved, PermissionNeed};
use crate::uri::path_of;
use crate::workspace::Workspace;

/// A rename across a large project is a search of it: longer than a lookup.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Rename;

#[async_trait]
impl Tool for Rename {
    fn name(&self) -> &'static str {
        "Rename"
    }

    fn description(&self) -> &'static str {
        "Rename a symbol — a variable, function, method, type, field, module — \
         everywhere it is used, through the language server. The server knows which \
         uses are this symbol and which only share its name, which a text replace \
         does not. Prefer it to Edit for any rename that reaches past one place. \
         Name the position as for Lsp: `path` + `line` (1-based) + `symbol`, the \
         text on that line to stand on; do not count columns. Every file the server \
         names is changed in one go, or none is. The result lists the files; read \
         one before editing it further."
    }

    fn properties(&self) -> Value {
        json!({
            "path": { "type": "string", "description": "File with a use or the definition of the symbol." },
            "line": { "type": "integer", "minimum": 1, "description": "1-based line." },
            "symbol": {
                "type": "string",
                "description": "Text on that line to stand at the start of, usually the \
                                current name. If it occurs more than once on the line, use \
                                a longer fragment or `column`.",
            },
            "column": {
                "type": "integer",
                "minimum": 1,
                "description": "1-based column, instead of `symbol`.",
            },
            "new_name": { "type": "string", "description": "The new name." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path", "line", "new_name"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    /// Asked as an edit of the named file. Which other files the rename
    /// reaches only the server knows, and it is asked after the gate; the
    /// question says so rather than pretending to one file.
    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let new_name = input["new_name"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        let what = input["symbol"].as_str().unwrap_or("the symbol");
        Some(
            files::edit_need(ws, self.name(), raw, abs)
                .note(format!("Renames `{what}` to `{new_name}` in every file that uses it")),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let lsp = ctx
            .lsp
            .as_ref()
            .filter(|lsp| !lsp.names().is_empty())
            .ok_or_else(|| ToolError::failed("no language servers in this session"))?;
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let line = input["line"]
            .as_u64()
            .ok_or_else(|| ToolError::failed("line is required"))?;
        let new_name = input["new_name"]
            .as_str()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| ToolError::failed("new_name is required"))?;

        let abs = ctx.ws.resolve_existing(raw)?;
        let text = read_text(&abs)?.ok_or_else(|| ToolError::failed(format!("not found: {raw}")))?;
        let server = lsp.server_for(&abs).ok_or_else(|| {
            ToolError::failed_with(
                format!("no language server for {raw}"),
                "No language server handles this file's extension, so there is nobody to \
                 ask which uses are the same symbol. Rename with Edit, after finding the \
                 uses with Grep.",
            )
        })?;
        let client = lsp.client(&server, &ctx.cancel).await.map_err(mapped)?;
        let (uri, _) = client.sync(&abs, &text).await.map_err(mapped)?;

        let (line, character) = locate(
            &text,
            line as usize,
            input["symbol"].as_str(),
            input["column"].as_u64().map(|c| c as usize),
        )
        .map_err(ToolError::failed)?;
        let at = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
        });

        // Not every server answers `prepareRename`; one that does and says
        // `null` is saying there is nothing renameable here, which is worth
        // hearing before an answer of "no edits".
        match client
            .request("textDocument/prepareRename", at.clone(), REQUEST_TIMEOUT, &ctx.cancel)
            .await
        {
            Ok(Value::Null) => {
                return Err(ToolError::failed(
                    "the language server says there is nothing to rename at this position: \
                     stand on the name itself",
                ))
            }
            Err(LspError::Cancelled) => return Err(ToolError::Cancelled),
            // A server that cannot rename says so to `rename` as well.
            Ok(_) | Err(_) => {}
        }

        let mut params = at;
        params["newName"] = json!(new_name);
        let answer = client
            .request("textDocument/rename", params, REQUEST_TIMEOUT, &ctx.cancel)
            .await
            .map_err(mapped)?;
        let per_file = edits_of(&answer)?;
        if per_file.is_empty() {
            return Err(ToolError::failed("the language server returned no edits for this rename"));
        }

        // Everything is placed before anything is written.
        let mut planned = Vec::new();
        for (path, edits) in per_file {
            planned.push(plan(ctx, &abs, &path, &edits)?);
        }
        ctx.check_cancelled()?;

        let mut out = ToolOutput::new(String::new());
        let mut lines = Vec::new();
        let mut written = Vec::new();
        for file in planned {
            if let Err(e) = std::fs::write(&file.abs, &file.after) {
                let done = if written.is_empty() {
                    "nothing was changed".to_string()
                } else {
                    format!("already changed: {}", written.join(", "))
                };
                return Err(ToolError::failed(format!(
                    "could not write {}: {e}; {done}",
                    file.rel
                )));
            }
            // A file the agent had read stays editable: it knows what changed.
            // One it never read stays unread — the rename is no reading of it.
            if ctx.ws.was_read(&file.abs) {
                ctx.ws.mark_read(&file.abs);
            }
            let _ = client.sync(&file.server_path, &file.after).await;

            let (fact, diff) = FileFact::of(
                file.rel.clone(),
                ChangeKind::Modified,
                Some(&file.before),
                Some(&file.after),
            );
            lines.push(format!(
                "  {} (+{} −{}, {} {})",
                file.rel,
                fact.added,
                fact.removed,
                file.count,
                if file.count == 1 { "edit" } else { "edits" }
            ));
            written.push(file.rel.clone());
            out = out.edited(Edited {
                change: super::FileChange { fact, diff },
                abs: file.abs,
                before: file.before,
                after: file.after,
            });
        }

        let what = input["symbol"].as_str().unwrap_or("the symbol");
        let files = lines.len();
        let mut text = format!(
            "Renamed `{what}` to `{new_name}` in {files} {}:\n",
            if files == 1 { "file" } else { "files" }
        );
        text.push_str(&lines.join("\n"));
        text.push('\n');
        out.text = text;
        out.meta.lines = out.text.lines().count() as u32;
        Ok(out.items(files as u32))
    }
}

/// One text edit: a range in the server's coordinates and what goes there.
#[derive(Debug, Clone, PartialEq)]
struct TextEdit {
    start: (u32, u32),
    end: (u32, u32),
    text: String,
}

/// The edits of a `WorkspaceEdit`, per file, in the order the server named
/// the files. Either form the protocol allows: `documentChanges`, or the
/// older `changes` map.
fn edits_of(answer: &Value) -> Result<Vec<(PathBuf, Vec<TextEdit>)>, ToolError> {
    let mut out: Vec<(PathBuf, Vec<TextEdit>)> = Vec::new();
    let mut add = |uri: &str, edits: &Value| -> Result<(), ToolError> {
        let path = path_of(uri).ok_or_else(|| {
            ToolError::failed(format!("the rename touches {uri}, which is not a local file"))
        })?;
        let edits = edits
            .as_array()
            .ok_or_else(|| ToolError::failed("the language server sent edits that are not a list"))?
            .iter()
            .map(text_edit)
            .collect::<Result<Vec<_>, _>>()?;
        match out.iter_mut().find(|(p, _)| *p == path) {
            Some((_, list)) => list.extend(edits),
            None => out.push((path, edits)),
        }
        Ok(())
    };

    if let Some(changes) = answer["documentChanges"].as_array() {
        for change in changes {
            if let Some(kind) = change["kind"].as_str() {
                return Err(ToolError::failed_with(
                    format!("the rename would also {kind} a file, which Rename does not do"),
                    format!(
                        "The language server wants this rename to {kind} a file as well \
                         ({}). Rename does not move or create files. Rename the symbol \
                         with Edit, and the file with Move.",
                        change["oldUri"].as_str().or(change["uri"].as_str()).unwrap_or("?")
                    ),
                ));
            }
            let uri = change["textDocument"]["uri"]
                .as_str()
                .ok_or_else(|| ToolError::failed("the language server sent an edit without a document"))?;
            add(uri, &change["edits"])?;
        }
    } else if let Some(changes) = answer["changes"].as_object() {
        for (uri, edits) in changes {
            add(uri, edits)?;
        }
    }
    out.retain(|(_, edits)| !edits.is_empty());
    Ok(out)
}

fn text_edit(value: &Value) -> Result<TextEdit, ToolError> {
    let bad = || ToolError::failed("the language server sent a malformed edit");
    let pos = |p: &Value| -> Option<(u32, u32)> {
        Some((p["line"].as_u64()? as u32, p["character"].as_u64()? as u32))
    };
    Ok(TextEdit {
        start: pos(&value["range"]["start"]).ok_or_else(bad)?,
        end: pos(&value["range"]["end"]).ok_or_else(bad)?,
        text: value["newText"].as_str().ok_or_else(bad)?.to_string(),
    })
}

/// A file the rename will write, with everything checked.
struct Planned {
    abs: PathBuf,
    /// The path as the server named it: what it is told the new text under.
    server_path: PathBuf,
    rel: String,
    before: String,
    after: String,
    count: usize,
}

/// Checks one file of the rename and works out its new text.
fn plan(ctx: &ToolCtx, asked: &Path, path: &Path, edits: &[TextEdit]) -> Result<Planned, ToolError> {
    let ws = &ctx.ws;
    // A server names files its own way; the workspace's form is what the
    // boundary check and the read registry know them by.
    let abs = ws.resolve_new(&ws.in_form(path).to_string_lossy())?;
    let rel = ws.display(&abs);
    if invariant::of_path(ws.root_for(&abs), &abs).is_some() {
        return Err(ToolError::rejected(format!(
            "the rename would change {rel}, which is asked about separately in every mode; \
             nothing was changed — edit it with Edit"
        )));
    }
    // The file the human was asked about carries its own warning. Any other
    // that a rollback would not restore is one they were never asked about.
    let same = ws.in_form(asked) == abs;
    if !same && unsaved::outside_checkpoint(ws, [abs.as_path()]).is_some() {
        return Err(ToolError::rejected(format!(
            "the rename would change {rel}, outside what a rollback restores; \
             nothing was changed — rename there with Edit"
        )));
    }
    let before = read_text(&abs)?
        .ok_or_else(|| ToolError::failed(format!("the rename names {rel}, which does not exist")))?;
    let after = apply(&before, edits).map_err(|why| {
        ToolError::failed(format!("the rename's edits do not fit {rel} ({why}); nothing was changed"))
    })?;
    Ok(Planned {
        abs,
        server_path: path.to_path_buf(),
        rel,
        before,
        after,
        count: edits.len(),
    })
}

/// Applies edits given in LSP coordinates — zero-based lines, columns in
/// UTF-16 units — to `text`.
fn apply(text: &str, edits: &[TextEdit]) -> Result<String, String> {
    let starts = line_starts(text);
    let mut placed: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|e| {
            let from = offset(text, &starts, e.start)?;
            let to = offset(text, &starts, e.end)?;
            if to < from {
                return Err(format!("range ends before it starts at line {}", e.start.0 + 1));
            }
            Ok((from, to, e.text.as_str()))
        })
        .collect::<Result<_, String>>()?;
    placed.sort_by_key(|&(from, to, _)| (from, to));
    for pair in placed.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err("two edits overlap".into());
        }
    }
    let mut out = text.to_string();
    for (from, to, new) in placed.into_iter().rev() {
        out.replace_range(from..to, new);
    }
    Ok(out)
}

/// Byte offsets where each line starts. A line ends at `\n`, `\r\n` or `\r`.
fn line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => starts.push(i + 1),
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                starts.push(i + 2);
                i += 1;
            }
            b'\r' => starts.push(i + 1),
            _ => {}
        }
        i += 1;
    }
    starts
}

/// The byte offset of a position. A column past the end of its line stands
/// at the line's end, as the protocol says it should.
fn offset(text: &str, starts: &[usize], (line, character): (u32, u32)) -> Result<usize, String> {
    let line = line as usize;
    let Some(&start) = starts.get(line) else {
        return Err(format!("line {} is past the end of the file", line + 1));
    };
    let end = starts
        .get(line + 1)
        .map_or(text.len(), |&next| next)
        .min(text.len());
    let content = text[start..end].trim_end_matches(['\n', '\r']);
    let mut units = 0u32;
    for (i, c) in content.char_indices() {
        if units >= character {
            return Ok(start + i);
        }
        units += c.len_utf16() as u32;
    }
    Ok(start + content.len())
}

fn mapped(error: LspError) -> ToolError {
    match error {
        LspError::Cancelled => ToolError::Cancelled,
        LspError::Rpc { message, .. } => {
            ToolError::failed(format!("the language server refused the rename: {message}"))
        }
        other => ToolError::failed(format!("language server: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: (u32, u32), end: (u32, u32), text: &str) -> TextEdit {
        TextEdit { start, end, text: text.into() }
    }

    #[test]
    fn edits_land_by_line_and_utf16_column() {
        // `é` is one UTF-16 unit, `𝔁` is two: columns count units, not bytes.
        let text = "let é = parse(x);\nlet 𝔁 = parse(y);\n";
        let out = apply(
            text,
            &[edit((0, 8), (0, 13), "parse_line"), edit((1, 9), (1, 14), "parse_line")],
        )
        .unwrap();
        assert_eq!(out, "let é = parse_line(x);\nlet 𝔁 = parse_line(y);\n");
    }

    #[test]
    fn crlf_files_keep_their_line_endings() {
        let text = "fn a() {}\r\nfn b() { a() }\r\n";
        let out = apply(text, &[edit((0, 3), (0, 4), "z"), edit((1, 9), (1, 10), "z")]).unwrap();
        assert_eq!(out, "fn z() {}\r\nfn b() { z() }\r\n");
    }

    #[test]
    fn overlapping_or_misplaced_edits_change_nothing() {
        let text = "abc\n";
        assert!(apply(text, &[edit((0, 0), (0, 2), "x"), edit((0, 1), (0, 3), "y")]).is_err());
        assert!(apply(text, &[edit((5, 0), (5, 1), "x")]).is_err());
        // A column past the end clamps to the line's end.
        assert_eq!(apply(text, &[edit((0, 3), (0, 99), "!")]).unwrap(), "abc!\n");
    }

    #[test]
    fn both_forms_of_workspace_edit_are_read() {
        let range = json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } });
        let older = json!({ "changes": { "file:///p/a.rs": [{ "range": range, "newText": "b" }] } });
        let edits = edits_of(&older).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].1[0].text, "b");

        let newer = json!({ "documentChanges": [
            { "textDocument": { "uri": "file:///p/a.rs", "version": 1 }, "edits": [{ "range": range, "newText": "b" }] },
            { "textDocument": { "uri": "file:///p/a.rs", "version": 1 }, "edits": [{ "range": range, "newText": "c" }] },
        ] });
        let edits = edits_of(&newer).unwrap();
        assert_eq!(edits.len(), 1, "one file named twice is one file");
        assert_eq!(edits[0].1.len(), 2);
    }

    #[test]
    fn a_file_operation_refuses_the_whole_rename() {
        let answer = json!({ "documentChanges": [
            { "kind": "rename", "oldUri": "file:///p/a.rs", "newUri": "file:///p/b.rs" },
        ] });
        let err = edits_of(&answer).unwrap_err();
        assert!(err.to_string().contains("rename a file"), "{err}");
        assert!(err.for_model().contains("Move"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn without_a_language_server_it_says_so() {
        let dir = crate::testutil::TempDir::new();
        dir.write("a.rs", "fn a() {}\n");
        let (ctx, _ws) = crate::tool::tests_support::ctx_for(&dir);
        let err = Rename
            .run(&ctx, json!({ "path": "a.rs", "line": 1, "symbol": "a", "new_name": "b" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no language servers"), "{err}");
    }

    #[tokio::test]
    async fn a_planned_file_outside_the_project_is_refused() {
        let dir = crate::testutil::TempDir::new();
        let inside = dir.write("a.rs", "fn a() {}\n");
        let (ctx, _ws) = crate::tool::tests_support::ctx_for(&dir);
        let outside = dir.path().parent().unwrap().join("elsewhere.rs");
        let err = plan(&ctx, &inside, &outside, &[edit((0, 0), (0, 1), "x")])
            .err()
            .expect("outside the project");
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_planned_file_gets_its_new_text_and_nothing_is_written_yet() {
        let dir = crate::testutil::TempDir::new();
        let file = dir.write("a.rs", "fn a() {}\nfn b() { a() }\n");
        let (ctx, _ws) = crate::tool::tests_support::ctx_for(&dir);
        let planned = plan(
            &ctx,
            &file,
            &file,
            &[edit((0, 3), (0, 4), "run"), edit((1, 9), (1, 10), "run")],
        )
        .unwrap();
        assert_eq!(planned.after, "fn run() {}\nfn b() { run() }\n");
        assert_eq!(planned.count, 2);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fn a() {}\nfn b() { a() }\n");
    }
}

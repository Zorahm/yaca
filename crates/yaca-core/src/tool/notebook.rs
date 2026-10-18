//! Jupyter notebooks: read as cells, edited as cells.
//!
//! An `.ipynb` is JSON, and both tools that already handle files would handle
//! it badly. `Read` would show the JSON — every line of source a quoted string
//! with a `\n` inside, outputs as base64 — and `Edit` on it means the model
//! writing escaped JSON by hand, where one missing comma leaves a notebook
//! Jupyter will not open. So `Read` shows the cells as a person sees them, and
//! `NotebookEdit` changes one cell and writes the JSON back itself.
//!
//! What is written back keeps the file's own shape: its indentation, its
//! trailing newline, the metadata of every cell it did not touch.

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::ChangeKind;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::PermissionNeed;
use crate::workspace::Workspace;

/// How much of one cell's outputs the model is shown. A training loop that
/// printed ten thousand lines is a cell worth knowing about, not reading.
const OUTPUT_CHARS: usize = 4_000;

/// The cells of a notebook rendered for the model.
pub(crate) struct Rendered {
    pub text: String,
    pub total: usize,
    /// The last cell shown, counted from one; below `total` when cut.
    pub end: usize,
}

/// Whether `path` names a notebook.
pub(crate) fn is_notebook(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ipynb"))
}

fn parse(text: &str, shown: &str) -> Result<Value, ToolError> {
    let notebook: Value = serde_json::from_str(text).map_err(|e| {
        ToolError::failed(format!("{shown} is not a valid notebook: {e}"))
    })?;
    if !notebook["cells"].is_array() {
        return Err(ToolError::failed(format!(
            "{shown} is not a valid notebook: no cells array"
        )));
    }
    Ok(notebook)
}

/// Renders cells `offset..offset + limit` (counted from one) within `budget` bytes.
pub(crate) fn render(
    text: &str,
    shown: &str,
    offset: usize,
    limit: usize,
    budget: usize,
) -> Result<Rendered, ToolError> {
    let notebook = parse(text, shown)?;
    let cells = notebook["cells"].as_array().expect("checked by parse");
    let total = cells.len();
    if total == 0 {
        return Ok(Rendered {
            text: "[notebook has no cells]\n".into(),
            total,
            end: 0,
        });
    }
    if offset > total {
        return Err(ToolError::failed(format!(
            "notebook has {total} cells, requested cell {offset}"
        )));
    }

    let mut out = String::new();
    let mut end = offset - 1;
    for (i, cell) in cells.iter().enumerate().skip(offset - 1).take(limit) {
        let block = render_cell(i + 1, cell);
        // The first cell always goes in, cut if it must: a call that shows
        // nothing gives the model nowhere to go on from.
        if !out.is_empty() && out.len() + block.len() > budget {
            break;
        }
        if block.len() > budget {
            out.push_str(&cut(&block, budget));
        } else {
            out.push_str(&block);
        }
        end = i + 1;
    }
    Ok(Rendered { text: out, total, end })
}

fn render_cell(number: usize, cell: &Value) -> String {
    let kind = cell["cell_type"].as_str().unwrap_or("unknown");
    let mut head = format!("[cell {number} · {kind}");
    if let Some(id) = cell["id"].as_str() {
        head.push_str(&format!(" · id {id}"));
    }
    if let Some(count) = cell["execution_count"].as_u64() {
        head.push_str(&format!(" · ran [{count}]"));
    }
    head.push_str("]\n");

    let mut out = head;
    let source = joined(&cell["source"]);
    if source.is_empty() {
        out.push_str("[empty]\n");
    } else {
        out.push_str(&source);
        if !source.ends_with('\n') {
            out.push('\n');
        }
    }

    if let Some(outputs) = cell["outputs"].as_array().filter(|o| !o.is_empty()) {
        let text: String = outputs.iter().map(render_output).collect();
        out.push_str("[output]\n");
        if text.chars().count() > OUTPUT_CHARS {
            let kept: String = text.chars().take(OUTPUT_CHARS).collect();
            out.push_str(&kept);
            out.push_str("\n[output cut]\n");
        } else {
            out.push_str(&text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out.push('\n');
    out
}

fn render_output(output: &Value) -> String {
    match output["output_type"].as_str() {
        Some("stream") => joined(&output["text"]),
        Some("error") => {
            let mut text = format!(
                "{}: {}\n",
                output["ename"].as_str().unwrap_or("Error"),
                output["evalue"].as_str().unwrap_or("")
            );
            if let Some(trace) = output["traceback"].as_array() {
                for line in trace.iter().filter_map(Value::as_str) {
                    text.push_str(&strip_ansi(line));
                    text.push('\n');
                }
            }
            text
        }
        Some("execute_result") | Some("display_data") => {
            let Some(data) = output["data"].as_object() else {
                return String::new();
            };
            let mut text = data.get("text/plain").map(joined).unwrap_or_default();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            for kind in data.keys().filter(|k| k.as_str() != "text/plain") {
                // Rich HTML usually repeats the plain text beside it; an image
                // has nothing a text model could read.
                if kind == "text/html" && data.contains_key("text/plain") {
                    continue;
                }
                text.push_str(&format!("[{kind} output omitted]\n"));
            }
            text
        }
        _ => String::new(),
    }
}

/// `source` and stream `text` are a string or a list of lines.
fn joined(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(lines) => lines.iter().filter_map(Value::as_str).collect(),
        _ => String::new(),
    }
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            // Parameters and intermediates up to the final letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn cut(text: &str, budget: usize) -> String {
    let mut end = budget.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[cell cut]\n", &text[..end])
}

/// Source in the form Jupyter writes it: a list of lines, each but the last
/// keeping its newline.
fn as_lines(source: &str) -> Value {
    Value::Array(
        source
            .split_inclusive('\n')
            .map(|line| Value::String(line.to_string()))
            .collect(),
    )
}

/// The indentation the file was written with: Jupyter writes one space,
/// other tools two or four. A rewrite in another indentation would be a diff
/// of every line.
fn indent_of(text: &str) -> usize {
    text.lines()
        .nth(1)
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .filter(|&n| n > 0)
        .unwrap_or(1)
}

fn serialize(notebook: &Value, original: &str) -> Result<String, ToolError> {
    use serde::Serialize;
    let indent = " ".repeat(indent_of(original));
    let mut buffer = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
    notebook
        .serialize(&mut serializer)
        .map_err(|e| ToolError::failed(format!("notebook could not be written: {e}")))?;
    let mut text = String::from_utf8(buffer).expect("serde_json writes UTF-8");
    if original.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// A new cell id the notebook does not have yet, in the form Jupyter makes them.
fn fresh_id(cells: &[Value]) -> String {
    loop {
        let id: String = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        if !cells.iter().any(|c| c["id"].as_str() == Some(id.as_str())) {
            return id;
        }
    }
}

pub struct NotebookEdit;

#[async_trait]
impl Tool for NotebookEdit {
    fn name(&self) -> &'static str {
        "NotebookEdit"
    }

    fn description(&self) -> &'static str {
        "Change one cell of a Jupyter notebook (.ipynb): replace its source, insert a \
         new cell, or delete one. Read the notebook first: Read shows every cell with \
         its number and id. Name the cell by `cell_id`, or by `cell` (its number) when \
         the notebook has no ids. Use this instead of Edit on a notebook: the JSON is \
         written for you. Replacing a code cell clears its outputs, since they belong \
         to the old code."
    }

    fn properties(&self) -> Value {
        json!({
            "path": { "type": "string", "description": "Path relative to the project root." },
            "op": {
                "type": "string",
                "enum": ["replace", "insert", "delete"],
                "description": "replace: new source for the cell. insert: a new cell \
                    after the named one, or first in the notebook when none is named. \
                    delete: remove the cell.",
            },
            "cell_id": { "type": "string", "description": "Id of the cell, as Read shows it." },
            "cell": {
                "type": "integer",
                "minimum": 1,
                "description": "Number of the cell, counted from one, instead of cell_id.",
            },
            "source": {
                "type": "string",
                "description": "The cell's full new source, for replace and insert.",
            },
            "cell_type": {
                "type": "string",
                "enum": ["code", "markdown"],
                "description": "Required for insert. For replace, changes the cell's type.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path", "op"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        Some(files::edit_need(ws, self.name(), raw, abs))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let op = input["op"]
            .as_str()
            .ok_or_else(|| ToolError::failed("op is required"))?;

        let target = files::look_editable(ctx, raw)?;
        if !is_notebook(&target.abs) {
            return Err(ToolError::failed(format!(
                "{} is not a notebook (.ipynb); use Edit",
                target.rel
            )));
        }
        let Some(base) = target.current.clone() else {
            return Err(ToolError::failed(format!("not found: {raw}")));
        };

        let mut notebook = parse(&base, &target.rel)?;
        let done = apply(&mut notebook, op, &input)?;
        let updated = serialize(&notebook, &base)?;
        let (fact, diff) = files::write(ctx, &target, ChangeKind::Modified, Some(&updated))?;

        Ok(ToolOutput::new(format!(
            "Modified {} (+{} −{}): {done}\n",
            target.rel, fact.added, fact.removed
        ))
        .changed(FileChange { fact, diff }))
    }
}

/// Finds the cell the call names: by id, or by number.
fn find(cells: &[Value], input: &Value) -> Result<Option<usize>, ToolError> {
    if let Some(id) = input["cell_id"].as_str() {
        return cells
            .iter()
            .position(|c| c["id"].as_str() == Some(id))
            .map(Some)
            .ok_or_else(|| ToolError::failed(format!("no cell with id {id}")));
    }
    if let Some(number) = input["cell"].as_u64() {
        let at = number as usize;
        if at == 0 || at > cells.len() {
            return Err(ToolError::failed(format!(
                "notebook has {} cells, cell {number} was named",
                cells.len()
            )));
        }
        return Ok(Some(at - 1));
    }
    Ok(None)
}

/// Applies one operation; returns what was done, for the result line.
fn apply(notebook: &mut Value, op: &str, input: &Value) -> Result<String, ToolError> {
    // Ids came with nbformat 4.5: a notebook that has them gets one on every
    // new cell, and one that does not is not given a field it never had.
    let with_ids = notebook["nbformat"].as_u64().unwrap_or(4) > 4
        || notebook["nbformat_minor"].as_u64().unwrap_or(0) >= 5;
    let cells = notebook["cells"].as_array_mut().expect("checked by parse");
    let source = input["source"].as_str();
    let kind = input["cell_type"].as_str();
    if let Some(kind) = kind.filter(|k| !matches!(*k, "code" | "markdown")) {
        return Err(ToolError::failed(format!("cell_type must be code or markdown, got {kind}")));
    }

    match op {
        "replace" => {
            let at = find(cells, input)?
                .ok_or_else(|| ToolError::failed("replace needs cell_id or cell"))?;
            let source = source.ok_or_else(|| ToolError::failed("replace needs source"))?;
            let cell = cells[at]
                .as_object_mut()
                .ok_or_else(|| ToolError::failed(format!("cell {} is not an object", at + 1)))?;
            let kind = kind.unwrap_or_else(|| cell.get("cell_type").and_then(Value::as_str).unwrap_or("code"));
            let kind = kind.to_string();
            cell.insert("source".into(), as_lines(source));
            shape(cell, &kind);
            Ok(format!("replaced cell {}", at + 1))
        }
        "insert" => {
            let kind = kind.ok_or_else(|| ToolError::failed("insert needs cell_type"))?;
            let source = source.ok_or_else(|| ToolError::failed("insert needs source"))?;
            let at = find(cells, input)?.map_or(0, |after| after + 1);
            let mut cell = Map::new();
            if with_ids {
                cell.insert("id".into(), Value::String(fresh_id(cells)));
            }
            cell.insert("metadata".into(), json!({}));
            cell.insert("source".into(), as_lines(source));
            shape(&mut cell, kind);
            cells.insert(at, Value::Object(cell));
            Ok(format!("inserted cell {}", at + 1))
        }
        "delete" => {
            let at = find(cells, input)?
                .ok_or_else(|| ToolError::failed("delete needs cell_id or cell"))?;
            cells.remove(at);
            Ok(format!("deleted cell {}", at + 1))
        }
        other => Err(ToolError::failed(format!(
            "unknown op {other}: use replace, insert or delete"
        ))),
    }
}

/// Gives a cell the fields its type requires and drops the ones it may not
/// have. A code cell whose source changed loses its outputs: they were
/// printed by code that is no longer there.
fn shape(cell: &mut Map<String, Value>, kind: &str) {
    cell.insert("cell_type".into(), Value::String(kind.to_string()));
    if kind == "code" {
        cell.insert("outputs".into(), json!([]));
        cell.insert("execution_count".into(), Value::Null);
    } else {
        cell.remove("outputs");
        cell.remove("execution_count");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    const NOTEBOOK: &str = r##"{
 "cells": [
  {
   "cell_type": "markdown",
   "id": "aaaa0001",
   "metadata": {},
   "source": [
    "# Title"
   ]
  },
  {
   "cell_type": "code",
   "execution_count": 3,
   "id": "aaaa0002",
   "metadata": {
    "tags": ["keep"]
   },
   "outputs": [
    {
     "name": "stdout",
     "output_type": "stream",
     "text": [
      "hello\n"
     ]
    },
    {
     "data": {
      "image/png": "iVBORw0KGgo=",
      "text/plain": [
       "<Figure>"
      ]
     },
     "metadata": {},
     "output_type": "display_data"
    }
   ],
   "source": [
    "x = 1\n",
    "print('hello')"
   ]
  }
 ],
 "metadata": {},
 "nbformat": 4,
 "nbformat_minor": 5
}
"##;

    #[test]
    fn cells_are_shown_with_number_id_and_outputs() {
        let shown = render(NOTEBOOK, "a.ipynb", 1, 100, 100_000).unwrap();
        assert_eq!(shown.total, 2);
        assert_eq!(shown.end, 2);
        let text = shown.text;
        assert!(text.contains("[cell 1 · markdown · id aaaa0001]\n# Title\n"), "{text}");
        assert!(text.contains("[cell 2 · code · id aaaa0002 · ran [3]]\nx = 1\nprint('hello')\n"), "{text}");
        assert!(text.contains("[output]\nhello\n<Figure>\n[image/png output omitted]\n"), "{text}");
    }

    #[test]
    fn offset_and_limit_count_cells() {
        let shown = render(NOTEBOOK, "a.ipynb", 2, 1, 100_000).unwrap();
        assert!(!shown.text.contains("# Title"));
        assert!(shown.text.contains("cell 2"));
    }

    #[test]
    fn an_error_output_loses_its_terminal_colours() {
        let output = json!({
            "output_type": "error", "ename": "ValueError", "evalue": "bad",
            "traceback": ["\u{1b}[0;31mValueError\u{1b}[0m: bad"],
        });
        assert_eq!(render_output(&output), "ValueError: bad\nValueError: bad\n");
    }

    #[test]
    fn replacing_a_code_cell_clears_its_outputs_and_keeps_the_rest() {
        let mut nb: Value = serde_json::from_str(NOTEBOOK).unwrap();
        let done = apply(&mut nb, "replace", &json!({ "cell_id": "aaaa0002", "source": "y = 2\nprint(y)" })).unwrap();
        assert_eq!(done, "replaced cell 2");
        let cell = &nb["cells"][1];
        assert_eq!(cell["source"], json!(["y = 2\n", "print(y)"]));
        assert_eq!(cell["outputs"], json!([]));
        assert_eq!(cell["execution_count"], Value::Null);
        assert_eq!(cell["metadata"]["tags"], json!(["keep"]));
    }

    #[test]
    fn a_cell_can_change_type() {
        let mut nb: Value = serde_json::from_str(NOTEBOOK).unwrap();
        apply(&mut nb, "replace", &json!({ "cell": 2, "source": "Now prose", "cell_type": "markdown" })).unwrap();
        let cell = &nb["cells"][1];
        assert_eq!(cell["cell_type"], "markdown");
        assert!(cell.get("outputs").is_none() && cell.get("execution_count").is_none());
    }

    #[test]
    fn insert_goes_after_the_named_cell_or_first() {
        let mut nb: Value = serde_json::from_str(NOTEBOOK).unwrap();
        apply(&mut nb, "insert", &json!({ "cell_id": "aaaa0001", "source": "z = 3", "cell_type": "code" })).unwrap();
        assert_eq!(nb["cells"][1]["source"], json!(["z = 3"]));
        let id = nb["cells"][1]["id"].as_str().unwrap();
        assert_eq!(id.len(), 8);
        assert_ne!(id, "aaaa0001");

        apply(&mut nb, "insert", &json!({ "source": "# Top", "cell_type": "markdown" })).unwrap();
        assert_eq!(nb["cells"][0]["source"], json!(["# Top"]));
        assert_eq!(nb["cells"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn an_old_notebook_gets_no_ids() {
        let mut nb = json!({ "cells": [], "nbformat": 4, "nbformat_minor": 2 });
        apply(&mut nb, "insert", &json!({ "source": "x", "cell_type": "code" })).unwrap();
        assert!(nb["cells"][0].get("id").is_none());
    }

    #[test]
    fn delete_and_bad_names_are_reported() {
        let mut nb: Value = serde_json::from_str(NOTEBOOK).unwrap();
        assert!(apply(&mut nb, "delete", &json!({ "cell_id": "nope" })).is_err());
        assert!(apply(&mut nb, "delete", &json!({ "cell": 3 })).is_err());
        apply(&mut nb, "delete", &json!({ "cell": 1 })).unwrap();
        assert_eq!(nb["cells"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_file_keeps_its_indentation_and_newline() {
        let nb: Value = serde_json::from_str(NOTEBOOK).unwrap();
        let text = serialize(&nb, NOTEBOOK).unwrap();
        assert!(text.starts_with("{\n \"cells\": [\n  {"), "{text}");
        assert!(text.ends_with("}\n"));
        let four = serialize(&nb, "{\n    \"cells\": []\n}").unwrap();
        assert!(four.starts_with("{\n    \"cells\""));
        assert!(!four.ends_with('\n'));
    }

    #[tokio::test]
    async fn the_tool_edits_a_notebook_it_has_read() {
        let dir = TempDir::new();
        let path = dir.write("a.ipynb", NOTEBOOK);
        let (ctx, ws) = ctx_for(&dir);

        let err = NotebookEdit
            .run(&ctx, json!({ "path": "a.ipynb", "op": "delete", "cell": 1 }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("was not read"), "{err}");

        ws.mark_read(&path);
        let out = NotebookEdit
            .run(&ctx, json!({ "path": "a.ipynb", "op": "replace", "cell": 1, "source": "# New title" }))
            .await
            .unwrap();
        assert!(out.text.contains("replaced cell 1"), "{}", out.text);
        assert!(out.changed.is_some());
        let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["cells"][0]["source"], json!(["# New title"]));
    }

    #[tokio::test]
    async fn the_tool_refuses_a_file_that_is_not_a_notebook() {
        let dir = TempDir::new();
        let path = dir.write("a.json", "{}");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);
        let err = NotebookEdit
            .run(&ctx, json!({ "path": "a.json", "op": "delete", "cell": 1 }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not a notebook"), "{err}");
    }
}

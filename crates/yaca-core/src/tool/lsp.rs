//! Questions put to a language server: where a name is defined, who uses it,
//! what type it has, what a file contains, what the compiler objects to.
//!
//! The model names a line and the text on it; the column is found here
//! (`lsp::format::locate`). It does not count columns well, and a server that
//! is asked about the wrong column answers about something else without
//! saying so.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files::read_text;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::lsp::format::{self, locate};
use crate::lsp::{Lsp, LspError};
use crate::uri::file_uri;

/// A lookup is not a build: a server that takes longer is stuck or indexing,
/// and the model is better told so than kept waiting.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long `diagnostics` waits for a file the server has not looked at yet.
/// Cold, a server may be loading the whole project first.
const DIAGNOSTICS_WAIT: Duration = Duration::from_secs(10);

const MAX_PLACES: usize = 50;
const MAX_SYMBOLS: usize = 200;
const MAX_DIAGNOSTICS: usize = 50;
const MAX_HOVER_CHARS: usize = 6_000;

pub struct LspQuery;

#[async_trait]
impl Tool for LspQuery {
    fn name(&self) -> &'static str {
        "Lsp"
    }

    fn description(&self) -> &'static str {
        "Ask the language server about code, instead of guessing from text search. \
         `definition`: where the name at a position is defined. `references`: every \
         use of it. `hover`: its type and documentation. `symbols`: the outline of \
         one file. `workspace_symbols`: find a symbol by name across the project. \
         `diagnostics`: the errors and warnings the server reports for a file. \
         A position is `path` + `line` (1-based) + `symbol`, the text on that line \
         to stand on; do not count columns. Prefer this to Grep when you need the \
         actual definition or the actual callers of a name, and to re-reading a file \
         when you need its type. Positions refer to the file as it is on disk."
    }

    fn properties(&self) -> Value {
        json!({
            "action": {
                "type": "string",
                "enum": ["definition", "references", "hover", "symbols",
                         "workspace_symbols", "diagnostics"],
                "description": "What to ask.",
            },
            "path": {
                "type": "string",
                "description": "File to ask about. Required for every action except \
                                workspace_symbols, where it only picks the server.",
            },
            "line": {
                "type": "integer",
                "minimum": 1,
                "description": "1-based line. Required for definition, references and hover.",
            },
            "symbol": {
                "type": "string",
                "description": "Text on that line to stand at the start of, usually the \
                                identifier. If it occurs more than once, use a longer \
                                fragment (e.g. `self.parse`).",
            },
            "column": {
                "type": "integer",
                "minimum": 1,
                "description": "1-based column, instead of `symbol`.",
            },
            "query": {
                "type": "string",
                "description": "Name or part of a name, for workspace_symbols.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["action"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Asks and changes nothing on disk.
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
        let lsp = ctx
            .lsp
            .as_ref()
            .filter(|lsp| !lsp.names().is_empty())
            .ok_or_else(|| ToolError::failed("no language servers in this session"))?;
        let action = input["action"]
            .as_str()
            .ok_or_else(|| ToolError::failed("action is required"))?;

        let file = match input["path"].as_str() {
            Some(raw) => Some(open(ctx, raw)?),
            None if action == "workspace_symbols" => None,
            None => return Err(ToolError::failed(format!("path is required for {action}"))),
        };
        let server = pick(lsp, file.as_ref().map(|f| f.abs.as_path()))?;
        let client = lsp.client(&server, &ctx.cancel).await.map_err(mapped)?;

        let ask = |method: &'static str, params: Value| {
            let client = client.clone();
            let cancel = ctx.cancel.clone();
            async move {
                client
                    .request(method, params, REQUEST_TIMEOUT, &cancel)
                    .await
                    .map_err(mapped)
            }
        };
        let show = |path: &Path| ctx.ws.display(&ctx.ws.in_form(path));

        if action == "workspace_symbols" {
            let query = input["query"]
                .as_str()
                .filter(|q| !q.trim().is_empty())
                .ok_or_else(|| ToolError::failed("query is required for workspace_symbols"))?;
            let answer = ask("workspace/symbol", json!({ "query": query })).await?;
            let (text, total) = format::render_found(&answer, MAX_SYMBOLS, &show);
            return Ok(found_or(text, total, format!("No symbols found for \"{query}\"."))
                .truncated(total > MAX_SYMBOLS));
        }

        // The file is always `Some` from here: every other action needs it.
        let file = file.expect("a path was required above");
        let before = client.published(&file_uri(&file.abs));
        let (uri, told) = client.sync(&file.abs, &file.text).await.map_err(mapped)?;
        let document = json!({ "uri": uri });

        match action {
            "symbols" => {
                let answer = ask("textDocument/documentSymbol", json!({ "textDocument": document })).await?;
                let (text, total) = format::render_outline(&answer, MAX_SYMBOLS);
                Ok(found_or(text, total, format!("No symbols in {}.", file.shown))
                    .truncated(total > MAX_SYMBOLS))
            }
            "diagnostics" => diagnostics(&client, &file, &uri, before, told, ctx).await,
            "definition" | "references" | "hover" => {
                let line = input["line"]
                    .as_u64()
                    .ok_or_else(|| ToolError::failed(format!("line is required for {action}")))?;
                let (line, character) = locate(
                    &file.text,
                    line as usize,
                    input["symbol"].as_str(),
                    input["column"].as_u64().map(|c| c as usize),
                )
                .map_err(ToolError::failed)?;
                let mut params = json!({
                    "textDocument": document,
                    "position": { "line": line, "character": character },
                });
                match action {
                    "definition" => {
                        let answer = ask("textDocument/definition", params).await?;
                        let places = format::places(&answer);
                        let text = format::render_places(&places, MAX_PLACES, &show);
                        Ok(found_or(text, places.len(), "Definition not found.".into()))
                    }
                    "references" => {
                        params["context"] = json!({ "includeDeclaration": true });
                        let answer = ask("textDocument/references", params).await?;
                        let places = format::places(&answer);
                        let text = format::render_places(&places, MAX_PLACES, &show);
                        Ok(found_or(text, places.len(), "No references found.".into())
                            .truncated(places.len() > MAX_PLACES))
                    }
                    _ => {
                        let answer = ask("textDocument/hover", params).await?;
                        match format::hover_text(&answer) {
                            Some(text) => {
                                let cut: String = text.chars().take(MAX_HOVER_CHARS).collect();
                                let more = text.chars().count() > MAX_HOVER_CHARS;
                                Ok(ToolOutput::new(cut).items(1).truncated(more))
                            }
                            None => Ok(ToolOutput::new("The server has no information for this position.".into())
                                .items(0)),
                        }
                    }
                }
            }
            other => Err(ToolError::failed(format!("unknown action: {other}"))),
        }
    }
}

/// An open, readable file of the project.
struct File {
    abs: PathBuf,
    shown: String,
    text: String,
}

fn open(ctx: &ToolCtx, raw: &str) -> Result<File, ToolError> {
    let abs = ctx.ws.resolve_existing(raw)?;
    if abs.is_dir() {
        return Err(ToolError::failed(format!("{raw} is a directory, not a file")));
    }
    let text = read_text(&abs)?.ok_or_else(|| ToolError::failed(format!("not found: {raw}")))?;
    Ok(File {
        shown: ctx.ws.display(&abs),
        abs,
        text,
    })
}

/// The server for a file, or the only one there is. A model that does not
/// know which server it wants is told what it can name.
fn pick(lsp: &Lsp, path: Option<&Path>) -> Result<String, ToolError> {
    if let Some(path) = path {
        return lsp.server_for(path).ok_or_else(|| {
            let known: Vec<String> = lsp
                .snapshot()
                .iter()
                .map(|s| format!("{} ({})", s.name, s.extensions.join(", ")))
                .collect();
            ToolError::failed_with(
                format!("no language server for {}", path.display()),
                format!(
                    "No language server handles this file's extension. Known servers: {}. \
                     Use Grep and Read for this file.",
                    known.join("; ")
                ),
            )
        });
    }
    let names = lsp.names();
    match (lsp.only_ready(), names.as_slice()) {
        (Some(name), _) => Ok(name),
        (None, [only]) => Ok(only.clone()),
        _ => Err(ToolError::failed(format!(
            "specify path: multiple servers available ({})",
            names.join(", ")
        ))),
    }
}

async fn diagnostics(
    client: &crate::lsp::Client,
    file: &File,
    uri: &str,
    before: u64,
    told: bool,
    ctx: &ToolCtx,
) -> Result<ToolOutput, ToolError> {
    // A document the server has just been told something new about is waited
    // for. One it already had is answered from what it last published, which
    // is current: nothing has changed since.
    let after = if told { before } else { before.saturating_sub(1) };
    let (items, arrived) = client
        .diagnostics(uri, after, DIAGNOSTICS_WAIT, &ctx.cancel)
        .await;
    if !arrived && items.is_empty() {
        return Ok(ToolOutput::new(format!(
            "Server has not yet sent diagnostics for {}: project may still be indexing. \
             Retry the request later.",
            file.shown
        ))
        .items(0));
    }
    let (text, rest) =
        format::render_diagnostics(&items, &file.shown, &file.abs, 2, MAX_DIAGNOSTICS, 500);
    if text.is_empty() {
        return Ok(ToolOutput::new(format!("No errors or warnings in {}.", file.shown)).items(0));
    }
    let n = text.lines().count() as u32;
    let mut text = text;
    if rest > 0 {
        text.push_str(&format!("[showing first {MAX_DIAGNOSTICS}, {rest} more]\n"));
    }
    Ok(ToolOutput::new(text).items(n).truncated(rest > 0))
}

fn found_or(text: String, total: usize, none: String) -> ToolOutput {
    if total == 0 || text.trim().is_empty() {
        ToolOutput::new(none).items(0)
    } else {
        ToolOutput::new(text).items(total as u32)
    }
}

fn mapped(error: LspError) -> ToolError {
    match error {
        LspError::Cancelled => ToolError::Cancelled,
        other => ToolError::failed(format!("language server: {other}")),
    }
}

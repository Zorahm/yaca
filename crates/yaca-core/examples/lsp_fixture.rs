//! A small language server over stdio, for the tests in `tests/lsp.rs`.
//!
//! An example rather than a test helper because it has to be a program of its
//! own: the client under test starts a process and talks to its standard
//! streams, and `cargo test` builds examples next to the test binaries.
//!
//! It "understands" one thing: a line containing `BROKEN` is an error. Every
//! other answer is canned, and says enough about the request to check that the
//! client asked what it meant to.
//!
//! A rename is answered only at `Thing` in `struct Thing` (line 0, character
//! 7), and renames it there and in `use Thing;` on the first line of `b.zz`
//! beside the file. Renaming to `Moved` asks to move the file as well.
//!
//! Flags, for the ways a server goes wrong:
//! * `--silent` — never publish diagnostics.
//! * `--pull` — never publish either, but answer `textDocument/diagnostic`
//!   (pull diagnostics) and say so in its capabilities.
//! * `--crash-on-hover` — exit when asked for a hover, mid-session.
//! * `--crash` — say why on standard error and exit before the handshake.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

fn main() {
    let flag = std::env::args().nth(1).unwrap_or_default();
    if flag == "--crash" {
        eprintln!("boom: no toolchain");
        std::process::exit(3);
    }

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    // Set once the client has answered the question the server asked it.
    let mut configured = false;

    // The text of every open document, for pull diagnostics.
    let mut docs: std::collections::HashMap<String, String> = Default::default();

    // Messages that arrived while the server was waiting for an answer.
    let mut queued: std::collections::VecDeque<Value> = Default::default();

    while let Some(message) = queued.pop_front().or_else(|| read(&mut input)) {
        let method = message["method"].as_str().unwrap_or_default().to_string();
        let params = message["params"].clone();

        match method.as_str() {
            "initialized" => {
                send(&json!({
                    "jsonrpc": "2.0", "id": "cfg", "method": "workspace/configuration",
                    "params": { "items": [{ "section": "fixture" }] },
                }));
                // Waits for the answer, so that whatever is asked next is
                // answered knowing whether the client gave it. Nothing else
                // is lost: the rest is kept in order.
                while let Some(next) = read(&mut input) {
                    if next["method"].is_null() && next["id"] == json!("cfg") {
                        configured = next["result"] == json!([null]);
                        break;
                    }
                    queued.push_back(next);
                }
            }
            "textDocument/didOpen" => {
                let doc = &params["textDocument"];
                let uri = doc["uri"].as_str().unwrap_or_default();
                let text = doc["text"].as_str().unwrap_or_default();
                docs.insert(uri.to_string(), text.to_string());
                publish(&flag, uri, text);
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
                let text = params["contentChanges"][0]["text"].as_str().unwrap_or_default();
                docs.insert(uri.to_string(), text.to_string());
                publish(&flag, uri, text);
            }
            "exit" => return,
            _ => {}
        }

        let Some(id) = message.get("id").cloned().filter(|_| !method.is_empty()) else {
            continue;
        };
        let uri = params["textDocument"]["uri"].clone();
        let at = |line: u64, character: u64| {
            json!({ "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character + 1 } })
        };
        let result = match method.as_str() {
            "initialize" => {
                let mut capabilities = json!({ "textDocumentSync": 1, "definitionProvider": true });
                if flag == "--pull" {
                    capabilities["diagnosticProvider"] =
                        json!({ "interFileDependencies": false, "workspaceDiagnostics": false });
                }
                json!({
                    "capabilities": capabilities,
                    "serverInfo": { "name": "fixture-ls", "version": "1.0" },
                })
            }
            "textDocument/diagnostic" => {
                let text = uri.as_str().and_then(|u| docs.get(u)).cloned().unwrap_or_default();
                json!({ "kind": "full", "items": broken(&text) })
            }
            // Always the same place; the test checks the request named the
            // right position by what hover says about it below.
            "textDocument/definition" => json!({ "uri": uri, "range": at(0, 3) }),
            "textDocument/references" => json!([
                { "uri": uri, "range": at(0, 3) },
                { "uri": uri, "range": at(2, 4) },
            ]),
            "textDocument/hover" => {
                if flag == "--crash-on-hover" {
                    eprintln!("hover is not implemented");
                    std::process::exit(5);
                }
                let position = &params["position"];
                json!({ "contents": { "kind": "plaintext", "value": format!(
                    "at {}:{} configured={configured}",
                    position["line"], position["character"]
                ) } })
            }
            "textDocument/documentSymbol" => json!([{
                "name": "Thing", "kind": 23, "range": at(0, 0), "selectionRange": at(0, 7),
                "children": [{ "name": "go", "kind": 6, "range": at(2, 0), "selectionRange": at(2, 4) }],
            }]),
            "workspace/symbol" => json!([{
                "name": params["query"], "kind": 12,
                "location": { "uri": "file:///elsewhere/lib.rs", "range": at(9, 0) },
                "containerName": "lib",
            }]),
            "textDocument/prepareRename" => {
                let position = &params["position"];
                if position["line"] == 0 && position["character"] == 7 {
                    json!({ "start": { "line": 0, "character": 7 }, "end": { "line": 0, "character": 12 } })
                } else {
                    Value::Null
                }
            }
            "textDocument/rename" => {
                let name = params["newName"].as_str().unwrap_or_default();
                let here = uri.as_str().unwrap_or_default();
                let sibling = here.replace("a.zz", "b.zz");
                let span = |line: u64, from: u64, to: u64| {
                    json!({ "start": { "line": line, "character": from },
                            "end": { "line": line, "character": to } })
                };
                let mut changes = vec![
                    json!({ "textDocument": { "uri": here, "version": null },
                            "edits": [{ "range": span(0, 7, 12), "newText": name }] }),
                    json!({ "textDocument": { "uri": sibling, "version": null },
                            "edits": [{ "range": span(0, 4, 9), "newText": name }] }),
                ];
                if name == "Moved" {
                    changes.push(json!({ "kind": "rename", "oldUri": here,
                                         "newUri": here.replace("a.zz", "moved.zz") }));
                }
                json!({ "documentChanges": changes })
            }
            "shutdown" => Value::Null,
            _ => Value::Null,
        };
        send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
}

fn publish(flag: &str, uri: &str, text: &str) {
    if flag == "--silent" || flag == "--pull" {
        return;
    }
    send(&json!({
        "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": broken(text) },
    }));
}

/// An error on every line that says `BROKEN`.
fn broken(text: &str) -> Vec<Value> {
    text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("BROKEN"))
        .map(|(n, line)| {
            let character = line.find("BROKEN").unwrap_or(0);
            json!({
                "range": { "start": { "line": n, "character": character },
                           "end": { "line": n, "character": character + 6 } },
                "severity": 1, "source": "fixture", "code": "E1",
                "message": "this line is BROKEN",
            })
        })
        .collect()
}

fn read(input: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn send(message: &Value) {
    let body = message.to_string();
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "Content-Length: {}\r\n\r\n{body}", body.len());
    let _ = out.flush();
}

//! A small MCP server over stdio, for the tests in `tests/mcp.rs`.
//!
//! An example rather than a test helper because it has to be a program of its
//! own: the client under test starts a process and talks to its standard
//! streams, and `cargo test` builds examples next to the test binaries.
//!
//! Flags, for the ways a server goes wrong:
//! * `--crash` — say why on standard error and exit before the handshake.
//! * `--hang` — read forever and answer nothing.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

fn main() {
    let flag = std::env::args().nth(1).unwrap_or_default();
    if flag == "--crash" {
        eprintln!("boom: the token is missing");
        std::process::exit(3);
    }

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    // Set by the `grow` tool: the server has one more tool from then on.
    let mut grown = false;

    while let Some(Ok(line)) = lines.next() {
        if flag == "--hang" {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = message.get("id").cloned() else {
            // A notification: nothing is owed.
            continue;
        };
        let params = &message["params"];
        let result = match message["method"].as_str().unwrap_or_default() {
            "initialize" => json!({
                "protocolVersion": params["protocolVersion"],
                "capabilities": { "tools": { "listChanged": true }, "resources": {}, "prompts": {} },
                "serverInfo": { "name": "fixture", "version": "1.0" },
                "instructions": "Use echo to repeat a text back.",
            }),
            "tools/list" => json!({ "tools": tools(grown) }),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or_default();
                let args = &params["arguments"];
                match name {
                    "grow" => {
                        grown = true;
                        send(&json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
                        text("grown")
                    }
                    // Asks the client a question of its own and answers with
                    // what it said.
                    "roots" => {
                        send(&json!({ "jsonrpc": "2.0", "id": "r1", "method": "roots/list" }));
                        let mut answer = Value::Null;
                        for line in lines.by_ref() {
                            let Ok(line) = line else { break };
                            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                                continue;
                            };
                            if reply["id"] == "r1" {
                                answer = reply["result"].clone();
                                break;
                            }
                        }
                        text(&answer.to_string())
                    }
                    // Asks the human through the client (elicitation) and
                    // answers with what came back.
                    "ask" => {
                        send(&json!({
                            "jsonrpc": "2.0", "id": "e1", "method": "elicitation/create",
                            "params": {
                                "message": "Who is this for?",
                                "requestedSchema": {
                                    "type": "object",
                                    "properties": { "name": { "type": "string", "title": "Name" } },
                                    "required": ["name"],
                                },
                            },
                        }));
                        let mut answer = Value::Null;
                        for line in lines.by_ref() {
                            let Ok(line) = line else { break };
                            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                                continue;
                            };
                            if reply["id"] == "e1" {
                                answer = reply["result"].clone();
                                break;
                            }
                        }
                        text(&answer.to_string())
                    }
                    other => call(other, args),
                }
            }
            "resources/list" => json!({ "resources": [{
                "uri": "note://hello", "name": "hello", "mimeType": "text/plain",
                "description": "A greeting",
            }] }),
            "resources/read" => match params["uri"].as_str() {
                Some("note://hello") => json!({ "contents": [{
                    "uri": "note://hello", "mimeType": "text/plain", "text": "hello from a resource",
                }] }),
                _ => {
                    send(&json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32002, "message": "resource not found" },
                    }));
                    continue;
                }
            },
            "prompts/list" => json!({ "prompts": [{
                "name": "review",
                "description": "Review a pull request",
                "arguments": [
                    { "name": "pr", "description": "Its number", "required": true },
                    { "name": "focus", "description": "What to look at" },
                ],
            }] }),
            "prompts/get" => {
                let args = &params["arguments"];
                json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": format!(
                    "Review PR {} focusing on {}",
                    args["pr"].as_str().unwrap_or("?"),
                    args["focus"].as_str().unwrap_or("everything"),
                ) } }] })
            }
            "ping" => json!({}),
            other => {
                send(&json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32601, "message": format!("unknown method {other}") },
                }));
                continue;
            }
        };
        send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
}

fn send(message: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

fn text(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn call(name: &str, args: &Value) -> Value {
    match name {
        "echo" => text(args["text"].as_str().unwrap_or_default()),
        // The one tool with an effect, so a test can tell "ran" from "did not".
        "write_note" => {
            let path = args["path"].as_str().unwrap_or_default();
            match std::fs::write(path, args["text"].as_str().unwrap_or_default()) {
                Ok(()) => text("written"),
                Err(err) => failure(&err.to_string()),
            }
        }
        "fail" => failure("the tracker has no such task"),
        "slow" => {
            std::thread::sleep(std::time::Duration::from_millis(args["ms"].as_u64().unwrap_or(0)));
            text("done")
        }
        // Has an argument called `description` of its own.
        "describe" => text(&format!("described: {}", args["description"].as_str().unwrap_or("-"))),
        "late" => text("late"),
        other => failure(&format!("unknown tool {other}")),
    }
}

fn failure(why: &str) -> Value {
    json!({ "isError": true, "content": [{ "type": "text", "text": why }] })
}

fn tools(grown: bool) -> Vec<Value> {
    let string = |name: &str| json!({ name: { "type": "string" } });
    let read_only = json!({ "readOnlyHint": true });
    let mut tools = vec![
        json!({
            "name": "echo", "description": "Repeats the text back.",
            "inputSchema": { "type": "object", "properties": string("text"), "required": ["text"] },
            "annotations": read_only,
        }),
        json!({
            "name": "write_note", "description": "Writes text to a file.",
            "inputSchema": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": "object",
                "properties": { "path": { "type": "string" }, "text": { "type": "string" } },
                "required": ["path", "text"],
            },
        }),
        json!({
            "name": "fail", "description": "Always fails.",
            "inputSchema": { "type": "object" }, "annotations": read_only,
        }),
        json!({
            "name": "slow", "description": "Sleeps.",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer" } } },
            "annotations": read_only,
        }),
        json!({
            "name": "describe", "description": "Takes a description.",
            "inputSchema": { "type": "object", "properties": string("description") },
            "annotations": read_only,
        }),
        json!({
            "name": "grow", "description": "Adds a tool to this server.",
            "inputSchema": { "type": "object" }, "annotations": read_only,
        }),
        json!({
            "name": "roots", "description": "Says which roots the client reports.",
            "inputSchema": { "type": "object" }, "annotations": read_only,
        }),
        json!({
            "name": "ask", "description": "Asks the human for a name and says what came back.",
            "inputSchema": { "type": "object" }, "annotations": read_only,
        }),
    ];
    if grown {
        tools.push(json!({
            "name": "late", "description": "Appeared after the server started.",
            "inputSchema": { "type": "object" }, "annotations": read_only,
        }));
    }
    tools
}

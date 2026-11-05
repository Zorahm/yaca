//! The requests and panel rows the feed draws beside the conversation: what the
//! agent asks the human (permissions, questions, forms) and the state of what
//! runs beside it (MCP and language servers, background processes), plus the
//! queue and the task list. Each list is built by iterating a variant list, so
//! every variant is in the fixtures by construction.

use serde_json::json;
use yaca_core::event::{QueuedMessage, TodoItem};
use yaca_core::lsp::{LspServerSummary, LspStatus};
use yaca_core::mcp::{
    ElicitField, ElicitKind, ElicitationRequest, McpPromptSummary, McpServerSummary, McpStatus,
    McpToolSummary,
};
use yaca_core::permission::{FileCount, Invariant, PermissionKind, PermissionRequest, Unsaved};
use yaca_core::process::{ProcessId, ProcessState, ProcessSummary};
use yaca_core::question::{QuestionOption, QuestionRequest};

use crate::ids;
use crate::variants;

/// The target and the detail the permission block shows for each kind.
fn sample(kind: &PermissionKind) -> (&'static str, &'static str) {
    match kind {
        PermissionKind::RunCommand => ("cargo", "cargo test -p yaca-remote --test fixtures"),
        PermissionKind::DeleteFile => ("old.rs", "src/old.rs"),
        PermissionKind::DeleteDirectory => ("cache", "target/cache"),
        PermissionKind::MoveFile => ("lib.rs", "src/lib.rs -> src/core.rs"),
        PermissionKind::EditFile => ("greet.rs", "src/greet.rs"),
        PermissionKind::FetchUrl => ("docs.rs", "https://docs.rs/serde/latest/serde/"),
        PermissionKind::McpWrite => ("create_task", "{\"name\":\"Release notes\"}"),
    }
}

/// A request with every optional field empty; callers set the ones they show.
fn blank_request(n: u32, kind: PermissionKind, target: &str, detail: &str) -> PermissionRequest {
    PermissionRequest {
        id: ids::request(n),
        call_id: ids::call(n),
        kind,
        target: target.to_owned(),
        detail: detail.to_owned(),
        note: None,
        size_bytes: None,
        files: None,
        invariant: None,
        unsaved: None,
        server: None,
        scopes: vec![],
        position: 0,
        total: 0,
    }
}

/// One request per kind, one per invariant, and two that set every option at
/// once and none at all. The optional fields are the feed's decisions about
/// what to show, so the extremes get fixtures of their own.
pub fn permission_requests() -> Vec<PermissionRequest> {
    let mut requests: Vec<PermissionRequest> = variants::permission_kinds()
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            let (target, detail) = sample(&kind);
            let is_command = matches!(kind, PermissionKind::RunCommand);
            let is_delete_file = matches!(kind, PermissionKind::DeleteFile);
            let is_delete_dir = matches!(kind, PermissionKind::DeleteDirectory);
            let is_mcp = matches!(kind, PermissionKind::McpWrite);
            PermissionRequest {
                note: is_command.then(|| "crates/yaca-remote".to_owned()),
                size_bytes: is_delete_file.then_some(2_048),
                files: is_delete_dir.then_some(FileCount {
                    files: 120,
                    more: true,
                }),
                unsaved: is_delete_dir.then(|| Unsaved {
                    dirs: vec!["target/cache/tmp".to_owned()],
                    dir_count: 3,
                    files: 2,
                    roots: vec!["D:/shared-libs".to_owned()],
                }),
                server: is_mcp.then(|| "asana".to_owned()),
                scopes: if is_command {
                    variants::scope_keys()
                } else {
                    vec![]
                },
                kind,
                ..blank_request(i as u32 + 1, PermissionKind::RunCommand, target, detail)
            }
        })
        .collect();

    let first_invariant = requests.len() as u32 + 1;
    requests.extend(variants::invariants().into_iter().enumerate().map(
        |(i, invariant): (usize, Invariant)| PermissionRequest {
            note: Some(".".to_owned()),
            invariant: Some(invariant),
            ..blank_request(
                first_invariant + i as u32,
                PermissionKind::RunCommand,
                "git",
                "git commit -m \"Release notes\"",
            )
        },
    ));

    // Every optional field set, then none set.
    let n = requests.len() as u32 + 1;
    requests.push(PermissionRequest {
        note: Some("crates/yaca-remote".to_owned()),
        size_bytes: Some(4_096),
        files: Some(FileCount {
            files: 3,
            more: false,
        }),
        invariant: Some(Invariant::GitCommit),
        unsaved: Some(Unsaved {
            dirs: vec![],
            dir_count: 0,
            files: 1,
            roots: vec![],
        }),
        server: Some("asana".to_owned()),
        ..blank_request(
            n,
            PermissionKind::McpWrite,
            "create_task",
            "{\"name\":\"Release notes\"}",
        )
    });
    requests.push(blank_request(
        n + 1,
        PermissionKind::EditFile,
        "greet.rs",
        "src/greet.rs",
    ));

    let total = requests.len() as u32;
    for (i, request) in requests.iter_mut().enumerate() {
        request.position = i as u32 + 1;
        request.total = total;
    }
    requests
}

/// A question with options. One allows a free-text answer and is single-select,
/// the other is a multi-select without one; descriptions are present on one
/// option and absent on the others.
pub fn question_requests() -> Vec<QuestionRequest> {
    vec![
        QuestionRequest {
            id: ids::request(20),
            call_id: ids::call(30),
            question: "Which test runner should I use?".to_owned(),
            options: vec![
                QuestionOption {
                    label: "cargo test".to_owned(),
                    description: Some("Runs the workspace tests.".to_owned()),
                },
                QuestionOption {
                    label: "nextest".to_owned(),
                    description: None,
                },
            ],
            allow_other: true,
            multi_select: false,
        },
        QuestionRequest {
            id: ids::request(21),
            call_id: ids::call(31),
            question: "Which crates should be checked?".to_owned(),
            options: vec![
                QuestionOption {
                    label: "yaca-core".to_owned(),
                    description: None,
                },
                QuestionOption {
                    label: "yaca-remote".to_owned(),
                    description: None,
                },
            ],
            allow_other: false,
            multi_select: true,
        },
    ]
}

/// A form with one field of each kind. Required, description and default come
/// in both states, and the choice field carries its options with their labels.
fn elicit_fields() -> Vec<ElicitField> {
    variants::elicit_kinds()
        .into_iter()
        .enumerate()
        .map(|(i, kind)| {
            let is_choice = matches!(kind, ElicitKind::Choice);
            let default = match kind {
                ElicitKind::Text => Some(json!("Release notes")),
                ElicitKind::Number => None,
                ElicitKind::Integer => Some(json!(3)),
                ElicitKind::Boolean => Some(json!(false)),
                ElicitKind::Choice => Some(json!("low")),
            };
            ElicitField {
                name: format!("field_{i}"),
                title: format!("Field {i}"),
                description: (i % 2 == 0).then(|| "Shown under the field.".to_owned()),
                kind,
                required: i % 2 == 0,
                options: if is_choice {
                    vec!["low".to_owned(), "high".to_owned()]
                } else {
                    vec![]
                },
                option_labels: if is_choice {
                    vec!["Low".to_owned(), "High".to_owned()]
                } else {
                    vec![]
                },
                default,
            }
        })
        .collect()
}

pub fn elicitation_requests() -> Vec<ElicitationRequest> {
    vec![ElicitationRequest {
        id: ids::request(40),
        server: "tracker".to_owned(),
        message: "Describe the bug to file.".to_owned(),
        fields: elicit_fields(),
    }]
}

/// One server per MCP status. Source and transport cycle through their lists,
/// so every source and every transport is written as well.
pub fn mcp_servers() -> Vec<McpServerSummary> {
    variants::mcp_statuses()
        .into_iter()
        .enumerate()
        .map(|(i, status)| {
            let ready = matches!(status, McpStatus::Ready);
            let failed = matches!(status, McpStatus::Failed);
            let logging_in = matches!(status, McpStatus::LoggingIn);
            let source = variants::mcp_sources()
                .into_iter()
                .nth(i % 5)
                .expect("five sources");
            let transport = variants::mcp_transports()
                .into_iter()
                .nth(i % 3)
                .expect("three transports");
            McpServerSummary {
                name: format!("server-{i}"),
                source,
                transport,
                target: "npx -y @modelcontextprotocol/server-filesystem .".to_owned(),
                status,
                reason: failed.then(|| "connection refused".to_owned()),
                label: ready.then(|| "filesystem 1.2.0".to_owned()),
                tools: if ready {
                    vec![
                        McpToolSummary {
                            name: "read_file".to_owned(),
                            description: Some("Reads a file.".to_owned()),
                            read_only: true,
                        },
                        McpToolSummary {
                            name: "write_file".to_owned(),
                            description: None,
                            read_only: false,
                        },
                    ]
                } else {
                    vec![]
                },
                resources: ready,
                prompts: if ready {
                    vec![McpPromptSummary {
                        command: format!("mcp__server-{i}__summarize"),
                        description: Some("Summarises a file.".to_owned()),
                        arguments: vec!["path".to_owned()],
                    }]
                } else {
                    vec![]
                },
                login_url: logging_in
                    .then(|| "https://auth.example.com/authorize?state=fixture".to_owned()),
                logged_in: logging_in,
            }
        })
        .collect()
}

/// One language server per status.
pub fn lsp_servers() -> Vec<LspServerSummary> {
    variants::lsp_statuses()
        .into_iter()
        .enumerate()
        .map(|(i, status)| {
            let failed = matches!(status, LspStatus::Failed);
            let ready = matches!(status, LspStatus::Ready);
            LspServerSummary {
                installation: matches!(status, LspStatus::Missing | LspStatus::Installing).then(
                    || "npm install --prefix /config/yaca/lsp/tools/pyright pyright".to_owned(),
                ),
                name: format!("lsp-{i}"),
                command: "rust-analyzer".to_owned(),
                extensions: vec!["rs".to_owned()],
                status,
                reason: failed.then(|| "binary not found".to_owned()),
                label: ready.then(|| "rust-analyzer 2026.10".to_owned()),
            }
        })
        .collect()
}

/// One background command per process state. The one still running has no end
/// time; the ones that ended have one.
pub fn process_summaries() -> Vec<ProcessSummary> {
    variants::process_states()
        .into_iter()
        .enumerate()
        .map(|(i, state)| {
            let n = i as u32 + 1;
            let running = matches!(state, ProcessState::Running);
            ProcessSummary {
                id: ProcessId(n),
                command: "npm run dev".to_owned(),
                cwd: "web".to_owned(),
                state,
                started_at: ids::at(1_000 * u64::from(n)),
                finished_at: (!running).then(|| ids::at(1_000 * u64::from(n) + 500)),
                lines: 42 * n,
                dropped: if n > 2 { 10 * u64::from(n) } else { 0 },
            }
        })
        .collect()
}

pub fn queued_messages() -> Vec<QueuedMessage> {
    vec![
        QueuedMessage {
            id: ids::queued(1),
            text: "Also run the linter.".to_owned(),
            attachments: vec![],
        },
        QueuedMessage {
            id: ids::queued(2),
            text: "Compare with the old one.".to_owned(),
            attachments: vec!["src/greet.rs".to_owned()],
        },
    ]
}

/// The task list: one item per status, in the order the model wrote them.
pub fn todo_items() -> Vec<TodoItem> {
    ["Read src/greet.rs", "Change the greeting", "Run the tests"]
        .into_iter()
        .zip(variants::todo_statuses())
        .map(|(content, status)| TodoItem {
            content: content.to_owned(),
            status,
        })
        .collect()
}

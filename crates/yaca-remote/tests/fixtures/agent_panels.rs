//! The events that feed the panels and the blocks over the feed: what the agent
//! asks the human and how each question closed, the file panel, the background
//! processes, the MCP and language servers, the checkpoints and the queue. Each
//! snapshot event is sent with its whole list, as the core does.

use yaca_core::event::AgentEvent;
use yaca_core::process::ProcessId;

use crate::ids;
use crate::nested_change;
use crate::nested_panels;
use crate::variants;

pub fn all() -> Vec<AgentEvent> {
    let mut events = Vec::new();
    events.extend(permissions());
    events.extend(questions_and_forms());
    events.extend(file_views());
    events.extend(processes());
    events.extend(servers());
    events.extend(checkpoints());
    events.extend(queue());
    events
}

fn permissions() -> Vec<AgentEvent> {
    nested_panels::permission_requests()
        .into_iter()
        .flat_map(|request| {
            let request_id = request.id.clone();
            [
                AgentEvent::PermissionRequested { request },
                AgentEvent::PermissionResolved { request_id },
            ]
        })
        .collect()
}

fn questions_and_forms() -> Vec<AgentEvent> {
    let mut events: Vec<AgentEvent> = nested_panels::question_requests()
        .into_iter()
        .map(|request| AgentEvent::QuestionAsked { request })
        .collect();
    // Every way a question can close, and one the turn stopped before anyone answered.
    events.extend(variants::question_replies().into_iter().map(|reply| {
        AgentEvent::QuestionResolved {
            request_id: ids::request(20),
            reply: Some(reply),
        }
    }));
    events.push(AgentEvent::QuestionResolved {
        request_id: ids::request(21),
        reply: None,
    });

    events.extend(
        nested_panels::elicitation_requests()
            .into_iter()
            .map(|request| AgentEvent::ElicitationRequested { request }),
    );
    events.extend(variants::elicitation_replies().into_iter().map(|reply| {
        AgentEvent::ElicitationResolved {
            request_id: ids::request(40),
            reply: Some(reply),
        }
    }));
    events.push(AgentEvent::ElicitationResolved {
        request_id: ids::request(41),
        reply: None,
    });
    events
}

fn file_views() -> Vec<AgentEvent> {
    let greet = nested_change::fact("src/greet.rs", yaca_core::change::ChangeKind::Modified);
    vec![
        // A file the session changed.
        AgentEvent::FileViewLoaded {
            path: "src/greet.rs".to_owned(),
            view: Some(nested_change::file_view(Some(greet))),
        },
        // A file the agent only read.
        AgentEvent::FileViewLoaded {
            path: "docs/notes.md".to_owned(),
            view: Some(nested_change::file_view(None)),
        },
        // A file that is gone, binary or outside the workspace.
        AgentEvent::FileViewLoaded {
            path: "old/bin.dat".to_owned(),
            view: None,
        },
        // A file cut short before its end.
        AgentEvent::FileViewLoaded {
            path: "src/big.rs".to_owned(),
            view: Some(yaca_core::change::FileView {
                truncated: true,
                ..nested_change::file_view(None)
            }),
        },
    ]
}

fn processes() -> Vec<AgentEvent> {
    vec![
        AgentEvent::ProcessesUpdated {
            processes: nested_panels::process_summaries(),
        },
        AgentEvent::ProcessesUpdated { processes: vec![] },
        AgentEvent::ProcessOutputReset {
            id: ProcessId(2),
            text: "compiling web\nready in 320 ms\n".to_owned(),
            dropped: 1_250,
        },
        AgentEvent::ProcessOutput {
            id: ProcessId(2),
            chunk: "listening on http://localhost:5173\n".to_owned(),
        },
    ]
}

fn servers() -> Vec<AgentEvent> {
    vec![
        AgentEvent::McpUpdated {
            servers: nested_panels::mcp_servers(),
        },
        AgentEvent::LspUpdated {
            servers: nested_panels::lsp_servers(),
        },
    ]
}

fn checkpoints() -> Vec<AgentEvent> {
    let summaries = nested_change::checkpoint_summaries();
    let mut events: Vec<AgentEvent> = summaries
        .iter()
        .cloned()
        .map(|checkpoint| AgentEvent::CheckpointCreated { checkpoint })
        .collect();
    events.push(AgentEvent::CheckpointsUpdated {
        checkpoints: summaries,
    });
    events.push(AgentEvent::CheckpointPreviewed {
        id: ids::checkpoint(1),
        files: nested_change::checkpoint_changes(),
    });
    events.push(AgentEvent::CheckpointRestored {
        id: ids::checkpoint(1),
        at: ids::at(60_000),
        files: 3,
    });
    events
}

fn queue() -> Vec<AgentEvent> {
    vec![
        AgentEvent::QueueUpdated {
            queued: nested_panels::queued_messages(),
        },
        AgentEvent::QueueUpdated { queued: vec![] },
    ]
}

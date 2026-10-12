//! A file attached with a message counts as read from the moment the message
//! is taken in (§7.2). The human attached it, so the agent must not be made to
//! read it again before editing it: that is what the read-before-edit rule
//! would otherwise do.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode};

use common::{calls, is_turn_over, session, until, TempProject};

#[tokio::test]
async fn an_attached_file_can_be_edited_without_reading_it_first() {
    let dir = TempProject::new();
    dir.write("notes.txt", "one\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "e1",
            "Edit",
            json!({ "path": "notes.txt", "old_string": "one", "new_string": "two" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    handle
        .send(Command::SendMessage {
            text: "fix it".into(),
            attachments: vec!["notes.txt".into()],
        })
        .await
        .unwrap();
    let events = until(&mut rx, is_turn_over).await;

    // Refused for "not read in this session" would be a rejection; the edit
    // landing is the proof that the attachment registered the file.
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolCallRejected { .. } | AgentEvent::ToolCallFailed { .. })),
        "{events:?}"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::FileChanged { .. })));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("notes.txt")).unwrap(),
        "two\n"
    );
}

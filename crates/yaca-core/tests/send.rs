//! The public agent API delivers rich metadata while the model gets a receipt.
mod common;

use common::{ask, calls, is_turn_over, session, tool_result_of, until, TempProject};
use serde_json::json;
use std::sync::Arc;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

#[tokio::test]
async fn send_delivers_a_file_without_returning_its_contents_to_the_model() {
    let dir = TempProject::new();
    dir.write("result.csv", "name,value\nprivate-content,42\n");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "s1",
            "Send",
            json!({ "path": "result.csv", "title": "Result", "caption": "Ready" }),
        )])),
        Turn::Events(script_text("done")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "deliver the file").await;
    let seen = until(&mut rx, is_turn_over).await;
    let files: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            AgentEvent::FileSent { file } => Some(file),
            _ => None,
        })
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].title, "Result");
    assert_eq!(files[0].mime, "text/csv");
    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
    assert!(result.contains("Success:"), "{result}");
    assert!(!result.contains("private-content"), "{result}");
    let started = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::ToolCallStarted { tool, .. } if tool == "Send"))
        .unwrap();
    let delivered = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::FileSent { .. }))
        .unwrap();
    let finished = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::ToolCallFinished { .. }))
        .unwrap();
    assert!(started < delivered && delivered < finished);
}

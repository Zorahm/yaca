//! Tool previews must reach the public stream before execution, without bodies.
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{ask, calls, is_turn_over, session, until, TempProject};
use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::usage::TokenCounts;
use yaca_core::{Command, Mode};

fn writing() -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::ToolUse {
                id: "write1".into(),
                name: "Write".into(),
            },
        },
        ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: r#"{"path":"new.rs","content":""#.into(),
        },
        ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: r#"fn main() {}\n"}"#.into(),
        },
        ProviderEvent::BlockStop { index: 0 },
        ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::ToolUse),
            usage: TokenCounts::default(),
        },
        ProviderEvent::MessageStop,
    ]
}

#[tokio::test]
async fn previews_a_write_before_the_model_finishes_then_executes_with_full_arguments() {
    let dir = TempProject::new();
    let provider = Arc::new(
        MockProvider::new(vec![
            Turn::Events(writing()),
            Turn::Events(script_text("Created.")),
        ])
        .with_gap(Duration::from_millis(30)),
    );
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "create a file").await;
    let first = until(&mut rx, |event| matches!(event, AgentEvent::ToolCallPreparing { target: Some(path), .. } if path == "new.rs")).await;
    assert!(!dir.path().join("new.rs").exists());
    assert!(!first.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallStarted { .. } | AgentEvent::FileChanged { .. }
    )));
    assert_eq!(
        first
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ToolCallPreparing { target, .. } => Some(target.as_deref()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![None, Some("new.rs")]
    );
    for event in &first {
        if let AgentEvent::ToolCallPreparing { input, .. } = event {
            assert!(input.get("content").is_none());
            let wire = serde_json::to_value(event).unwrap();
            assert_eq!(wire["type"], "toolCallPreparing");
            assert_eq!(serde_json::from_value::<AgentEvent>(wire).unwrap(), *event);
        }
    }
    let rest = until(&mut rx, is_turn_over).await;
    assert!(rest.iter().any(|event| matches!(event, AgentEvent::ToolCallStarted { input, .. } if input["content"] == "fn main() {}\n")));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new.rs")).unwrap(),
        "fn main() {}\n"
    );
}

#[tokio::test]
async fn stopping_generation_does_not_execute_the_preview() {
    let dir = TempProject::new();
    let provider = Arc::new(
        MockProvider::new(vec![Turn::Events(writing())]).with_gap(Duration::from_millis(50)),
    );
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "create a file").await;
    until(&mut rx, |event| {
        matches!(
            event,
            AgentEvent::ToolCallPreparing {
                target: Some(_),
                ..
            }
        )
    })
    .await;
    handle.send(Command::Cancel).await.unwrap();
    let rest = until(&mut rx, is_turn_over).await;
    assert!(rest
        .iter()
        .any(|event| matches!(event, AgentEvent::TurnCancelled { .. })));
    assert!(!dir.path().join("new.rs").exists());
}

#[tokio::test]
async fn silent_tools_do_not_leak_preview_rows() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "title",
            "SetTitle",
            serde_json::json!({ "title": "Preview test" }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "set title").await;
    let seen = until(&mut rx, is_turn_over).await;
    assert!(seen
        .iter()
        .any(|event| matches!(event, AgentEvent::TitleSet { .. })));
    assert!(!seen.iter().any(|event| matches!(
        event,
        AgentEvent::ToolCallPreparing { .. } | AgentEvent::ToolCallStarted { .. }
    )));
}

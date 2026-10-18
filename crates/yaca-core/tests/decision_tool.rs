//! The agent can ask the Decision model a question — when there is one.
//!
//! Through the public surface only: a session, a scripted agent model and a
//! scripted small model. The tool must reach the small model with the question
//! as written, hand its answer back as the tool result, and not exist at all in
//! a session that has no small model to ask.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::tool::decision::DecisionSetup;
use yaca_core::{Mode, SessionConfig};

use common::{ask, calls, is_turn_over, tool_result_of, until, TempProject};

fn agent_asking(question: &str) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("c1", "Decision", json!({
            "question": question,
            "description": "Choosing a skill",
        }))])),
        Turn::Events(script_text("Using code-review.")),
    ]))
}

fn start(
    dir: &TempProject,
    agent: Arc<MockProvider>,
    decision: Option<DecisionSetup>,
) -> yaca_core::SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    cfg.decision = decision;
    yaca_core::spawn(cfg, agent).unwrap()
}

#[tokio::test]
async fn the_agent_asks_the_small_model_and_reads_its_answer() {
    let dir = TempProject::new();
    let small = Arc::new(MockProvider::text("code-review — the user shows a diff."));
    let agent = agent_asking("Which skill fits: code-review, debugging or refactor?");
    let handle = start(
        &dir,
        agent.clone(),
        Some(DecisionSetup {
            provider: small.clone(),
            model: "qwen3:1.7b".into(),
            systemone: None,
        }),
    );

    let mut rx = handle.subscribe();
    ask(&handle, "look at this diff").await;
    until(&mut rx, is_turn_over).await;

    // The small model got the question as the agent wrote it, under its own id.
    let seen = small.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].model, "qwen3:1.7b");
    assert!(format!("{:?}", seen[0].messages)
        .contains("Which skill fits: code-review, debugging or refactor?"));

    // And its answer is what the agent read as the result of its call.
    let (text, is_error) = tool_result_of(&agent, "c1");
    assert!(!is_error, "{text}");
    assert!(text.contains("code-review"), "{text}");
}

#[tokio::test]
async fn the_tool_is_in_the_request_only_when_there_is_a_small_model() {
    let offered = |decision: Option<DecisionSetup>| async move {
        let dir = TempProject::new();
        let agent = Arc::new(MockProvider::text("ok"));
        let handle = start(&dir, agent.clone(), decision);
        let mut rx = handle.subscribe();
        ask(&handle, "hello").await;
        until(&mut rx, is_turn_over).await;

        let first = agent.seen.lock().unwrap()[0].clone();
        first.tools.iter().any(|t| t.name == "Decision")
    };

    let small = Arc::new(MockProvider::text("unused"));
    assert!(
        offered(Some(DecisionSetup {
            provider: small,
            model: "small".into(),
            systemone: None,
        }))
        .await,
        "configured: the tool is offered"
    );
    assert!(
        !offered(None).await,
        "not configured: the tool must not be in the request at all"
    );
}

#[tokio::test]
async fn a_small_model_that_fails_does_not_stall_the_agent() {
    let dir = TempProject::new();
    let small = Arc::new(MockProvider::new(vec![Turn::Fail(
        yaca_core::ProviderError::Network("connection refused".into()),
    )]));
    let agent = agent_asking("Which one?");
    let handle = start(
        &dir,
        agent.clone(),
        Some(DecisionSetup {
            provider: small,
            model: "small".into(),
            systemone: None,
        }),
    );

    let mut rx = handle.subscribe();
    ask(&handle, "pick").await;
    until(&mut rx, is_turn_over).await;

    // The agent was told what to do next, and went on to answer.
    let (text, is_error) = tool_result_of(&agent, "c1");
    assert!(is_error);
    assert!(text.contains("Decide it yourself"), "{text}");
    assert_eq!(agent.seen.lock().unwrap().len(), 2, "the turn continued");
}

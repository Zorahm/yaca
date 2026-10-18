use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::provider::mock::{script_text, MockProvider, Turn};
use crate::tool::tests_support::ctx_for;
use crate::tool::ToolRegistry;
use crate::testutil::TempDir;

fn tool(mock: Arc<MockProvider>) -> Decision {
    Decision::new(DecisionSetup {
        provider: mock,
        model: "small".into(),
        systemone: None,
    })
}

#[tokio::test]
async fn the_question_goes_to_the_small_model_as_written() {
    let mock = Arc::new(MockProvider::text("code-review — it is a diff."));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);

    let out = tool(mock.clone())
        .run(&ctx, json!({ "question": "Which skill: code-review or debugging?" }))
        .await
        .unwrap();

    assert_eq!(out.text, "code-review — it is a diff.");

    let seen = mock.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.model, "small");
    assert!(req.tools.is_empty());
    assert!(!req.thinking);
    assert_eq!(req.effort, None);
    assert_eq!(req.messages.len(), 1);
    let text = format!("{:?}", req.messages[0].content);
    assert!(text.contains("Which skill: code-review or debugging?"), "{text}");
}

#[tokio::test]
async fn a_missing_question_is_refused_before_anything_is_asked() {
    let mock = Arc::new(MockProvider::text("unused"));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);

    for input in [json!({}), json!({ "question": "   " })] {
        assert!(tool(mock.clone()).run(&ctx, input).await.is_err());
    }
    assert!(mock.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_refusing_model_tells_the_agent_to_carry_on() {
    let mock = Arc::new(MockProvider::new(vec![Turn::Fail(ProviderError::Network(
        "connection refused".into(),
    ))]));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);

    let err = tool(mock)
        .run(&ctx, json!({ "question": "Which one?" }))
        .await
        .unwrap_err();

    // What the model reads says what to do next; what the feed shows is Russian.
    let for_model = err.for_model();
    assert!(for_model.contains("Decide it yourself"), "{for_model}");
    assert!(err.to_string().contains("Decision-модель не ответила"), "{err}");
}

#[tokio::test]
async fn an_empty_reply_is_a_failure_and_not_an_empty_answer() {
    let mock = Arc::new(MockProvider::new(vec![Turn::Events(script_text("   "))]));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);

    assert!(tool(mock)
        .run(&ctx, json!({ "question": "Which one?" }))
        .await
        .is_err());
}

#[tokio::test]
async fn a_slow_model_is_cut_off_by_the_deadline() {
    let mock = Arc::new(MockProvider::text("late").with_gap(Duration::from_millis(300)));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);

    let err = tool(mock)
        .with_timeout(Duration::from_millis(30))
        .run(&ctx, json!({ "question": "Which one?" }))
        .await
        .unwrap_err();

    assert!(err.for_model().contains("in time"), "{}", err.for_model());
}

#[test]
fn a_huge_question_is_cut_and_says_so() {
    let big = "я".repeat(MAX_QUESTION_CHARS + 50);
    let cut = capped(&big);
    assert!(cut.ends_with("[question truncated]"));
    assert_eq!(cut.chars().filter(|c| *c == 'я').count(), MAX_QUESTION_CHARS);
}

#[test]
fn it_is_offered_only_where_it_was_added() {
    let plain = ToolRegistry::of_session(false);
    assert!(!plain.names().any(|n| n == "Decision"));

    let mock = Arc::new(MockProvider::text("x"));
    let with = ToolRegistry::of_session(false).with(vec![Arc::new(tool(mock))]);
    assert!(with.names().any(|n| n == "Decision"));
    // Read-only: offered in Plan as well as Agent.
    assert!(with
        .schemas(crate::intent::Intent::Plan)
        .iter()
        .any(|s| s.name == "Decision"));
}

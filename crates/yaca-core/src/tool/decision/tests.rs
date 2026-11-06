use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::provider::mock::{script_text, MockProvider, Turn};
use crate::testutil::TempDir;
use crate::tool::tests_support::ctx_for;
use crate::tool::ToolRegistry;

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
        .run(
            &ctx,
            json!({ "question": "Which skill: code-review or debugging?" }),
        )
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
    assert!(
        text.contains("Which skill: code-review or debugging?"),
        "{text}"
    );
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

    // A failed side question must not stall the agent's task.
    let for_model = err.for_model();
    assert!(for_model.contains("Decide it yourself"), "{for_model}");
    assert!(
        err.to_string().contains("Decision model did not answer"),
        "{err}"
    );
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
    assert_eq!(
        cut.chars().filter(|c| *c == 'я').count(),
        MAX_QUESTION_CHARS
    );
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
        .schemas(crate::intent::Intent::Plan, crate::profile::Profile::Code)
        .iter()
        .any(|s| s.name == "Decision"));
}

#[tokio::test]
async fn invalid_choices_are_refused_before_the_model_is_asked() {
    let mock = Arc::new(MockProvider::text("unused"));
    let dir = TempDir::new();
    let (ctx, _ws) = ctx_for(&dir);
    for choices in [
        json!({}),
        json!({"only": "one option"}),
        json!({"": "empty name", "other": "another option"}),
        json!({"a": " ", "b": "another option"}),
        json!({"a": 1, "b": "another option"}),
        json!(["a", "b"]),
    ] {
        assert!(tool(mock.clone())
            .run(
                &ctx,
                json!({
                    "question": "Which option?", "choices": choices,
                })
            )
            .await
            .is_err());
    }
    assert!(mock.seen.lock().unwrap().is_empty());
}

#[test]
fn an_invalid_choice_distribution_cannot_be_presented_as_advice() {
    let question = Question::parse(&json!({
        "question": "Which skill?",
        "choices": {"debugging": "Investigate failures", "design": "Design a layout"},
    }))
    .unwrap();
    for answer in [
        json!({"choice": "unknown", "probabilities": {"debugging": 0.9, "design": 0.1}}),
        json!({"choice": "debugging", "probabilities": {"debugging": 0.9}}),
        json!({"choice": "debugging", "probabilities": {"debugging": 0.0, "design": 0.0}}),
        json!({"choice": "debugging", "probabilities": {"debugging": 0.1, "design": 0.9}}),
        json!({"choice": "debugging", "probabilities": {"debugging": 1.1, "design": -0.1}}),
        json!({"choice": "debugging", "probabilities": {"debugging": "0.9", "design": 0.1}}),
    ] {
        assert!(question
            .read_answer(&json!({"answers": {"q": answer}}))
            .is_err());
    }
}

#[tokio::test]
async fn a_pending_typed_request_can_be_cancelled_or_time_out_without_chat_retry() {
    use crate::provider::registry::{Protocol, ProviderSpec};

    for cancel in [true, false] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let chat = Arc::new(MockProvider::text("must not be asked"));
        let setup = DecisionSetup {
            provider: chat.clone(),
            model: "jev".into(),
            systemone: Some(Arc::new(SystemOne::new(
                ProviderSpec {
                    id: "decision".into(),
                    display_name: "Decision".into(),
                    protocol: Protocol::OpenAi,
                    base_url: format!("http://{}", listener.local_addr().unwrap()),
                    headers: Default::default(),
                    models: Vec::new(),
                },
                None,
            ))),
        };
        let tool = Decision::new(setup).with_timeout(Duration::from_millis(50));
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let input = json!({"question":"Is this a test?"});
        let peer = async {
            let (_socket, _) = listener.accept().await.unwrap();
            if cancel {
                ctx.cancel.cancel();
            }
            // Keep the connection open beyond the tool's deadline.
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let (result, ()) = tokio::join!(tool.run(&ctx, input), peer);
        let err = result.unwrap_err();
        if cancel {
            assert!(matches!(err, ToolError::Cancelled));
        } else {
            assert!(err.for_model().contains("in time"), "{err}");
        }
        assert!(chat.seen.lock().unwrap().is_empty());
    }
}

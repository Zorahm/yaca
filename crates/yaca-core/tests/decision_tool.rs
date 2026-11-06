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
use yaca_core::provider::registry::{Protocol, ProviderSpec};
use yaca_core::provider::systemone::SystemOne;
use yaca_core::tool::decision::DecisionSetup;
use yaca_core::{Mode, SessionConfig};

use common::wire::{json_response, serve};
use common::{ask, calls, is_turn_over, tool_result_of, until, TempProject};

fn typed_setup(url: String, chat: Arc<MockProvider>) -> DecisionSetup {
    DecisionSetup {
        provider: chat,
        model: "jev-style".into(),
        systemone: Some(Arc::new(SystemOne::new(
            ProviderSpec {
                id: "decision".into(),
                display_name: "Decision".into(),
                protocol: Protocol::OpenAi,
                base_url: url,
                headers: Default::default(),
                models: Vec::new(),
            },
            Some("test-key".into()),
        ))),
    }
}

fn agent_calling(input: serde_json::Value) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("c1", "Decision", input)])),
        Turn::Events(script_text("Done.")),
    ]))
}

fn agent_asking(question: &str) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "c1",
            "Decision",
            json!({
                "question": question,
                "description": "Choosing a skill",
            }),
        )])),
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

#[tokio::test]
async fn a_decision_server_gets_typed_choices_instead_of_chat_completions() {
    let (url, seen) = serve(json_response(
        "200 OK",
        r#"{
        "answers":{"q":{"type":"choice","choice":"debugging",
            "probabilities":{"debugging":0.9,"design":0.1},"confidence":0.8}},
        "usage":{"input_tokens":42,"output_tokens":0}
    }"#,
    ))
    .await;
    let chat = Arc::new(MockProvider::text("must not be asked"));
    let agent = agent_calling(json!({
        "description": "Choosing a skill",
        "question": "Which skill fits?",
        "state": "A test fails with HTTP 405.",
        "choices": {"debugging": "Investigate the failing test", "design": "Design a layout"},
    }));
    let dir = TempProject::new();
    let handle = start(&dir, agent.clone(), Some(typed_setup(url, chat.clone())));
    let mut rx = handle.subscribe();
    ask(&handle, "test the decision tool").await;
    let events = until(&mut rx, is_turn_over).await;

    let tokens = events
        .iter()
        .rev()
        .find_map(|event| match event {
            yaca_core::AgentEvent::UsageUpdated { usage } => Some(usage.total),
            _ => None,
        })
        .unwrap();
    // The final scripted agent reply uses 100 input and 20 output tokens.
    assert_eq!(tokens.input, 142);
    assert_eq!(tokens.output, 20);

    let request = seen.await.unwrap();
    assert!(
        request.headers.starts_with("POST /v1/systemone "),
        "{}",
        request.headers
    );
    assert!(request.has_header("authorization", "Bearer test-key"));
    assert!(request.has_header("accept", "application/json"));
    assert_eq!(
        request.body,
        json!({
            "model": "jev-style", "state": "A test fails with HTTP 405.",
            "questions": {"q": {"type": "choice", "instructions": "Which skill fits?",
                "criteria": {"debugging": "Investigate the failing test", "design": "Design a layout"}}},
        })
    );
    let (text, is_error) = tool_result_of(&agent, "c1");
    assert!(!is_error, "{text}");
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(answer["choice"], "debugging");
    assert_eq!(answer["probabilities"]["debugging"], 0.9);
    assert!(chat.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_yes_no_question_returns_the_probability_without_inventing_prose() {
    let (url, seen) = serve(json_response(
        "200 OK",
        r#"{"answers":{"q":{"type":"noul","noul":0.97}}}"#,
    ))
    .await;
    let chat = Arc::new(MockProvider::text("must not be asked"));
    let agent = agent_asking("Is Rust a programming language?");
    let dir = TempProject::new();
    let handle = start(&dir, agent.clone(), Some(typed_setup(url, chat.clone())));
    let mut rx = handle.subscribe();
    ask(&handle, "test").await;
    until(&mut rx, is_turn_over).await;

    let request = seen.await.unwrap();
    assert_eq!(request.body["state"], "Is Rust a programming language?");
    assert_eq!(request.body["questions"]["q"]["type"], "noul");
    let (text, is_error) = tool_result_of(&agent, "c1");
    assert!(!is_error, "{text}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        json!({"type":"noul","noul":0.97})
    );
    assert!(chat.seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_server_without_the_decision_route_falls_back_to_chat_and_remembers_it() {
    for status in [
        "404 Not Found",
        "405 Method Not Allowed",
        "501 Not Implemented",
    ] {
        let (url, seen) = serve(json_response(status, r#"{"error":"no decision route"}"#)).await;
        let chat = Arc::new(MockProvider::new(vec![
            Turn::Events(script_text("debugging")),
            Turn::Events(script_text("debugging")),
        ]));
        let agent = Arc::new(MockProvider::new(vec![
            Turn::Events(calls(&[(
                "c1",
                "Decision",
                json!({
                    "description":"Choosing", "question":"Which skill?", "state":"A test fails.",
                    "choices":{"debugging":"Investigate failures","design":"Design layouts"},
                }),
            )])),
            Turn::Events(calls(&[(
                "c2",
                "Decision",
                json!({
                    "description":"Checking", "question":"Is debugging appropriate?",
                }),
            )])),
            Turn::Events(script_text("Done.")),
        ]));
        let setup = typed_setup(url, chat.clone());
        let client = setup.systemone.clone().unwrap();
        let dir = TempProject::new();
        let handle = start(&dir, agent.clone(), Some(setup));
        let mut rx = handle.subscribe();
        ask(&handle, "test").await;
        until(&mut rx, is_turn_over).await;

        seen.await.unwrap();
        assert!(client.is_absent());
        let (text, is_error) = tool_result_of(&agent, "c2");
        assert!(!is_error, "{status}: {text}");
        assert_eq!(text, "debugging");
        let first_result = agent.seen.lock().unwrap()[1].clone();
        assert!(first_result
            .messages
            .iter()
            .flat_map(|message| &message.content)
            .any(|block| {
                matches!(block, yaca_core::message::ContentBlock::ToolResult {
                tool_use_id, content, is_error: false,
            } if tool_use_id.as_str() == "c1" && content == "debugging")
            }));
        let requests = chat.seen.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let prompt = format!("{:?}", requests[0].messages);
        for text in [
            "Which skill?",
            "A test fails.",
            "Investigate failures",
            "Design layouts",
        ] {
            assert!(prompt.contains(text), "{prompt}");
        }
    }
}

#[tokio::test]
async fn a_failing_or_unreadable_decision_response_does_not_retry_in_chat() {
    for (status, body) in [
        ("401 Unauthorized", r#"{"error":"invalid key"}"#),
        ("403 Forbidden", r#"{"error":"forbidden"}"#),
        ("400 Bad Request", r#"{"error":"invalid model"}"#),
        (
            "422 Unprocessable Entity",
            r#"{"error":"invalid question"}"#,
        ),
        ("500 Internal Server Error", r#"{"error":"scoring failed"}"#),
        ("200 OK", "not JSON"),
        ("200 OK", r#"{"answers":{}}"#),
        ("200 OK", r#"{"answers":{"q":{"noul":1.2}}}"#),
    ] {
        let (url, seen) = serve(json_response(status, body)).await;
        let chat = Arc::new(MockProvider::text("must not be asked"));
        let agent = agent_asking("Is Rust a programming language?");
        let dir = TempProject::new();
        let handle = start(&dir, agent.clone(), Some(typed_setup(url, chat.clone())));
        let mut rx = handle.subscribe();
        ask(&handle, "test").await;
        until(&mut rx, is_turn_over).await;

        seen.await.unwrap();
        let (text, is_error) = tool_result_of(&agent, "c1");
        assert!(is_error, "{status}: {body}: {text}");
        assert!(text.contains("Decide it yourself"), "{text}");
        assert!(chat.seen.lock().unwrap().is_empty());
        assert_eq!(agent.seen.lock().unwrap().len(), 2, "the agent continued");
    }
}

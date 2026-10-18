//! The router's behaviour, with a mock behind each provider id. Every test
//! reads what each backend received, so a wrong route shows as a wrong request.

use std::collections::BTreeMap;
use std::sync::Arc;

use futures_util::StreamExt;

use super::*;
use crate::error::ProviderError;
use crate::intent::Effort;
use crate::message::{ContentBlock, Message};
use crate::provider::mock::{script_text, MockProvider, Turn};
use crate::provider::registry::Protocol;
use crate::router::config::{
    Condition, Decision, DecisionModel, ModelRef, OnUnknown, Question, Rule, Signal,
};
use crate::usage::{ContextSource, Pricing};

const CODE_YES: &str = r#"{"code": 0.9}"#;
const CODE_NO: &str = r#"{"code": 0.1}"#;

fn info(id: &str, ctx: u32, out: u32, effort: bool, thinking: bool) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        provider_id: String::new(),
        display_name: id.into(),
        context_max: ctx,
        max_output: out,
        pricing: Pricing {
            input: 1.0,
            output: 2.0,
        },
        supports_effort: effort,
        supports_thinking: thinking,
        context_source: ContextSource::ProviderReported,
        supports_images: true,
    }
}

fn spec(id: &str, model: ModelInfo) -> ProviderSpec {
    ProviderSpec {
        id: id.into(),
        display_name: id.into(),
        protocol: Protocol::OpenAi,
        base_url: format!("https://{id}.example.com"),
        headers: BTreeMap::new(),
        models: vec![model],
    }
}

/// The decision model, a small target without effort or thinking, and a big
/// target with both. The default is the small one.
fn providers() -> Providers {
    Providers {
        list: vec![
            spec("decider", info("judge", 8_000, 1_000, false, false)),
            spec("small", info("tiny", 8_000, 1_000, false, false)),
            spec("big", info("large", 200_000, 8_000, true, true)),
        ],
    }
}

fn model(provider: &str, model: &str) -> ModelRef {
    ModelRef {
        provider: provider.into(),
        model: model.into(),
    }
}

fn code_signal() -> Signal {
    Signal {
        name: "code".into(),
        question: Question::Noul {
            instructions: "Is the request about code?".into(),
        },
    }
}

fn code_route() -> Decision {
    Decision {
        name: "code".into(),
        priority: 10,
        rules: Rule::Signal(Condition {
            name: "code".into(),
            label: None,
            predicate: None,
        }),
        on_unknown: OnUnknown::NoMatch,
        model: model("big", "large"),
    }
}

fn config() -> RoutingConfig {
    RoutingConfig {
        name: "auto".into(),
        decision_model: DecisionModel {
            model: model("decider", "judge"),
            timeout_ms: 1000,
        },
        default: model("small", "tiny"),
        signals: vec![code_signal()],
        decisions: vec![code_route()],
    }
}

/// The mocks behind the provider ids, kept so a test can read what they saw.
struct Rig {
    decider: Arc<MockProvider>,
    small: Arc<MockProvider>,
    big: Arc<MockProvider>,
}

impl Rig {
    fn new(decider: Vec<Turn>, small: Vec<Turn>, big: Vec<Turn>) -> Self {
        Rig {
            decider: Arc::new(MockProvider::new(decider)),
            small: Arc::new(MockProvider::new(small)),
            big: Arc::new(MockProvider::new(big)),
        }
    }

    fn build(&self, spec: &ProviderSpec) -> Arc<dyn Provider> {
        let mock = match spec.id.as_str() {
            "decider" => &self.decider,
            "small" => &self.small,
            _ => &self.big,
        };
        Arc::clone(mock) as Arc<dyn Provider>
    }

    fn router_for(&self, cfg: RoutingConfig) -> (RouterProvider, ModelInfo) {
        RouterProvider::new(cfg, &providers(), |spec| self.build(spec)).unwrap()
    }

    fn router(&self) -> (RouterProvider, ModelInfo) {
        self.router_for(config())
    }
}

fn judge(answer: &str) -> Turn {
    Turn::Events(script_text(answer))
}

fn request(messages: Vec<Message>) -> ProviderRequest {
    ProviderRequest {
        model: "auto".into(),
        system: "system".into(),
        messages,
        tools: vec![],
        max_tokens: 32_000,
        effort: Some(Effort::High),
        thinking: true,
    }
}

fn user(text: &str) -> Message {
    Message::user_text(text)
}

fn said(text: &str) -> Message {
    Message::assistant(vec![ContentBlock::Text { text: text.into() }])
}

fn thought_then(text: &str) -> Message {
    Message::assistant(vec![
        ContentBlock::Thinking {
            thinking: "hm".into(),
            signature: Some("sig".into()),
        },
        ContentBlock::Text { text: text.into() },
    ])
}

async fn collect(router: &RouterProvider, req: ProviderRequest) -> Vec<ProviderEvent> {
    let stream = router.stream(req).await.unwrap();
    stream.map(|event| event.unwrap()).collect().await
}

fn seen(mock: &MockProvider) -> Vec<ProviderRequest> {
    mock.seen.lock().unwrap().clone()
}

fn routed(route: Option<&str>, provider: &str, model: &str, matched: &[&str]) -> ProviderEvent {
    ProviderEvent::Routed {
        route: route.map(String::from),
        provider: provider.into(),
        model: model.into(),
        matched: matched.iter().map(|s| s.to_string()).collect(),
    }
}

#[tokio::test]
async fn a_matched_decision_routes_to_its_target() {
    let rig = Rig::new(
        vec![judge(CODE_YES)],
        vec![],
        vec![Turn::Events(script_text("big answer"))],
    );
    let (router, _) = rig.router();

    let events = collect(&router, request(vec![user("write a parser")])).await;

    assert_eq!(events[0], routed(Some("code"), "big", "large", &["code"]));
    assert_eq!(events[1..], script_text("big answer")[..]);
    let forwarded = seen(&rig.big);
    assert_eq!(forwarded.len(), 1);
    assert_eq!(forwarded[0].model, "large");
    assert!(seen(&rig.small).is_empty());
    // The decision model was asked about the message itself.
    let asked = seen(&rig.decider);
    assert_eq!(asked.len(), 1);
    assert!(matches!(
        &asked[0].messages[0].content[..],
        [ContentBlock::Text { text }] if text.contains("write a parser")
    ));
}

#[tokio::test]
async fn nothing_matched_goes_to_the_default_without_a_route() {
    let rig = Rig::new(
        vec![judge(CODE_NO)],
        vec![Turn::Events(script_text("small"))],
        vec![],
    );
    let (router, _) = rig.router();

    let events = collect(&router, request(vec![user("hello")])).await;

    assert_eq!(events[0], routed(None, "small", "tiny", &[]));
    assert_eq!(seen(&rig.small)[0].model, "tiny");
    assert!(seen(&rig.big).is_empty());
}

#[tokio::test]
async fn a_failing_decision_model_gives_the_default() {
    let rig = Rig::new(
        vec![Turn::Fail(ProviderError::Network("down".into()))],
        vec![Turn::Events(script_text("small"))],
        vec![],
    );
    let (router, _) = rig.router();

    let events = collect(&router, request(vec![user("write a parser")])).await;

    assert_eq!(events[0], routed(None, "small", "tiny", &[]));
}

#[tokio::test]
async fn a_turn_asks_the_decision_model_once_and_keeps_its_target() {
    // One judgement is scripted. A second one would get a refusal and fall to
    // the default, so the second request going to `big` also shows the reuse.
    let rig = Rig::new(
        vec![judge(CODE_YES)],
        vec![],
        vec![
            Turn::Events(script_text("a")),
            Turn::Events(script_text("b")),
        ],
    );
    let (router, _) = rig.router();

    let first = request(vec![user("write a parser")]);
    let calls = Message::assistant(vec![ContentBlock::ToolUse {
        id: "c1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    }]);
    let results = Message::tool_results(vec![ContentBlock::ToolResult {
        tool_use_id: "c1".into(),
        content: "ok".into(),
        is_error: false,
    }]);
    let second = request(vec![user("write a parser"), calls, results]);

    let one = collect(&router, first).await;
    let two = collect(&router, second.clone()).await;

    assert_eq!(one[0], routed(Some("code"), "big", "large", &["code"]));
    assert_eq!(two[0], one[0], "the same turn shows the same route");
    assert_eq!(seen(&rig.decider).len(), 1);
    let forwarded = seen(&rig.big);
    assert_eq!(forwarded.len(), 2);
    assert_eq!(
        forwarded[1].messages, second.messages,
        "nothing stripped within a turn"
    );
}

#[tokio::test]
async fn a_new_user_message_asks_again() {
    let rig = Rig::new(
        vec![judge(CODE_YES), judge(CODE_NO)],
        vec![Turn::Events(script_text("small"))],
        vec![Turn::Events(script_text("big"))],
    );
    let (router, _) = rig.router();

    let first = collect(&router, request(vec![user("write a parser")])).await;
    let second = collect(
        &router,
        request(vec![user("write a parser"), said("big"), user("thanks")]),
    )
    .await;

    assert_eq!(first[0], routed(Some("code"), "big", "large", &["code"]));
    assert_eq!(second[0], routed(None, "small", "tiny", &[]));
    assert_eq!(seen(&rig.decider).len(), 2);
}

#[tokio::test]
async fn effort_and_thinking_follow_the_target() {
    let rig = Rig::new(
        vec![judge(CODE_NO), judge(CODE_YES)],
        vec![Turn::Events(script_text("small"))],
        vec![Turn::Events(script_text("big"))],
    );
    let (router, _) = rig.router();

    collect(&router, request(vec![user("hello")])).await;
    collect(
        &router,
        request(vec![user("hello"), said("hi"), user("write a parser")]),
    )
    .await;

    // The default: neither is supported, and output is capped at its limit.
    let small = &seen(&rig.small)[0];
    assert_eq!(small.effort, None);
    assert!(!small.thinking);
    assert_eq!(small.max_tokens, 1_000);
    // The routed target: both are kept, and output is capped at its own limit.
    let big = &seen(&rig.big)[0];
    assert_eq!(big.effort, Some(Effort::High));
    assert!(big.thinking);
    assert_eq!(big.max_tokens, 8_000);
}

#[tokio::test]
async fn a_foreign_model_id_is_refused() {
    let rig = Rig::new(vec![], vec![], vec![]);
    let (router, _) = rig.router();

    let mut req = request(vec![user("hello")]);
    req.model = "gpt-9".into();
    let Err(err) = router.stream(req).await else {
        panic!("a foreign model id must be refused");
    };

    let ProviderError::Config(message) = err else {
        panic!("expected a config error, got {err:?}");
    };
    assert!(message.contains("gpt-9"), "{message}");
    assert!(seen(&rig.decider).is_empty());
    assert!(seen(&rig.small).is_empty());
}

#[tokio::test]
async fn thinking_is_dropped_when_the_target_changes() {
    let rig = Rig::new(
        vec![judge(CODE_NO), judge(CODE_YES)],
        vec![Turn::Events(script_text("small"))],
        vec![Turn::Events(script_text("big"))],
    );
    let (router, _) = rig.router();

    collect(&router, request(vec![user("hello")])).await;
    collect(
        &router,
        request(vec![
            user("hello"),
            thought_then("hi"),
            user("write a parser"),
        ]),
    )
    .await;

    // The signature was made by the small model; the big one must not see it.
    let sent = &seen(&rig.big)[0].messages;
    assert!(!sent
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::Thinking { .. })));
    assert!(sent
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::Text { text } if text == "hi")));
}

#[tokio::test]
async fn thinking_is_kept_when_the_target_stays() {
    let rig = Rig::new(
        vec![judge(CODE_NO), judge(CODE_NO)],
        vec![
            Turn::Events(script_text("one")),
            Turn::Events(script_text("two")),
        ],
        vec![],
    );
    let (router, _) = rig.router();

    collect(&router, request(vec![user("hello")])).await;
    collect(
        &router,
        request(vec![user("hello"), thought_then("hi"), user("again")]),
    )
    .await;

    let sent = &seen(&rig.small)[1].messages;
    assert!(sent
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::Thinking { .. })));
}

#[tokio::test]
async fn a_resumed_conversation_drops_thinking_it_cannot_attribute() {
    // A fresh router has no record of which model made the reasoning in a
    // conversation it did not route, so the reasoning does not go out unchecked.
    let rig = Rig::new(
        vec![judge(CODE_NO)],
        vec![Turn::Events(script_text("ok"))],
        vec![],
    );
    let (router, _) = rig.router();

    collect(
        &router,
        request(vec![user("hello"), thought_then("hi"), user("again")]),
    )
    .await;

    let sent = &seen(&rig.small)[0].messages;
    assert!(!sent
        .iter()
        .flat_map(|m| &m.content)
        .any(|b| matches!(b, ContentBlock::Thinking { .. })));
}

#[tokio::test]
async fn no_user_text_uses_the_default_without_asking() {
    let rig = Rig::new(vec![], vec![Turn::Events(script_text("ok"))], vec![]);
    let (router, _) = rig.router();

    let events = collect(&router, request(vec![said("earlier")])).await;

    assert_eq!(events[0], routed(None, "small", "tiny", &[]));
    assert!(seen(&rig.decider).is_empty());
}

#[tokio::test]
async fn the_decision_memory_stays_bounded() {
    let turns = (0..MAX_CONVERSATIONS + 1).map(|_| judge(CODE_NO)).collect();
    let rig = Rig::new(
        turns,
        (0..MAX_CONVERSATIONS + 1)
            .map(|_| Turn::Events(script_text("ok")))
            .collect(),
        vec![],
    );
    let (router, _) = rig.router();

    for n in 0..=MAX_CONVERSATIONS {
        collect(&router, request(vec![user(&format!("conversation {n}"))])).await;
    }

    assert!(router.sticky.lock().unwrap().len() <= MAX_CONVERSATIONS);
}

#[tokio::test]
async fn an_unknown_provider_in_the_config_is_refused() {
    let rig = Rig::new(vec![], vec![], vec![]);
    let mut cfg = config();
    cfg.default = model("elsewhere", "x");

    assert!(RouterProvider::new(cfg, &providers(), |spec| rig.build(spec)).is_err());
}

#[tokio::test]
async fn the_offered_model_aggregates_its_targets() {
    let rig = Rig::new(vec![], vec![], vec![]);
    let mut providers = providers();
    // The small default is the smallest window and the one whose guess it is;
    // only the big one sees images.
    providers.list[1].models[0] = ModelInfo {
        pricing: Pricing {
            input: 0.5,
            output: 0.75,
        },
        context_source: ContextSource::DefaultGuess,
        supports_images: false,
        ..info("tiny", 8_000, 1_000, false, false)
    };
    let (router, offered) =
        RouterProvider::new(config(), &providers, |spec| rig.build(spec)).unwrap();

    assert_eq!(offered.id, "auto");
    assert_eq!(offered.provider_id, ROUTER_ID);
    assert_eq!(offered.context_max, 8_000);
    assert_eq!(offered.context_source, ContextSource::DefaultGuess);
    assert_eq!(offered.max_output, 1_000);
    assert!(offered.supports_effort, "the big target supports effort");
    assert!(
        offered.supports_thinking,
        "the big target supports thinking"
    );
    assert!(offered.supports_images, "one capable target is enough");
    assert_eq!(
        offered.pricing,
        Pricing {
            input: 0.5,
            output: 0.75
        },
        "priced by the default"
    );

    let targets = router.targets();
    assert_eq!(
        targets
            .iter()
            .map(|t| (t.provider_id.as_str(), t.id.as_str()))
            .collect::<Vec<_>>(),
        [("small", "tiny"), ("big", "large")]
    );
}

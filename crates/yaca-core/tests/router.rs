//! A session on the router: one message is one decision, and the model the
//! rules chose answers it.
//!
//! The session is given the router as its provider and the virtual model as
//! its model, the way the window and the CLI set it up. Only the public surface
//! is used, so what passes here is what the interface can rely on.

mod common;

use std::sync::Arc;

use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::registry::{Protocol, ProviderSpec, Providers};
use yaca_core::provider::Provider;
use yaca_core::router::config::{
    Condition, Decision, DecisionModel, ModelRef, OnUnknown, Question, RoutingConfig, Rule, Signal,
};
use yaca_core::router::provider::RouterProvider;
use yaca_core::router::ROUTER_ID;
use yaca_core::usage::{ContextSource, ModelInfo, Pricing};
use yaca_core::{AgentEvent, Mode, SessionConfig};

use common::{ask, is_turn_over, until, TempProject};

fn info(id: &str, ctx: u32, out: u32, effort: bool) -> ModelInfo {
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
        supports_thinking: effort,
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
        headers: Default::default(),
        models: vec![model],
    }
}

fn providers() -> Providers {
    Providers {
        list: vec![
            spec("decider", info("judge", 8_000, 1_000, false)),
            spec("small", info("tiny", 8_000, 1_000, false)),
            spec("big", info("large", 200_000, 8_000, true)),
        ],
    }
}

fn config() -> RoutingConfig {
    let model = |provider: &str, model: &str| ModelRef {
        provider: provider.into(),
        model: model.into(),
    };
    RoutingConfig {
        name: "auto".into(),
        decision_model: DecisionModel {
            model: model("decider", "judge"),
            timeout_ms: 1000,
        },
        default: model("small", "tiny"),
        signals: vec![Signal {
            name: "code".into(),
            question: Question::Noul {
                instructions: "Is the request about code?".into(),
            },
        }],
        decisions: vec![Decision {
            name: "code".into(),
            priority: 10,
            rules: Rule::Signal(Condition {
                name: "code".into(),
                label: None,
                predicate: None,
            }),
            on_unknown: OnUnknown::NoMatch,
            model: model("big", "large"),
        }],
    }
}

#[tokio::test]
async fn one_message_is_one_decision_and_the_chosen_model_answers() {
    let dir = TempProject::new();
    let decider = Arc::new(MockProvider::new(vec![Turn::Events(script_text(
        r#"{"code": 0.9}"#,
    ))]));
    let big = Arc::new(MockProvider::text("routed answer"));
    let small = Arc::new(MockProvider::text("default answer"));
    let backend = |spec: &ProviderSpec| -> Arc<dyn Provider> {
        match spec.id.as_str() {
            "decider" => decider.clone(),
            "small" => small.clone(),
            _ => big.clone(),
        }
    };
    let (router, offered) = RouterProvider::new(config(), &providers(), backend).unwrap();

    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    cfg.provider = ROUTER_ID.into();
    cfg.model = offered.id.clone();
    cfg.models = std::iter::once(offered).chain(router.targets()).collect();
    let handle = yaca_core::spawn(cfg, Arc::new(router)).unwrap();

    let mut rx = handle.subscribe();
    ask(&handle, "write a parser").await;
    let seen = until(&mut rx, is_turn_over).await;

    let decided: Vec<&AgentEvent> = seen
        .iter()
        .filter(|e| matches!(e, AgentEvent::RouteDecided { .. }))
        .collect();
    assert_eq!(decided.len(), 1, "one message, one decision: {seen:?}");
    assert_eq!(
        *decided[0],
        AgentEvent::RouteDecided {
            route: Some("code".into()),
            provider: "big".into(),
            model: "large".into(),
            matched: vec!["code".into()],
        }
    );

    let answered = big.seen.lock().unwrap();
    assert_eq!(answered.len(), 1);
    assert_eq!(answered[0].model, "large");
    assert!(small.seen.lock().unwrap().is_empty());
}

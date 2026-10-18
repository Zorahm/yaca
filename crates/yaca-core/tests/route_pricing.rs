//! A routed request is priced by the model that answered, and the feed shows a
//! route once per user turn rather than once per request.
//!
//! The session knows only the virtual model a router presents, so the prices
//! of the models it routes to have to come from the list the session was
//! given, matched by id and provider.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::usage::{ContextSource, ModelInfo, Pricing};
use yaca_core::{AgentEvent, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn priced(id: &str, provider: &str, input: f64, output: f64) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        provider_id: provider.into(),
        display_name: id.into(),
        context_max: 100_000,
        max_output: 8_000,
        pricing: Pricing { input, output },
        supports_effort: false,
        supports_thinking: false,
        context_source: ContextSource::ProviderReported,
        supports_images: true,
    }
}

/// A session whose configured model is `model`, with `models` as its list.
fn routed_session(
    dir: &TempProject,
    provider: Arc<MockProvider>,
    model: &str,
    models: Vec<ModelInfo>,
) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = None;
    cfg.model = model.into();
    cfg.models = models;
    yaca_core::spawn(cfg, provider).unwrap()
}

fn routed_to(provider: &str, model: &str) -> ProviderEvent {
    ProviderEvent::Routed {
        route: Some("code".into()),
        provider: provider.into(),
        model: model.into(),
        matched: vec!["keyword:refactor".into()],
    }
}

/// One request, with its route ahead of everything else, as a router sends it.
fn routed(mut events: Vec<ProviderEvent>) -> Turn {
    events.insert(0, routed_to("local", "qwen-coder"));
    Turn::Events(events)
}

fn decisions(seen: &[AgentEvent]) -> usize {
    seen.iter()
        .filter(|e| matches!(e, AgentEvent::RouteDecided { .. }))
        .count()
}

#[tokio::test]
async fn a_routed_request_is_priced_by_the_model_that_answered() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![routed(script_text("Done."))]));
    let models = vec![
        // The virtual model the session was configured with: cheap.
        priced("router", "gateway", 1.0, 1.0),
        // The model the request went to, under its own provider: dear.
        priced("qwen-coder", "local", 10.0, 20.0),
    ];
    let handle = routed_session(&dir, provider, "router", models);

    let mut rx = handle.subscribe();
    ask(&handle, "refactor the parser").await;
    let seen = until(&mut rx, is_turn_over).await;

    let cost = seen
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::UsageUpdated { usage } => Some(usage.cost_usd),
            _ => None,
        })
        .expect("no usage reported");
    // script_text answers with 100 input and 20 output tokens, at the routed
    // model's price. At the virtual model's price it would be 0.00012.
    let expected = 100.0 * 10.0 / 1_000_000.0 + 20.0 * 20.0 / 1_000_000.0;
    assert!(
        (cost - expected).abs() < 1e-12,
        "priced as {cost}, expected {expected}"
    );
}

#[tokio::test]
async fn a_turn_of_two_requests_shows_its_route_once() {
    let dir = TempProject::new();
    dir.write("a.rs", "fn main() {}\n");
    let provider = Arc::new(MockProvider::new(vec![
        routed(calls(&[("c1", "Read", json!({ "path": "a.rs" }))])),
        routed(script_text("Read it.")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);

    let mut rx = handle.subscribe();
    ask(&handle, "read a.rs").await;
    let seen = until(&mut rx, is_turn_over).await;

    // Both requests really happened, both carried a route, and the feed has
    // one chip for the turn.
    assert_eq!(provider.seen.lock().unwrap().len(), 2);
    assert_eq!(decisions(&seen), 1, "{seen:?}");
}

#[tokio::test]
async fn the_next_message_shows_its_route_again() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        routed(script_text("First answer.")),
        routed(script_text("Second answer.")),
    ]));
    let handle = session(&dir, provider, Mode::Manual);

    let mut rx = handle.subscribe();
    ask(&handle, "first question").await;
    let first = until(&mut rx, is_turn_over).await;
    ask(&handle, "second question").await;
    let second = until(&mut rx, is_turn_over).await;

    assert_eq!(decisions(&first), 1, "{first:?}");
    // The same route as the turn before: shown again, because it is a new turn.
    assert_eq!(decisions(&second), 1, "{second:?}");
}

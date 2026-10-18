//! A routed request says which model answered, ahead of what that model wrote.
//!
//! A routing provider emits `Routed` before the request's `MessageStart`. The
//! session turns it into one `RouteDecided` in the feed, so the human sees the
//! model before the answer it gave.

mod common;

use std::sync::Arc;

use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::{AgentEvent, Mode};

use common::{ask, is_turn_over, session, until, TempProject};

#[tokio::test]
async fn the_routed_model_is_shown_once_and_before_the_first_text() {
    let dir = TempProject::new();
    let mut events = vec![ProviderEvent::Routed {
        route: Some("code".into()),
        provider: "local".into(),
        model: "qwen-coder".into(),
        matched: vec!["keyword:refactor".into()],
    }];
    events.extend(script_text("Done."));
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(events)]));
    let handle = session(&dir, provider, Mode::Manual);

    let mut rx = handle.subscribe();
    ask(&handle, "refactor the parser").await;
    let seen = until(&mut rx, is_turn_over).await;

    let decided: Vec<usize> = seen
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, AgentEvent::RouteDecided { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(decided.len(), 1, "one request, one decision: {seen:?}");

    assert_eq!(
        seen[decided[0]],
        AgentEvent::RouteDecided {
            route: Some("code".into()),
            provider: "local".into(),
            model: "qwen-coder".into(),
            matched: vec!["keyword:refactor".into()],
        }
    );

    let first_text = seen
        .iter()
        .position(|e| matches!(e, AgentEvent::TextDelta { .. }))
        .expect("the answer has text");
    assert!(
        decided[0] < first_text,
        "the choice of model comes before what the model said"
    );
}

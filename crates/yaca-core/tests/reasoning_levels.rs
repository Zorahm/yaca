//! The public session surface must agree with what the provider receives.

mod common;

use std::sync::Arc;

use common::{ask, is_turn_over, until, TempProject};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::reasoning::ReasoningCapabilities;
use yaca_core::usage::unknown_model;
use yaca_core::{spawn, AgentEvent, Command, Effort, SessionConfig};

#[tokio::test]
async fn an_old_saved_pick_is_normalized_before_the_first_snapshot() {
    let dir = TempProject::new();
    let mut cfg = SessionConfig::new(dir.path());
    cfg.model = "tag-only-model".into();
    let mut model = unknown_model(&cfg.model);
    model.supports_effort = true;
    cfg.models = vec![model];
    cfg.effort = Effort::Max;
    let provider = Arc::new(MockProvider::text("default"));
    let handle = spawn(cfg, provider.clone()).unwrap();
    assert_eq!(handle.state().effort, Effort::Auto);
    let mut rx = handle.subscribe();
    ask(&handle, "use provider defaults").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap()[0].effort, None);
}

#[tokio::test]
async fn changing_model_and_setting_an_unsupported_level_reports_provider_defaults() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("default")),
        Turn::Events(script_text("low")),
        Turn::Events(script_text("default again")),
    ]));
    let mut cfg = SessionConfig::new(dir.path());
    let mut gemini = unknown_model("gemini");
    gemini.supports_effort = true;
    gemini.reasoning = Some(ReasoningCapabilities {
        supported_efforts: Some(vec![Effort::Low, Effort::Medium, Effort::High]),
        default_effort: Some(Effort::Medium),
        mandatory: true,
        ..Default::default()
    });
    cfg.models.push(gemini);
    cfg.effort = Effort::Max;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    handle
        .send(Command::SetModel {
            model: "gemini".into(),
        })
        .await
        .unwrap();
    let events = until(
        &mut rx,
        |e| matches!(e, AgentEvent::StateChanged { state } if state.model == "gemini"),
    )
    .await;
    assert!(
        matches!(events.last(), Some(AgentEvent::StateChanged { state }) if state.effort == Effort::Auto)
    );
    ask(&handle, "use the provider default").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap()[0].effort, None);

    handle
        .send(Command::SetEffort {
            effort: Effort::Low,
        })
        .await
        .unwrap();
    until(
        &mut rx,
        |e| matches!(e, AgentEvent::StateChanged { state } if state.effort == Effort::Low),
    )
    .await;
    ask(&handle, "use low effort").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap()[1].effort, Some(Effort::Low));

    handle
        .send(Command::SetEffort {
            effort: Effort::None,
        })
        .await
        .unwrap();
    until(
        &mut rx,
        |e| matches!(e, AgentEvent::StateChanged { state } if state.effort == Effort::Auto),
    )
    .await;
    ask(&handle, "reasoning cannot be disabled").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap()[2].effort, None);
}

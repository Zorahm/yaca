use std::collections::BTreeMap;

use super::*;
use crate::provider::registry::{Protocol, ProviderSpec};
use crate::router::config::{DecisionModel, ModelRef, RoutingConfig};
use crate::testutil::TempDir;
use crate::usage::unknown_model;

fn providers() -> Providers {
    Providers {
        list: vec![ProviderSpec {
            id: "gateway".into(),
            display_name: "Gateway".into(),
            protocol: Protocol::OpenAi,
            base_url: "https://gate.example.com".into(),
            headers: BTreeMap::new(),
            models: vec![unknown_model("fast"), unknown_model("big")],
        }],
    }
}

fn model(id: &str) -> ModelRef {
    ModelRef {
        provider: "gateway".into(),
        model: id.into(),
    }
}

fn config() -> RoutingConfig {
    RoutingConfig {
        name: "auto".into(),
        decision_model: DecisionModel {
            model: model("fast"),
            timeout_ms: 1000,
        },
        default: model("big"),
        signals: vec![],
        decisions: vec![],
    }
}

#[test]
fn no_file_is_absent_not_broken() {
    let dir = TempDir::new();
    assert_eq!(status(dir.path(), &providers()), Status::Absent);
}

#[test]
fn a_valid_file_is_offered_as_one_virtual_model() {
    let dir = TempDir::new();
    file::save(dir.path(), &config()).unwrap();

    match status(dir.path(), &providers()) {
        Status::Ready(offered) => {
            assert_eq!(offered.id, "auto");
            assert_eq!(offered.provider_id, ROUTER_ID);
        }
        other => panic!("expected a ready router, got {other:?}"),
    }
}

#[test]
fn a_file_naming_a_model_nobody_serves_is_broken_and_says_which() {
    let dir = TempDir::new();
    let mut cfg = config();
    cfg.default = model("ghost");
    file::save(dir.path(), &cfg).unwrap();

    match status(dir.path(), &providers()) {
        Status::Broken(message) => assert!(message.contains("ghost"), "{message}"),
        other => panic!("expected a broken router, got {other:?}"),
    }
}

#[test]
fn an_unparsable_file_is_broken_and_not_absent() {
    let dir = TempDir::new();
    dir.write(file::FILE, "{ not json");
    assert!(matches!(status(dir.path(), &providers()), Status::Broken(_)));
}

#[test]
fn attaching_makes_the_session_a_router_session_priced_by_the_real_models() {
    let dir = TempDir::new();
    file::save(dir.path(), &config()).unwrap();
    let mut session = SessionConfig::new(dir.path());

    attach(dir.path(), &providers(), &mut session).unwrap();

    assert_eq!(session.provider, ROUTER_ID);
    assert_eq!(session.model, "auto");
    // The virtual model first, then the real one the session will price by.
    let ids: Vec<_> = session
        .models
        .iter()
        .map(|m| (m.provider_id.as_str(), m.id.as_str()))
        .collect();
    assert_eq!(ids, [(ROUTER_ID, "auto"), ("gateway", "big")]);
}

#[test]
fn attaching_without_a_file_says_where_to_put_it() {
    let dir = TempDir::new();
    let mut session = SessionConfig::new(dir.path());

    let err = match attach(dir.path(), &providers(), &mut session) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("attached without a routing.json"),
    };

    assert!(err.contains("routing.json"), "{err}");
}

#[test]
fn an_edit_is_refused_by_the_same_rule_as_a_file() {
    let err = file::parse("{ not json", "editor").unwrap_err().to_string();
    assert!(err.contains("editor"), "{err}");
}

/// A provider list with the Decision preset, answered or not.
fn with_decision(models: &[&str]) -> Providers {
    let mut all = providers();
    all.list.push(ProviderSpec {
        id: DECISION_ID.into(),
        display_name: "Decision".into(),
        protocol: Protocol::OpenAi,
        base_url: "http://localhost:11434".into(),
        headers: BTreeMap::from([("authorization".to_string(), String::new())]),
        models: models.iter().map(|m| unknown_model(m)).collect(),
    });
    all
}

fn offline(spec: &ProviderSpec) -> Arc<dyn Provider> {
    provider::build_with_key(spec.clone(), None)
}

#[test]
fn nothing_to_ask_means_no_decision_tool() {
    let dir = TempDir::new();
    // The preset is there and has not answered with any model.
    assert!(decision_setup_with(dir.path(), &with_decision(&[]), offline).is_none());
    // And no Decision provider at all.
    assert!(decision_setup_with(dir.path(), &providers(), offline).is_none());
}

#[test]
fn the_decision_provider_is_asked_when_it_has_answered() {
    let dir = TempDir::new();
    let setup = decision_setup_with(dir.path(), &with_decision(&["qwen3:1.7b", "other"]), offline)
        .expect("a model to ask");
    assert_eq!(setup.model, "qwen3:1.7b");
}

#[test]
fn the_model_chosen_for_the_router_wins() {
    let dir = TempDir::new();
    // The router's small model is `gateway/fast`, not the Decision provider's.
    file::save(dir.path(), &config()).unwrap();

    let setup = decision_setup_with(dir.path(), &with_decision(&["qwen3:1.7b"]), offline)
        .expect("a model to ask");

    assert_eq!(setup.model, "fast");
}

#[test]
fn a_broken_routing_file_does_not_take_the_tool_down() {
    let dir = TempDir::new();
    dir.write(file::FILE, "{ not json");

    let setup = decision_setup_with(dir.path(), &with_decision(&["qwen3:1.7b"]), offline)
        .expect("the Decision provider still answers");

    assert_eq!(setup.model, "qwen3:1.7b");
}

//! Which provider a session talks to, given where its model came from (§10).
//!
//! The model carries its provider, and the pair travels together. These tests
//! pin the places where a model arrives without one — a project file that
//! names only a model, a session recorded before sessions wrote their provider
//! down — and check that the model decides rather than the built-in Anthropic
//! default. The Anthropic default winning is what turned a local model into
//! «у провайдера «anthropic» нет модели».
//!
//! Real files on disk throughout: `providers.json`, `.yaca/settings.json` and a
//! session index in the shape the stand actually has.

mod common;

use common::TempProject;
use yaca_core::id::SessionId;
use yaca_core::{Providers, SessionConfig};

/// A local gateway serving one model, the way the stand is set up.
const LOCAL: &str = r#"{"list":[{"id":"unsloth","displayName":"Unsloth",
    "protocol":"openai","baseUrl":"http://127.0.0.1:8888","models":[
      {"id":"ornith","displayName":"Ornith","contextMax":91136,"maxOutput":8192,
       "pricing":{"input":0,"output":0},"supportsEffort":false}]}]}"#;

/// The providers and the directory they are read from, kept alive together.
fn local() -> (TempProject, Providers) {
    let dir = TempProject::new();
    dir.write("providers.json", LOCAL);
    let providers = Providers::load(dir.path()).unwrap();
    (dir, providers)
}

fn resolved(providers: &Providers, config: &SessionConfig) -> (String, String) {
    let (spec, model) = providers.resolve(&config.provider, &config.model).unwrap();
    (spec.id.clone(), model)
}

fn pair(provider: &str, model: &str) -> (String, String) {
    (provider.to_string(), model.to_string())
}

#[test]
fn a_session_recorded_before_it_named_its_provider_reopens_on_its_own_model() {
    let project = TempProject::new();
    // The index as it was written before `provider` was added to it.
    project.write_journal(
        "ses_old.meta.json",
        r#"{"id":"ses_old","title":"slugify","createdAt":1,"updatedAt":1,
            "model":"ornith","messages":1}"#,
    );
    let (_dir, providers) = local();

    let mut config = SessionConfig::open(project.path()).unwrap();
    config.journal = Some(project.chats());
    config.resume_session(SessionId::from("ses_old"));

    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
}

#[test]
fn a_resumed_session_keeps_its_model_over_the_project_default() {
    let project = TempProject::new();
    project.write(
        ".yaca/settings.json",
        r#"{"provider":"anthropic","model":"claude-opus-5"}"#,
    );
    project.write_journal(
        "ses_local.meta.json",
        r#"{"id":"ses_local","title":"slugify","createdAt":1,"updatedAt":1,
            "provider":"unsloth","model":"ornith"}"#,
    );
    let (_dir, providers) = local();

    let mut config = SessionConfig::open(project.path()).unwrap();
    config.journal = Some(project.chats());
    config.resume_session(SessionId::from("ses_local"));

    // The conversation was held with this model; the default changing since
    // does not move it.
    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
    assert_eq!(config.resume, Some(SessionId::from("ses_local")));
}

#[test]
fn a_project_that_names_only_a_model_goes_to_whoever_serves_it() {
    let project = TempProject::new();
    project.write(".yaca/settings.json", r#"{"model":"ornith"}"#);
    let (_dir, providers) = local();

    let config = SessionConfig::open(project.path()).unwrap();

    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
}

#[test]
fn a_model_picked_in_the_window_brings_its_provider_along() {
    let project = TempProject::new();
    project.write(
        ".yaca/settings.json",
        r#"{"provider":"anthropic","model":"claude-opus-5"}"#,
    );
    let (_dir, providers) = local();

    let mut config = SessionConfig::open(project.path()).unwrap();
    config.choose_model("unsloth".into(), "ornith".into());

    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
}

#[test]
fn a_provider_named_without_a_model_starts_on_its_first() {
    let project = TempProject::new();
    project.write(".yaca/settings.json", r#"{"provider":"unsloth"}"#);
    let (_dir, providers) = local();

    let config = SessionConfig::open(project.path()).unwrap();

    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
}

/// Nothing said anywhere starts on the first provider that answered with its
/// models (§10) — not on a model written into the code, whose provider may
/// never have been set up.
#[test]
fn nothing_said_anywhere_starts_on_the_first_provider_that_answered() {
    let project = TempProject::new();
    let (_dir, providers) = local();

    let mut config = SessionConfig::open(project.path()).unwrap();
    // A source that says nothing leaves the choice as it was.
    config.choose_model(String::new(), String::new());

    assert_eq!(resolved(&providers, &config), pair("unsloth", "ornith"));
}

#[test]
fn with_no_provider_that_answered_nothing_starts() {
    let dir = TempProject::new();
    let providers = Providers::load(dir.path()).unwrap();

    let err = providers.resolve("", "").unwrap_err().to_string();

    // Said plainly, rather than blamed on whichever preset came first.
    assert!(!err.contains("anthropic"), "{err}");
    assert!(err.contains("опросите"), "{err}");
}

/// The four models the Anthropic preset used to ship were written into the
/// code, and `providers.json` kept them. The address never answered, so the
/// provider is not configured, and a new chat must not start on it.
#[test]
fn the_catalog_the_anthropic_preset_used_to_ship_is_not_an_answer() {
    let dir = TempProject::new();
    dir.write(
        "providers.json",
        &LOCAL.replacen(
            r#"{"list":["#,
            r#"{"list":[{"id":"anthropic","displayName":"Anthropic",
              "protocol":"anthropic","baseUrl":"https://api.anthropic.com","models":[
              {"id":"claude-opus-5","displayName":"Opus 5","contextMax":1,"maxOutput":1,"pricing":{"input":0,"output":0},"supportsEffort":true},
              {"id":"claude-opus-4-8","displayName":"Opus 4.8","contextMax":1,"maxOutput":1,"pricing":{"input":0,"output":0},"supportsEffort":true},
              {"id":"claude-sonnet-5","displayName":"Sonnet 5","contextMax":1,"maxOutput":1,"pricing":{"input":0,"output":0},"supportsEffort":true},
              {"id":"claude-haiku-4-5","displayName":"Haiku 4.5","contextMax":1,"maxOutput":1,"pricing":{"input":0,"output":0},"supportsEffort":true}]},"#,
            1,
        ),
    );
    let providers = Providers::load(dir.path()).unwrap();

    assert!(providers.get("anthropic").unwrap().models.is_empty());
    assert_eq!(providers.resolve("", "").unwrap().0.id, "unsloth");
}

#[test]
fn a_model_nobody_serves_is_not_blamed_on_anthropic() {
    let (_dir, providers) = local();

    let err = providers.resolve("", "qwen3-coder").unwrap_err().to_string();

    // Anthropic was never asked, and the message must not say it was.
    assert!(!err.contains("anthropic"), "{err}");
    assert!(err.contains("qwen3-coder"), "{err}");
}

#[test]
fn a_named_provider_that_is_gone_is_not_swapped_for_another() {
    let (_dir, providers) = local();

    // Anthropic serves this model, but the session asked for someone else:
    // quietly sending the conversation there is the thing not to do.
    let err = providers.resolve("ollama", "claude-opus-5").unwrap_err().to_string();

    assert!(err.contains("ollama"), "{err}");
}

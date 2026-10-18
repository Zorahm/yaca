use super::*;
use crate::testutil::TempDir;
use crate::usage::unknown_model;

fn spec(protocol: Protocol) -> ProviderSpec {
    ProviderSpec {
        id: "gateway".into(),
        display_name: "Gateway".into(),
        protocol,
        base_url: "https://gate.example.com/".into(),
        headers: BTreeMap::new(),
        models: vec![],
    }
}

#[test]
fn a_missing_file_gives_both_presets() {
    let dir = TempDir::new();
    let providers = Providers::load(dir.path()).unwrap();
    assert!(providers.get(ANTHROPIC_ID).is_some());

    // The OpenAI shape is the second one, and it arrives without models: they come from
    // `/v1/models`, not from a list written here by eye.
    let openai = providers.get(OPENAI_ID).unwrap();
    assert_eq!(openai.protocol, Protocol::OpenAi);
    assert!(openai.models.is_empty());
    assert!(openai.needs_key());
}

#[test]
fn a_preset_without_models_honestly_refuses_to_be_chosen() {
    // While the list is empty there is nothing to talk to — and that is an error, not silence.
    assert!(ProviderSpec::openai().resolve_model("gpt-5").is_err());
}

#[test]
fn both_presets_are_not_removed() {
    let mut providers = Providers::default();
    assert!(providers.remove(ANTHROPIC_ID).is_err());
    assert!(providers.remove(OPENAI_ID).is_err());
}

#[test]
fn a_broken_file_stops_the_load_instead_of_falling_back_to_defaults() {
    let dir = TempDir::new();
    dir.write(FILE, "{ not json");
    assert!(Providers::load(dir.path()).is_err());
}

#[test]
fn a_provider_from_the_file_stays_in_the_list() {
    let dir = TempDir::new();
    dir.write(
        FILE,
        r#"{"list":[{"id":"gateway","displayName":"Gateway",
           "protocol":"openai","baseUrl":"https://gate.example.com","models":[]}]}"#,
    );
    let providers = Providers::load(dir.path()).unwrap();
    assert!(providers.get(ANTHROPIC_ID).is_some());
    assert!(providers.get(OPENAI_ID).is_some());
    assert_eq!(providers.get("gateway").unwrap().protocol, Protocol::OpenAi);
    // A custom gateway stays where the file's author put it: the presets
    // arrive after it, not in front.
    assert_eq!(providers.list[0].id, "gateway");
}

#[test]
fn the_list_survives_a_save() {
    let dir = TempDir::new();
    let mut providers = Providers::default();
    providers.put(spec(Protocol::OpenAi));
    providers.save(dir.path()).unwrap();

    assert_eq!(Providers::load(dir.path()).unwrap(), providers);
}

#[test]
fn the_key_goes_into_the_header_and_not_into_the_file() {
    let mut s = spec(Protocol::OpenAi);
    s.headers.insert("x-title".into(), "Yaca".into());
    let headers = s.headers_for(Some("sk-live"));

    assert!(headers.contains(&("authorization".into(), "Bearer sk-live".into())));
    assert!(headers.contains(&("x-title".into(), "Yaca".into())));
    // The key is not in the description in any form.
    assert!(!serde_json::to_string(&s).unwrap().contains("sk-live"));
}

#[test]
fn an_own_header_overrides_the_protocol_default() {
    let mut s = spec(Protocol::Anthropic);
    s.headers
        .insert("x-api-key".into(), "static-token".into());
    let headers = s.headers_for(None);

    assert!(headers.contains(&("x-api-key".into(), "static-token".into())));
    assert!(!s.needs_key());
}

#[test]
fn without_a_placeholder_no_key_is_needed() {
    let mut s = spec(Protocol::OpenAi);
    s.headers.insert("authorization".into(), String::new());
    assert!(!s.needs_key());
}

/// No key is not a refusal (§10): the header that waits for one is left out,
/// and the server decides. A local one never asks.
#[test]
fn a_header_waiting_for_a_key_is_left_out_without_one() {
    let s = spec(Protocol::OpenAi);
    assert!(s.needs_key());
    for key in [None, Some("   ")] {
        let headers = s.headers_for(key);
        assert!(
            headers.iter().all(|(name, _)| name != "authorization"),
            "{headers:?}"
        );
        assert!(headers.iter().any(|(name, _)| name == "content-type"));
    }
}

#[test]
fn a_model_not_in_the_list_is_an_error() {
    let mut s = spec(Protocol::OpenAi);
    s.models = vec![ModelInfo {
        id: "qwen3-coder".into(),
        provider_id: "gateway".into(),
        display_name: "Qwen3 Coder".into(),
        context_max: 256_000,
        max_output: 32_000,
        pricing: crate::usage::Pricing {
            input: 0.0,
            output: 0.0,
        },
        supports_effort: false,
        supports_thinking: false,
        context_source: crate::usage::ContextSource::DefaultGuess,
        supports_images: true,
    }];

    // Another provider's model is not silently substituted, it is rejected.
    assert!(s.resolve_model("claude-opus-5").is_err());
    // A provider's own model stays itself.
    assert_eq!(s.resolve_model("qwen3-coder").unwrap(), "qwen3-coder");
    // An empty string picks the first one.
    assert_eq!(s.resolve_model("").unwrap(), "qwen3-coder");
}

#[test]
fn a_provider_without_models_cannot_be_chosen() {
    assert!(spec(Protocol::OpenAi).resolve_model("anything").is_err());
}

#[test]
fn a_file_written_before_the_model_field_is_still_read() {
    let dir = TempDir::new();
    dir.write(
        FILE,
        r#"{"active":"ollama","list":[{"id":"ollama","displayName":"Ollama",
           "protocol":"openai","baseUrl":"http://localhost:11434","models":[
             {"id":"qwen3-coder","displayName":"Qwen3","contextMax":256000,
              "maxOutput":32000,"pricing":{"input":0,"output":0},
              "supportsEffort":false}]}]}"#,
    );

    // An update must not break someone else's setting: a broken file here
    // stops the launch, and a new field is no reason for that.
    let providers = Providers::load(dir.path()).unwrap();
    let model = &providers.get("ollama").unwrap().models[0];
    assert_eq!(model.id, "qwen3-coder");
    assert!(!model.supports_thinking, "a silent file means «no»");
}

#[test]
fn a_trailing_slash_in_the_address_is_not_doubled() {
    assert_eq!(
        spec(Protocol::OpenAi).endpoint("/v1/chat/completions"),
        "https://gate.example.com/v1/chat/completions"
    );
}

#[test]
fn removing_a_provider_takes_it_off_the_list() {
    let mut providers = Providers::default();
    providers.put(spec(Protocol::OpenAi));
    assert!(providers.get("gateway").is_some());
    providers.remove("gateway").unwrap();
    assert!(providers.get("gateway").is_none());
}

#[test]
fn finding_a_model_and_all_models() {
    let mut providers = Providers::default();
    let mut s = spec(Protocol::OpenAi);
    s.models = vec![unknown_model("qwen3-coder")];
    providers.put(s);

    let (p, m) = providers.find_by_model("qwen3-coder").unwrap();
    assert_eq!(p.id, "gateway");
    assert_eq!(m.id, "qwen3-coder");

    let all = providers.all_models();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].provider_id, "gateway");
}

/// Configured means the address answered with its models (§10). A preset
/// has not been asked anything yet, so it offers none.
#[test]
fn a_preset_comes_without_models() {
    let providers = Providers::default();
    assert!(providers.all_models().is_empty());
}

#[test]
fn presets_are_all_there_by_id() {
    let ids: Vec<_> = Providers::default().list.into_iter().map(|p| p.id).collect();
    assert_eq!(
        ids,
        ["openai", "anthropic", "google", "openrouter", "deepseek", "zai", "decision"]
    );
    assert!(ids.iter().all(|id| is_preset(id)));
    assert!(!is_preset("gateway"));
    // The two the rest of the core names directly.
    assert_eq!(ProviderSpec::openai().id, OPENAI_ID);
    assert_eq!(ProviderSpec::anthropic().id, ANTHROPIC_ID);
}

/// A file saved when there were two presets gains the rest on the next load,
/// and what it already said about its own stays said.
#[test]
fn a_file_from_before_a_preset_existed_gains_it_at_the_end() {
    let dir = TempDir::new();
    dir.write(
        FILE,
        r#"{"list":[
            {"id":"anthropic","displayName":"Anthropic","protocol":"anthropic",
             "baseUrl":"https://proxy.example.com","models":[]},
            {"id":"openai","displayName":"OpenAI","protocol":"openai",
             "baseUrl":"https://api.openai.com","models":[]}]}"#,
    );
    let providers = Providers::load(dir.path()).unwrap();

    let ids: Vec<_> = providers.list.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["anthropic", "openai", "google", "openrouter", "deepseek", "zai", "decision"]
    );
    assert_eq!(
        providers.get(ANTHROPIC_ID).unwrap().base_url,
        "https://proxy.example.com"
    );
}

#[test]
fn the_protocol_path_goes_under_v1() {
    let s = spec(Protocol::OpenAi);
    assert_eq!(s.api("/models"), "https://gate.example.com/v1/models");
    // OpenRouter: a path of its own, and no version in it.
    let mut s = spec(Protocol::OpenAi);
    s.base_url = "https://openrouter.ai/api".into();
    assert_eq!(
        s.api("/chat/completions"),
        "https://openrouter.ai/api/v1/chat/completions"
    );
}

#[test]
fn an_address_that_names_its_version_is_not_given_another() {
    let mut s = spec(Protocol::OpenAi);
    for (base, expected) in [
        // Google AI Studio and z.ai: the version is not `v1`, and not last.
        (
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "https://generativelanguage.googleapis.com/v1beta/openai/models",
        ),
        (
            "https://api.z.ai/api/coding/paas/v4/",
            "https://api.z.ai/api/coding/paas/v4/models",
        ),
        // The base URL pasted with its `/v1` on.
        ("https://api.openai.com/v1", "https://api.openai.com/v1/models"),
        // A version in the host is not a version of the path.
        ("https://v2.example.com", "https://v2.example.com/v1/models"),
        // Nor is a word that only starts like one.
        ("https://example.com/vendor", "https://example.com/vendor/v1/models"),
    ] {
        s.base_url = base.into();
        assert_eq!(s.api("/models"), expected);
    }
}

/// The place the router's small model lives. It is a ready entry, and it must
/// not demand a key before it can be used: a local server never asks for one.
#[test]
fn the_decision_preset_is_a_keyless_openai_shaped_slot() {
    let providers = Providers::default();
    let decision = providers.get(DECISION_ID).expect("the Decision preset");

    assert_eq!(decision.protocol, Protocol::OpenAi);
    assert!(!decision.needs_key());
    // The protocol's `Bearer {key}` is overridden to nothing, so no credential
    // goes out — not even a half-filled `Bearer ` — which is how the rest of
    // the core writes a keyless server down.
    assert!(decision
        .headers_for(None)
        .iter()
        .all(|(name, value)| name != "authorization" || value.is_empty()));
    assert!(is_preset(DECISION_ID));
    // The preset cannot be deleted: the next load would put it back.
    let mut providers = providers;
    assert!(providers.remove(DECISION_ID).is_err());
}

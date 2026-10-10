use super::*;
use crate::testutil::TempDir;
use crate::usage::unknown_model;

fn spec(protocol: Protocol) -> ProviderSpec {
    ProviderSpec {
        id: "gateway".into(),
        display_name: "Шлюз".into(),
        protocol,
        base_url: "https://gate.example.com/".into(),
        headers: BTreeMap::new(),
        models: vec![],
    }
}

#[test]
fn отсутствующий_файл_даёт_оба_предустановленных() {
    let dir = TempDir::new();
    let providers = Providers::load(dir.path()).unwrap();
    assert!(providers.get(ANTHROPIC_ID).is_some());

    // OpenAI-форма — вторая, и приезжает без моделей: их отдаёт
    // `/v1/models`, а не список, написанный здесь на глазок.
    let openai = providers.get(OPENAI_ID).unwrap();
    assert_eq!(openai.protocol, Protocol::OpenAi);
    assert!(openai.models.is_empty());
    assert!(openai.needs_key());
}

#[test]
fn предустановленный_без_моделей_честно_отказывается_быть_выбранным() {
    // Пока список пуст, разговаривать нечем — и это ошибка, а не тишина.
    assert!(ProviderSpec::openai().resolve_model("gpt-5").is_err());
}

#[test]
fn предустановленные_не_удаляются_оба() {
    let mut providers = Providers::default();
    assert!(providers.remove(ANTHROPIC_ID).is_err());
    assert!(providers.remove(OPENAI_ID).is_err());
}

#[test]
fn битый_файл_останавливает_а_не_подменяется_дефолтом() {
    let dir = TempDir::new();
    dir.write(FILE, "{ не json");
    assert!(Providers::load(dir.path()).is_err());
}

#[test]
fn встроенный_провайдер_возвращается_в_список() {
    let dir = TempDir::new();
    dir.write(
        FILE,
        r#"{"list":[{"id":"gateway","displayName":"Шлюз",
           "protocol":"openai","baseUrl":"https://gate.example.com","models":[]}]}"#,
    );
    let providers = Providers::load(dir.path()).unwrap();
    assert!(providers.get(ANTHROPIC_ID).is_some());
    assert!(providers.get(OPENAI_ID).is_some());
    assert_eq!(providers.get("gateway").unwrap().protocol, Protocol::OpenAi);
    // Свой шлюз остаётся там, где его поставил автор файла.
    assert_eq!(providers.list[1].id, "gateway");
}

#[test]
fn список_переживает_запись() {
    let dir = TempDir::new();
    let mut providers = Providers::default();
    providers.put(spec(Protocol::OpenAi));
    providers.save(dir.path()).unwrap();

    assert_eq!(Providers::load(dir.path()).unwrap(), providers);
}

#[test]
fn ключ_подставляется_в_заголовок_а_не_лежит_в_файле() {
    let mut s = spec(Protocol::OpenAi);
    s.headers.insert("x-title".into(), "Yaca".into());
    let headers = s.headers_for(Some("sk-живой"));

    assert!(headers.contains(&("authorization".into(), "Bearer sk-живой".into())));
    assert!(headers.contains(&("x-title".into(), "Yaca".into())));
    // В самом описании ключа нет ни в каком виде.
    assert!(!serde_json::to_string(&s).unwrap().contains("sk-живой"));
}

#[test]
fn свой_заголовок_перебивает_умолчание_протокола() {
    let mut s = spec(Protocol::Anthropic);
    s.headers
        .insert("x-api-key".into(), "статичный-токен".into());
    let headers = s.headers_for(None);

    assert!(headers.contains(&("x-api-key".into(), "статичный-токен".into())));
    assert!(!s.needs_key());
}

#[test]
fn без_плейсхолдера_ключ_не_требуется() {
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
fn модель_не_из_списка_дает_ошибку() {
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

    // Модель другого провайдера не подменяется молча, а отвергается.
    assert!(s.resolve_model("claude-opus-5").is_err());
    // Своя модель остаётся собой.
    assert_eq!(s.resolve_model("qwen3-coder").unwrap(), "qwen3-coder");
    // Пустая строка берёт первую.
    assert_eq!(s.resolve_model("").unwrap(), "qwen3-coder");
}

#[test]
fn провайдер_без_моделей_нельзя_выбрать() {
    assert!(spec(Protocol::OpenAi).resolve_model("что-нибудь").is_err());
}

#[test]
fn файл_написанный_до_нового_поля_модели_всё_ещё_читается() {
    let dir = TempDir::new();
    dir.write(
        FILE,
        r#"{"active":"ollama","list":[{"id":"ollama","displayName":"Ollama",
           "protocol":"openai","baseUrl":"http://localhost:11434","models":[
             {"id":"qwen3-coder","displayName":"Qwen3","contextMax":256000,
              "maxOutput":32000,"pricing":{"input":0,"output":0},
              "supportsEffort":false}]}]}"#,
    );

    // Обновление не должно ронять чужую настройку: битый файл здесь
    // останавливает запуск, и новое поле не повод.
    let providers = Providers::load(dir.path()).unwrap();
    let model = &providers.get("ollama").unwrap().models[0];
    assert_eq!(model.id, "qwen3-coder");
    assert!(!model.supports_thinking, "молчание файла — это «нет»");
}

#[test]
fn косая_черта_в_конце_адреса_не_удваивается() {
    assert_eq!(
        spec(Protocol::OpenAi).endpoint("/v1/chat/completions"),
        "https://gate.example.com/v1/chat/completions"
    );
}

#[test]
fn удаление_провайдера_убирает_его_из_списка() {
    let mut providers = Providers::default();
    providers.put(spec(Protocol::OpenAi));
    assert!(providers.get("gateway").is_some());
    providers.remove("gateway").unwrap();
    assert!(providers.get("gateway").is_none());
}

#[test]
fn поиск_модели_и_все_модели() {
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

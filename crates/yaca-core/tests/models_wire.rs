//! Опрос моделей против настоящего сокета (§10).
//!
//! Разбор ответов покрыт юнит-тестами; здесь проверяется то, что между ними:
//! адрес, заголовок `accept` и постраничный обход. Всё это живёт в HTTP, а не
//! в разборе, и мок его не поймал бы.

use std::collections::BTreeMap;

use yaca_core::provider::models::{discover, residency};
use yaca_core::provider::registry::{Protocol, ProviderSpec};

mod common;
use common::wire::{json_response, serve, serve_all};

fn spec(base: impl Into<String>, protocol: Protocol) -> ProviderSpec {
    ProviderSpec {
        id: "gateway".into(),
        display_name: "Шлюз".into(),
        protocol,
        base_url: base.into(),
        headers: BTreeMap::new(),
        models: vec![],
    }
}

fn json(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn список_приезжает_страницами_и_склеивается() {
    let first = json(
        r#"{"data":[{"id":"claude-opus-5","display_name":"Claude Opus 5","max_input_tokens":1000000,"max_tokens":128000,"capabilities":{"effort":{"supported":true},"thinking":{"types":{"adaptive":{"supported":true}}}}}],"has_more":true,"last_id":"claude-opus-5"}"#,
    );
    let second = json(r#"{"data":[{"id":"claude-haiku-4-5"}],"has_more":false}"#);

    let (base, seen) = serve_all(vec![first, second]).await;
    let models = discover(&spec(base, Protocol::Anthropic), Some("sk-test"))
        .await
        .unwrap();

    let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["claude-opus-5", "claude-haiku-4-5"]);
    assert_eq!(models[0].context_max, 1_000_000);
    assert!(models[0].supports_effort);

    let seen = seen.await.unwrap();
    assert_eq!(seen.len(), 2, "вторая страница спрашивается отдельно");
    // Просим JSON, а не поток: `accept` протокола — для `/v1/messages`.
    assert!(seen[0].has_header("accept", "application/json"), "{}", seen[0].headers);
    assert!(seen[0].headers.contains("GET /v1/models?limit="));
    // Вторая страница начинается после последнего с первой.
    assert!(
        seen[1].headers.contains("after_id=claude-opus-5"),
        "{}",
        seen[1].headers
    );
    // Ключ подставился и сюда — список моделей за ним же.
    assert!(seen[0].has_header("x-api-key", "sk-test"));
}

#[tokio::test]
async fn страница_без_курсора_не_уходит_в_бесконечность() {
    // `has_more` без `last_id` — спрашивать нечем, и цикл кончается.
    let (base, _seen) = serve(json(r#"{"data":[{"id":"m"}],"has_more":true}"#)).await;
    let models = discover(&spec(base, Protocol::Anthropic), Some("sk-test"))
        .await
        .unwrap();
    assert_eq!(models.len(), 1);
}

#[tokio::test]
async fn openai_форма_спрашивает_один_раз() {
    let (base, seen) = serve(json(
        r#"{"object":"list","data":[{"id":"qwen3-coder"},{"id":"llama3.3"}]}"#,
    ))
    .await;

    let models = discover(&spec(base, Protocol::OpenAi), Some("sk-test"))
        .await
        .unwrap();

    let ids: Vec<_> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["llama3.3", "qwen3-coder"]);

    let seen = seen.await.unwrap();
    assert!(seen.headers.contains("GET /v1/models"));
    assert!(seen.has_header("accept", "application/json"));
}

#[tokio::test]
async fn отказ_доносится_как_отказ_а_не_как_пустой_список() {
    let body = r#"{"error":{"message":"invalid x-api-key"}}"#;
    let response = format!(
        "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let (base, _seen) = serve(response).await;

    // Пустой список означал бы «у провайдера нет моделей», а провайдер
    // сказал совсем другое (§0).
    let err = discover(&spec(base, Protocol::Anthropic), Some("sk-bad"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("invalid x-api-key"), "{err}");
}

#[tokio::test]
async fn discovery_without_a_key_says_which_key_is_missing() {
    let body = r#"{"error":{"message":"Incorrect API key provided"}}"#;
    let response = format!(
        "HTTP/1.1 401 Unauthorized
Content-Type: application/json
Content-Length: {}
Connection: close

{body}",
        body.len()
    );
    let (base, _seen) = serve(response).await;

    let err = discover(&spec(base, Protocol::OpenAi), None)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("Incorrect API key provided"), "{err}");
    assert!(err.contains("«gateway»"), "{err}");
}

#[tokio::test]
async fn residency_is_asked_of_the_server_each_time() {
    let (base, seen) = serve(json(
        r#"{"data":[{"id":"small","loaded":true},{"id":"large","loaded":false}]}"#,
    ))
    .await;

    let residency = residency(&spec(base, Protocol::OpenAi), Some("sk-test"))
        .await
        .unwrap();

    assert_eq!(residency.get("small"), Some(&true));
    assert_eq!(residency.get("large"), Some(&false));
    let seen = seen.await.unwrap();
    assert!(seen.headers.contains("GET /v1/models"));
}

#[tokio::test]
async fn anthropic_is_not_asked_about_residency() {
    // Nothing listens here: a request would fail, and an empty answer proves
    // none was made. Anthropic serves every model it lists.
    let residency = residency(&spec("http://127.0.0.1:9", Protocol::Anthropic), Some("sk"))
        .await
        .unwrap();
    assert!(residency.is_empty());
}

// The local servers' own endpoints (§10, `provider::models::local`). Ollama
// and LM Studio say nothing about context on the OpenAI shape; a loopback
// model list without it gets probed, and this is what the probes see.

#[tokio::test]
async fn lm_studio_window_is_detected_for_the_whole_list_with_one_probe() {
    let (base, seen) = serve_all(vec![
        json(r#"{"object":"list","data":[{"id":"qwen/qwen3-30b"},{"id":"gpt-oss-20b"}]}"#),
        json(
            r#"{"object":"list","data":[
                {"id":"qwen/qwen3-30b","max_context_length":262144,
                 "loaded_instances":[{"config":{"context_length":65536}}]},
                {"id":"gpt-oss-20b"}]}"#,
        ),
    ])
    .await;
    let models = discover(&spec(base, Protocol::OpenAi), None).await.unwrap();

    // The list is ordered by id; the allocation, not the maximum, is what a
    // live session has to fit into.
    assert_eq!(models[1].context_max, 65_536);
    assert_eq!(models[1].context_source, yaca_core::usage::ContextSource::DetectedAllocated);
    // No fields of its own: the guess stays a guess, visibly.
    assert_eq!(models[0].context_source, yaca_core::usage::ContextSource::DefaultGuess);
    assert_eq!(models[0].context_max, 128_000);

    let seen = seen.await.unwrap();
    assert!(seen[0].headers.contains("GET /v1/models"));
    assert!(seen[1].headers.contains("GET /api/v1/models"));
    assert_eq!(seen.len(), 2, "one probe answers for everyone");
}

#[tokio::test]
async fn an_idle_lm_studio_answers_with_the_architectural_maximum() {
    // Nothing loaded: the architectural maximum is all LM Studio says, and it
    // is read as such rather than as what a running session sees.
    let (base, _seen) = serve_all(vec![
        json(r#"{"object":"list","data":[{"id":"qwen/qwen3-30b"}]}"#),
        json(
            r#"{"object":"list","data":[
                {"id":"qwen/qwen3-30b","max_context_length":131072}]}"#,
        ),
    ])
    .await;
    let models = discover(&spec(base, Protocol::OpenAi), None).await.unwrap();
    assert_eq!(models[0].context_max, 131_072);
    assert_eq!(models[0].context_source, yaca_core::usage::ContextSource::DetectedMaximum);
}

#[tokio::test]
async fn ollama_answers_with_what_it_loaded_and_what_it_is_built_of() {
    // One model in memory with the exponent of a per-request `num_ctx`; one
    // not loaded, asked about individually by `/api/show`. The bare OpenAI
    // list names both nowhere.
    let (base, seen) = serve_all(vec![
        json(r#"{"object":"list","data":[{"id":"qwen3-coder:30b"},{"id":"llama3.3:70b"}]}"#),
        json_response("404 Not Found", r#"{"error":{"message":"no such path"}}"#),
        json(
            r#"{"models":[{"name":"qwen3-coder:30b","context_length":65536}]}"#,
        ),
        json(r#"{"model_info":{"llama.context_length":131072}}"#),
    ])
    .await;
    let models = discover(&spec(base, Protocol::OpenAi), None).await.unwrap();

    assert_eq!(models[0].context_max, 131_072, "architectural maximum");
    assert_eq!(models[0].context_source, yaca_core::usage::ContextSource::DetectedMaximum);
    assert_eq!(models[1].context_max, 65_536, "allocated wins over maximum");
    assert_eq!(models[1].context_source, yaca_core::usage::ContextSource::DetectedAllocated);

    let seen = seen.await.unwrap();
    assert!(seen[2].headers.contains("GET /api/ps"));
    assert!(seen[3].headers.contains("POST /api/show"));
    // The model asked about is the one not in `/api/ps`.
    assert_eq!(seen[3].body["model"], "llama3.3:70b");
}

#[tokio::test]
async fn a_local_server_that_says_nothing_keeps_the_openai_default() {
    // The 404s are what a loopback server that keeps neither probe endpoint
    // answers. The fallback window is the OpenAI shape's documented 128k —
    // `unknown_model`'s, left visibly a guess by the context source.
    let (base, seen) = serve_all(vec![
        json(r#"{"object":"list","data":[{"id":"qwen3-coder:30b"}]}"#),
        json_response("404 Not Found", r#"{"error":{"message":"no such path"}}"#),
        json_response("404 Not Found", r#"{"error":{"message":"no such path"}}"#),
    ])
    .await;
    let models = discover(&spec(base, Protocol::OpenAi), None)
        .await
        .unwrap();
    let m = &models[0];
    assert_eq!(m.context_source, yaca_core::usage::ContextSource::DefaultGuess);
    assert_eq!(m.context_max, 128_000);

    let seen = seen.await.unwrap();
    // No third endpoint to be tried: `/api/ps` refusing means this is not an
    // Ollama at all, and `/api/show` goes unasked.
    assert_eq!(seen.len(), 3);
}

//! Probing a local model server for what the OpenAI shape never tells (§10).
//!
//! Ollama and LM Studio answer `GET /v1/models` with ids and nothing else, so
//! their windows stay a guess — and a harmful one: Ollama allocates whatever
//! the modelfile left `num_ctx` at and then silently truncates a system prompt
//! longer than that. The same server keeps the real numbers on its own
//! endpoints, and they are read at discovery time, when the extra round trips
//! cost one button press.
//!
//! Two kinds of numbers, preferred in this order:
//!
//! - the window actually allocated to a loaded model — what the server will
//!   fill with this conversation;
//! - the model's architectural maximum — what a *freshly loaded* model would
//!   get; better than a guess, but not what a session runs into today.
//!
//! Neither endpoint reports an output cap anywhere, so `max_output` stays
//! whatever the OpenAI shape left it at — a configured value, not a detected
//! one.
//!
//! *Probing is loopback only.* The endpoints belong to these two servers, and
//! firing `POST /api/show` at api.openai.com because a port matched would be
//! a request nobody asked for. A provider elsewhere on the network keeps its
//! OpenAI-shape default (see [`crate::usage::unknown_model`]), and so does one
//! on this machine that refuses or stalls the probe. The fallback is not
//! silent: the model's window stays tagged [`ContextSource::DefaultGuess`],
//! which the switcher's ring can show as a guess rather than a fact, and what
//! the probe failed to read is said in the log.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Value, json};

use super::http;
use crate::provider::registry::ProviderSpec;
use crate::usage::{ContextSource, ModelInfo};

/// How long any one probe may take. Local servers answer in milliseconds; a
/// stalled one must not stall discovery, which already waited on `/v1/models`.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long reaching the server itself may take. A loopback port nobody is
/// listening on refuses instantly, but a firewall that eats the packets
/// otherwise hangs the probe until the total timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Paths on LM Studio's and Ollama's own HTTP surface — the points their
/// servers answer the way Chat Completions never does.
const LM_STUDIO_MODELS: &str = "/api/v1/models";
const OLLAMA_PS: &str = "/api/ps";
const OLLAMA_SHOW: &str = "/api/show";

/// A window plus how it came about: what the switcher shows and the ring
/// depends on knowing the guess from the fact.
type Window = (u32, ContextSource);

/// Reads context windows out of a local server, in place.
///
/// Only ever called on the OpenAI shape's own gateways: the local endpoints
/// sit next to Chat Completions, and the Anthropic shape answers the window
/// itself.
pub(super) async fn probe(spec: &ProviderSpec, key: Option<&str>, models: &mut [ModelInfo]) {
    if !is_local(spec) {
        return;
    }
    // Windows the OpenAI shape already stated are facts, not guesses, and
    // probing over them re-asked for numbers no server hides twice.
    if models
        .iter()
        .all(|m| m.context_source != ContextSource::DefaultGuess)
    {
        return;
    }
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .build()
        // Invariant: the builder only rejects TLS options, and this client
        // sets none — no custom certificates, no identity.
        .expect("клиент для опроса локального сервера");

    // LM Studio first: one request there answers for the whole list, where
    // Ollama needs one per model. The path is LM Studio's own; Ollama answers
    // 404 on it, and the probe moves on.
    if lm_studio(&client, spec, key, models).await.is_some() {
        return;
    }
    ollama(&client, spec, key, models).await;

    // The documented fallback, said rather than taken for granted: the models
    // left over keep `unknown_model`'s 128k, and anyone reading the log can
    // tell a fact from the guess that stayed.
    let remaining = models
        .iter()
        .filter(|m| m.context_source == ContextSource::DefaultGuess)
        .count();
    if remaining > 0 {
        tracing::warn!(
            "провайдер «{}»: у {} моделей окно контекста не прочитано — осталась догадка",
            spec.id,
            remaining
        );
    }
}

/// Is this provider's address on this machine?
///
/// By host rather than by port: both servers move, and neither owns a port.
fn is_local(spec: &ProviderSpec) -> bool {
    let Ok(url) = spec.base_url.parse::<reqwest::Url>() else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    // An IPv6 host arrives bracketed: `[::1]`. The brackets are in the way of
    // `IpAddr::from_str` and mean nothing to the comparison.
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// A `GET` of a local endpoint, parsed as JSON.
///
/// Unlike the OpenAI shape's list, a refusal here is not an error to raise:
/// it means the server does not keep that endpoint, and the answer is
/// `None` — the probe moves on.
async fn get_json(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    path: &str,
) -> Option<Value> {
    let mut headers = http::header_map(spec, key).ok()?;
    // The protocol's own accept is for streaming Chat Completions; asking a
    // plain endpoint with it earns a 406 on some gateways — the list of models
    // made the same correction already.
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    let resp = client
        .get(spec.endpoint(path))
        .headers(headers)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json().await.ok()
}

/// A `POST` with a JSON body, for the endpoints that read one. Same deal as
/// [`get_json`]: any refusal is silence, not a failure.
async fn post_json(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    path: &str,
    body: Value,
) -> Option<Value> {
    let mut headers = http::header_map(spec, key).ok()?;
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    let resp = client
        .post(spec.endpoint(path))
        .headers(headers)
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json().await.ok()
}

/// LM Studio's `/api/v1/models`: one request, windows for the whole list.
///
/// `Some` — answered as LM Studio does, every window it knew is written in.
/// `None` — it did not: the endpoint is not seen outside LM Studio, so silence
/// from it means the server is not LM Studio at all.
async fn lm_studio(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    models: &mut [ModelInfo],
) -> Option<()> {
    let answer = get_json(client, spec, key, LM_STUDIO_MODELS).await?;
    let windows = lm_studio_windows(&answer);
    for m in models {
        if let Some(window) = windows.get(&m.id) {
            m.context_max = window.0;
            m.context_source = window.1;
        }
    }
    Some(())
}

/// LM Studio's model entry: `max_context_length` is the architectural maximum,
/// each item of `loaded_instances` is a copy actually in memory, and
/// `config.context_length` under it is what that copy allocated. The server
/// cannot resize per request — the window is fixed at load — so a loaded
/// copy's figure is the one a session must fit into, and it wins.
fn lm_studio_windows(answer: &Value) -> HashMap<String, Window> {
    let Some(data) = answer.get("data").and_then(Value::as_array) else {
        tracing::warn!("{LM_STUDIO_MODELS} ответил не тем списком, который читает Yaca");
        return HashMap::new();
    };

    let mut windows = HashMap::new();
    for entry in data {
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(window) = lm_studio_window(entry) {
            windows.insert(id.to_string(), window);
        }
    }
    windows
}

fn lm_studio_window(entry: &Value) -> Option<Window> {
    // Allocated first: what a loaded copy holds is what this conversation has
    // to fit in. `config` travels under camelCase in the shape.
    let allocated = entry
        .get("loaded_instances")
        .and_then(Value::as_array)
        .and_then(|instances| {
            instances
                .iter()
                .find_map(|i| i.pointer("/config/context_length"))
                .and_then(Value::as_u64)
        })
        .map(|window| (window, ContextSource::DetectedAllocated));
    let maximum = entry
        .get("max_context_length")
        .and_then(Value::as_u64)
        .map(|window| (window, ContextSource::DetectedMaximum));
    let (window, source) = allocated.or(maximum)?;
    (u32::try_from(window).ok()).map(|value| (value, source))
}

/// Ollama, in two requests' worth of answers.
///
/// `GET /api/ps` lists what is loaded right now, each model with the window it
/// actually allocated; `POST /api/show` says what a model is built of, and
/// among it the architectural maximum. A loaded model gets its allocated
/// window — that is the one this conversation has to fit into. The rest are
/// asked about individually: the maximum is the only figure Ollama gives for a
/// model that is not loaded, and until it loads, that is still better than a
/// guess.
async fn ollama(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    models: &mut [ModelInfo],
) {
    // `None` — the endpoint is refused or missing. Ollama keeps `/api/ps`,
    // so a refusal means the server is not Ollama and `/api/show` is not
    // worth a request either: like an empty answer, it only means fewer
    // windows are known, and it is no error to raise.
    let allocated = match get_json(client, spec, key, OLLAMA_PS).await {
        Some(answer) => ollama_allocated(&answer),
        None => return,
    };

    // Ollama lists every quant of every library it ever pulled, and the shows
    // go out together: a burst of asks costs a local server the same, one
    // after another would cost discovery a second.
    let wanted: Vec<&str> = models
        .iter()
        .filter(|m| !allocated.contains_key(&m.id))
        .map(|m| m.id.as_str())
        .collect();
    let shows = wanted
        .iter()
        .map(|id| {
            let id = (*id).to_string();
            async move {
                match post_json(client, spec, key, OLLAMA_SHOW, json!({ "model": id })).await {
                    Some(answer) => (id, ollama_maximum(&answer)),
                    None => (id, None),
                }
            }
        })
        .collect::<Vec<_>>();
    let maximum: HashMap<String, u32> = futures_util::future::join_all(shows)
        .await
        .into_iter()
        .filter_map(|(id, window)| window.map(|window| (id, window)))
        .collect();

    for m in models {
        match allocated.get(&m.id) {
            // Allocated before maximum: see the `ollama` doc note.
            Some(window) => apply(m, *window),
            None => {
                if let Some(window) = maximum.get(&m.id) {
                    apply(m, (*window, ContextSource::DetectedMaximum));
                }
            }
        }
    }
}

/// The loaded window per model. `context_length` sits at the top level of the
/// model object — `docs/api.md` does not show it, but `api/types.go` carries
/// it (`ContextLength int`, tagged `context_length`), and it is read there.
fn ollama_allocated(answer: &Value) -> HashMap<String, Window> {
    let Some(loaded) = answer.get("models").and_then(Value::as_array) else {
        tracing::warn!("{OLLAMA_PS} ответил не тем списком, который читает Yaca");
        return HashMap::new();
    };

    loaded
        .iter()
        .filter_map(|m| {
            let id = m.get("name")?.as_str()?;
            let window = m.get("context_length").and_then(Value::as_u64)?;
            Some((id.to_string(), u32::try_from(window).ok()?))
        })
        .map(|(id, window)| (id, (window, ContextSource::DetectedAllocated)))
        .collect()
}

/// `<architecture>.context_length` in Ollama's `model_info` — the window the
/// model was built with, whichever family carries it.
fn ollama_maximum(answer: &Value) -> Option<u32> {
    let info = answer.get("model_info")?.as_object()?;
    info.iter()
        .filter_map(|(key, value)| {
            if !key.ends_with(".context_length") {
                return None;
            }
            value.as_u64().and_then(|window| u32::try_from(window).ok())
        })
        .max()
}

fn apply(m: &mut ModelInfo, window: Window) {
    m.context_max = window.0;
    m.context_source = window.1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows(answer: &str, read: fn(&Value) -> HashMap<String, Window>) -> HashMap<String, Window> {
        read(&serde_json::from_str(answer).unwrap())
    }

    fn ollama_ps() -> &'static str {
        // Recorded shape of `GET /api/ps` at a server with two models pulled
        // and one loaded. `context_length` is at the top level of the model
        // object, as `api/types.go` tags it; `details` carries nothing here.
        r#"{"models":[
            {"name":"qwen3-coder:30b","model":"sha","size":1,"digest":"d",
             "details":{"family":"qwen3","parameter_size":"30.8B","quantization_level":"Q4_K_M"},
             "expires_at":"2026-09-20T10:00:00Z","size_vram":1,"context_length":65536},
            {"name":"llama3.3:70b","model":"sha","size":1,"digest":"d",
             "details":{"family":"llama"},
             "expires_at":"0001-01-01T00:00:00Z","size_vram":0}
        ]}"#
    }

    #[test]
    fn ollama_reports_a_window_only_for_what_it_actually_loaded() {
        let allocated = windows(ollama_ps(), ollama_allocated);

        let (value, source) = allocated["qwen3-coder:30b"];
        assert_eq!(value, 65_536);
        assert_eq!(source, ContextSource::DetectedAllocated);
        // Not loaded: the window is absent, and Ollama says nothing for it.
        assert!(!allocated.contains_key("llama3.3:70b"));
    }

    #[test]
    fn ollama_ps_without_the_field_claims_nothing() {
        // An older Ollama, or one with `num_ctx` unset the way docs show it.
        let answer = r#"{"models":[{"name":"qwen3-coder:30b","context_length":65536},
                                    {"name":"llama3.3:70b"}]}"#;
        let allocated = windows(answer, ollama_allocated);
        assert_eq!(allocated.len(), 1, "no field, no window");
    }

    #[test]
    fn ollama_show_answers_with_the_maximum_of_whatever_family_it_is() {
        let answer = serde_json::from_str::<Value>(
            r#"{"license":"...","modelfile":"...",
                "model_info":{
                    "general.architecture":"llama",
                    "llama.context_length":131072,
                    "llama.block_count":48,
                    "general.parameter_count":7060000000.0},
                "capabilities":["completion"]}"#,
        )
        .unwrap();
        assert_eq!(ollama_maximum(&answer), Some(131_072));
    }

    #[test]
    fn ollama_show_without_a_window_is_silence() {
        // A shape where the field is absent: an optimization-less build, or a
        // model format Ollama does not summarize.
        let answer = serde_json::from_str::<Value>(
            r#"{"model_info":{"general.architecture":"ggml","general.file_type":15}}"#,
        )
        .unwrap();
        assert_eq!(ollama_maximum(&answer), None);
    }

    #[test]
    fn lm_studio_prefers_what_it_actually_allocated() {
        let answer = serde_json::from_str::<Value>(
            r#"{"object":"list","data":[
                {"id":"loaded","max_context_length":262144,
                 "loaded_instances":[{"config":{"context_length":65536}}]},
                {"id":"unloaded","max_context_length":131072},
                {"id":"mystery"}
            ]}"#,
        )
        .unwrap();
        let ws = lm_studio_windows(&answer);

        let (value, source) = ws["loaded"];
        assert_eq!(value, 65_536, "allocated wins over the maximum");
        assert_eq!(source, ContextSource::DetectedAllocated);
        let (value, source) = ws["unloaded"];
        assert_eq!(value, 131_072);
        assert_eq!(source, ContextSource::DetectedMaximum);
        // Fields absent on LM Studio's side: the default stays a guess.
        assert!(!ws.contains_key("mystery"));
    }

    #[test]
    fn lm_studio_answer_without_a_window_list_says_nothing() {
        // Recorded shape of an LM Studio with nothing loaded and no sizes
        // attached: the fields are simply absent.
        let answer = serde_json::from_str::<Value>(
            r#"{"object":"list","data":[{"id":"qwen/qwen3-30b"}]}"#,
        )
        .unwrap();
        assert!(lm_studio_windows(&answer).is_empty());
    }

    #[test]
    fn lm_studio_loads_and_config_context_length_wins() {
        // An entry with a loaded instance and both numbers — the summary the
        // server actually makes.
        let answer = serde_json::from_str::<Value>(
            r#"{"data":[{"id":"m","max_context_length":262144,
                         "loaded_instances":[{"config":{"context_length":16384}}]}]}"#,
        )
        .unwrap();
        assert_eq!(lm_studio_windows(&answer)["m"].0, 16_384);
    }

    #[test]
    fn only_a_loopback_is_probed() {
        let mut spec = ProviderSpec::anthropic();
        spec.protocol = crate::provider::registry::Protocol::OpenAi;
        let mut base = |url: &str| {
            spec.base_url = url.to_string();
            is_local(&spec)
        };
        assert!(base("http://localhost:11434"));
        assert!(base("http://localhost"));
        assert!(base("http://127.0.0.1:1234"));
        assert!(base("http://[::1]:1234"));
        assert!(base("http://[0:0:0:0:0:0:0:1]:11434"));
        assert!(!base("https://api.openai.com"));
        assert!(!base("http://openai.local"));
        assert!(!base("http://192.168.1.10:11434"));
        // A host that never was a URL is as remote as any other.
        assert!(!base("not a url"));
    }
}

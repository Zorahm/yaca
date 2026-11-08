//! DeepSeek publishes one model catalog for Chat Completions and Messages.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use super::{get_at, http, parse, reasoning};
use crate::error::ProviderError;
use crate::provider::registry::{Protocol, ProviderSpec};
use crate::usage::{unknown_model, ContextSource, ModelFacts, ModelInfo};

pub(super) fn catalog_url(spec: &ProviderSpec) -> Option<&'static str> {
    // Match the configured address, so a provider renamed or repointed by the
    // user keeps using its own API and credentials.
    let url = Url::parse(&spec.base_url).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("api.deepseek.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    match (spec.protocol, url.path().trim_end_matches('/')) {
        (Protocol::Anthropic, "/anthropic" | "/anthropic/v1") | (Protocol::OpenAi, "" | "/v1") => {
            Some("https://api.deepseek.com/models")
        }
        _ => None,
    }
}

pub(super) async fn discover(
    client: &reqwest::Client,
    spec: &ProviderSpec,
    key: Option<&str>,
    url: &str,
) -> Result<Vec<ModelInfo>, ProviderError> {
    let page: DeepSeekPage =
        parse(get_at(client, spec, key, url, catalog_headers(spec, key)?).await?)?;
    Ok(page
        .data
        .into_iter()
        .map(|model| model.into_info(spec.protocol))
        .collect())
}

fn catalog_headers(spec: &ProviderSpec, key: Option<&str>) -> Result<HeaderMap, ProviderError> {
    let mut headers = http::header_map(spec, key)?;
    // The catalog uses Bearer auth. Honor an explicit Authorization header;
    // otherwise reuse the effective Messages key, including a custom x-api-key.
    let api_key = headers
        .remove("x-api-key")
        .filter(|value| !value.is_empty());
    if let Some(api_key) = api_key.filter(|_| !headers.contains_key(AUTHORIZATION)) {
        let mut bearer = b"Bearer ".to_vec();
        bearer.extend_from_slice(api_key.as_bytes());
        let mut value = HeaderValue::from_bytes(&bearer)
            .map_err(|e| ProviderError::Config(format!("header value «authorization»: {e}")))?;
        value.set_sensitive(true);
        headers.insert(AUTHORIZATION, value);
    }
    Ok(headers)
}

#[derive(Deserialize)]
struct DeepSeekPage {
    data: Vec<DeepSeekModel>,
}

#[derive(Deserialize)]
struct DeepSeekModel {
    id: String,
    name: Option<String>,
    context_window: Option<u32>,
    max_output_tokens: Option<u32>,
    input_modalities: Option<Vec<String>>,
    effort: Option<Value>,
}

impl DeepSeekModel {
    fn into_info(self, protocol: Protocol) -> ModelInfo {
        let mut model = unknown_model(&self.id);
        if let Some(name) = self.name {
            model.display_name = name;
        }
        if let Some(context) = self.context_window {
            model.context_max = context;
            model.context_source = ContextSource::ProviderReported;
        }
        if let Some(output) = self.max_output_tokens {
            model.max_output = output;
        }
        if let Some(inputs) = self.input_modalities {
            model.supports_images = inputs.iter().any(|input| input == "image");
            model.input_modalities = Some(inputs);
        }
        model.reasoning = self.effort.as_ref().and_then(reasoning::deepseek);
        let supports_effort = model
            .reasoning
            .as_ref()
            .and_then(|caps| caps.supported_efforts.as_ref())
            .map(|levels| !levels.is_empty());
        model.supports_effort = supports_effort.unwrap_or(false);
        model.supports_thinking = protocol == Protocol::Anthropic && model.reasoning.is_some();
        model.fetched = Some(ModelFacts {
            context_max: self.context_window,
            max_output: self.max_output_tokens,
            supports_effort,
            ..ModelFacts::of(&model)
        });
        model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::Effort;

    fn spec(protocol: Protocol, base_url: &str) -> ProviderSpec {
        ProviderSpec {
            protocol,
            base_url: base_url.into(),
            ..ProviderSpec::anthropic()
        }
    }

    #[test]
    fn official_addresses_share_the_catalog_but_keep_their_chat_routes() {
        for (protocol, base, chat_path, chat_url) in [
            (
                Protocol::Anthropic,
                "https://api.deepseek.com/anthropic",
                "/messages",
                "https://api.deepseek.com/anthropic/v1/messages",
            ),
            (
                Protocol::Anthropic,
                "https://api.deepseek.com/anthropic/v1/",
                "/messages",
                "https://api.deepseek.com/anthropic/v1/messages",
            ),
            (
                Protocol::OpenAi,
                "https://api.deepseek.com",
                "/chat/completions",
                "https://api.deepseek.com/v1/chat/completions",
            ),
            (
                Protocol::OpenAi,
                "https://api.deepseek.com/v1/",
                "/chat/completions",
                "https://api.deepseek.com/v1/chat/completions",
            ),
        ] {
            let spec = spec(protocol, base);
            assert_eq!(catalog_url(&spec), Some("https://api.deepseek.com/models"));
            assert_eq!(spec.api(chat_path), chat_url);
        }
    }

    #[test]
    fn a_provider_named_deepseek_cannot_redirect_a_custom_gateway_key() {
        for base in [
            "https://gateway.example/anthropic",
            "https://api.anthropic.com",
            "https://api.deepseek.com.gateway.example/anthropic",
            "https://api.deepseek.com:8443/anthropic",
            "http://api.deepseek.com/anthropic",
        ] {
            let mut spec = spec(Protocol::Anthropic, base);
            spec.id = "deepseek".into();
            assert_eq!(catalog_url(&spec), None, "{base}");
        }
    }

    #[test]
    fn the_catalog_uses_bearer_for_both_protocols_and_respects_overrides() {
        for protocol in [Protocol::Anthropic, Protocol::OpenAi] {
            let mut spec = spec(protocol, "https://api.deepseek.com");
            let headers = catalog_headers(&spec, Some("sk-test")).unwrap();
            assert_eq!(headers[AUTHORIZATION], "Bearer sk-test");
            assert!(!headers.contains_key("x-api-key"));
            assert!(!catalog_headers(&spec, None)
                .unwrap()
                .contains_key(AUTHORIZATION));

            spec.headers
                .insert("authorization".into(), "Bearer override-{key}".into());
            let headers = catalog_headers(&spec, Some("sk-test")).unwrap();
            assert_eq!(headers[AUTHORIZATION], "Bearer override-sk-test");
        }
        let mut spec = spec(Protocol::Anthropic, "https://api.deepseek.com/anthropic");
        spec.headers
            .insert("x-api-key".into(), "custom-{key}".into());
        let headers = catalog_headers(&spec, Some("sk-test")).unwrap();
        assert_eq!(headers[AUTHORIZATION], "Bearer custom-sk-test");
    }

    #[test]
    fn reads_deepseek_limits_vision_and_effort_without_guessing_model_ids() {
        let fixture = r#"{"object":"list","data":[
            {"id":"deepseek-flash","name":"DeepSeek-V4.1-Flash",
             "context_window":1048576,"max_output_tokens":393216,
             "input_modalities":["text","image"],
             "effort":{"supported_levels":["low","high","max"],"default_level":"high"}},
            {"id":"deepseek-v4-pro","input_modalities":["text"]}
        ]}"#;
        for protocol in [Protocol::Anthropic, Protocol::OpenAi] {
            let page: DeepSeekPage = serde_json::from_str(fixture).unwrap();
            let mut models = page.data.into_iter().map(|model| model.into_info(protocol));
            let flash = models.next().unwrap();
            assert_eq!(flash.display_name, "DeepSeek-V4.1-Flash");
            assert_eq!(flash.context_max, 1_048_576);
            assert_eq!(flash.context_source, ContextSource::ProviderReported);
            assert_eq!(flash.max_output, 393_216);
            assert!(flash.supports_images);
            assert_eq!(flash.supports_thinking, protocol == Protocol::Anthropic);
            assert_eq!(flash.request_effort(Effort::Max), Some(Effort::Max));
            assert_eq!(flash.request_effort(Effort::Medium), None);
            assert_eq!(flash.reasoning.unwrap().default_effort, Some(Effort::High));
            let pro = models.next().unwrap();
            assert!(!pro.supports_images);
            assert_eq!(pro.fetched.unwrap().max_output, None);
        }
    }
}

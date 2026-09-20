//! Что у двух протоколов общее: заголовки, разбор неуспешного ответа и
//! перекачка SSE в канал.
//!
//! Отдельным файлом, потому что различаются они только формой тела и разбором
//! событий. Скопированный второй раз `bytes_stream` — это два места, где чинить
//! обрыв соединения.

use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio_stream::wrappers::ReceiverStream;

use super::registry::ProviderSpec;
use super::sse::SseDecoder;
use super::{ProviderEvent, ProviderStream};
use crate::error::ProviderError;

/// Заголовки провайдера в форме reqwest.
///
/// Через `HeaderMap`, а не цепочкой `.header()`: описание провайдера может
/// перебивать умолчания протокола, а `.header()` добавляет второе значение
/// вместо замены — запрос уехал бы с двумя `authorization`.
pub(super) fn header_map(
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<HeaderMap, ProviderError> {
    let mut map = HeaderMap::new();
    for (name, value) in spec.headers_for(key) {
        let name = HeaderName::try_from(name.as_str())
            .map_err(|e| ProviderError::Config(format!("заголовок «{name}»: {e}")))?;
        let value = HeaderValue::try_from(value.as_str())
            .map_err(|e| ProviderError::Config(format!("значение заголовка «{name}»: {e}")))?;
        map.insert(name, value);
    }
    Ok(map)
}

/// Проверяет статус и превращает неуспех в ошибку с телом внутри.
///
/// `spec` and `key` are what the request went out with: a refusal of a
/// request sent without the key its description asks for says so.
pub(super) async fn check_status(
    resp: reqwest::Response,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> Result<reqwest::Response, ProviderError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }

    let retry_after_s = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    // Не `unwrap_or_default`: молча проглоченная причина превращается
    // в пустой блок ошибки, по которому человеку нечего решать.
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => format!("тело ответа не прочиталось: {e}"),
    };
    Err(ProviderError::Http {
        status: status.as_u16(),
        message: name_missing_key(extract_api_message(&text), status.as_u16(), spec, key),
        retry_after_s,
    })
}

/// A 401 or 403 to a request that went without its key names the key.
///
/// No key is not a refusal of its own (see `registry`): the request goes, and
/// the server decides. When it refuses, its own words — `invalid x-api-key`,
/// `Incorrect API key provided` — do not say that no key was sent at all, and
/// the fix is in Yaca's settings rather than at the provider.
fn name_missing_key(
    message: String,
    status: u16,
    spec: &ProviderSpec,
    key: Option<&str>,
) -> String {
    let keyless = key.is_none_or(|k| k.trim().is_empty());
    if keyless && spec.needs_key() && (status == 401 || status == 403) {
        format!("{message} — ключ API для «{}» не задан", spec.id)
    } else {
        message
    }
}

/// Текст ошибки из тела неуспешного ответа.
///
/// Форма `{"error":{"message":…}}` одна и та же у обоих протоколов — это
/// единственное, в чём они сошлись без уговора.
pub(super) fn extract_api_message(body: &str) -> String {
    if let Some(message) = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        })
    {
        return message;
    }
    let raw: String = body.trim().chars().take(400).collect();
    if raw.is_empty() {
        // Оборванный ответ бывает. Пустая строка в блоке ошибки (§5.10) —
        // не бывает: человеку нечего было бы прочитать.
        "провайдер не прислал подробностей".into()
    } else {
        raw
    }
}

/// Разбор одной формы потока.
///
/// Трейт, а не функция: OpenAI-форма не даёт ни индексов блоков, ни явного
/// конца сообщения, поэтому разбору нужна память между посылками и хвост,
/// который дописывается, когда поток кончился.
pub(super) trait StreamDecoder: Send + 'static {
    fn payload(&mut self, payload: &str) -> Result<Vec<ProviderEvent>, ProviderError>;

    /// Байты кончились. Форма, закрывающая сообщение сама, ничего не добавляет.
    fn finish(&mut self) -> Vec<ProviderEvent> {
        Vec::new()
    }
}

/// Перекачивает SSE в канал, разбирая каждую посылку.
///
/// Канал вместо ручного `Stream`: разбор SSE — это состояние поверх
/// состояния, и городить `poll_next` ради экономии одной задачи незачем.
pub(super) fn pump(resp: reqwest::Response, mut decode: impl StreamDecoder) -> ProviderStream {
    let (tx, rx) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
        let mut decoder = SseDecoder::new();
        let mut bytes = resp.bytes_stream();

        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(ProviderError::Network(e.to_string()))).await;
                    return;
                }
            };
            for payload in decoder.push(&chunk) {
                match decode.payload(&payload) {
                    Ok(events) => {
                        for ev in events {
                            if tx.send(Ok(ev)).await.is_err() {
                                return; // читатель ушёл — турн отменён
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        return;
                    }
                }
            }
        }

        for ev in decode.finish() {
            if tx.send(Ok(ev)).await.is_err() {
                return;
            }
        }
    });

    Box::pin(ReceiverStream::new(rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::registry::{Protocol, ProviderSpec};
    use std::collections::BTreeMap;

    fn spec(protocol: Protocol) -> ProviderSpec {
        ProviderSpec {
            id: "gateway".into(),
            display_name: "Шлюз".into(),
            protocol,
            base_url: "https://gate.example.com".into(),
            headers: BTreeMap::new(),
            models: vec![],
        }
    }

    #[test]
    fn достаёт_текст_ошибки_из_тела() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"нет"}}"#;
        assert_eq!(extract_api_message(body), "нет");
    }

    #[test]
    fn пустое_тело_не_даёт_пустого_сообщения() {
        assert!(!extract_api_message("").is_empty());
        assert_eq!(extract_api_message("Bad Gateway"), "Bad Gateway");
    }

    #[test]
    fn перебитый_заголовок_едет_один_раз() {
        let mut s = spec(Protocol::OpenAi);
        s.headers
            .insert("authorization".into(), "Basic svoj".into());
        let map = header_map(&s, None).unwrap();
        assert_eq!(map.get_all("authorization").iter().count(), 1);
        assert_eq!(map["authorization"], "Basic svoj");
    }

    #[test]
    fn негодное_имя_заголовка_это_ошибка_описания() {
        let mut s = spec(Protocol::OpenAi);
        s.headers.insert("не заголовок".into(), "x".into());
        assert!(matches!(
            header_map(&s, Some("k")),
            Err(ProviderError::Config(_))
        ));
    }
}

//! Разбор потока Chat Completions в нормализованные `ProviderEvent`.
//!
//! Здесь больше состояния, чем у Messages API, и не по прихоти:
//!
//! * Индексов блоков форма не даёт вовсе — их назначает этот разбор, и
//!   `ProviderEvent` требует монотонности.
//! * Границы блоков тоже не приходят. Блок открывается на первой дельте
//!   своего рода и закрывается, когда начинается другой род или кончился
//!   поток.
//! * Рассуждение зовётся `reasoning_content` у DeepSeek и Qwen, но
//!   `reasoning` у OpenRouter. Читаются оба: это одно и то же поле, о названии
//!   которого не договорились.
//! * `finish_reason` приезжает раньше `usage`, поэтому конец сообщения
//!   дописывается по концу потока, а не по нему.
//! * Шлюзы вроде OpenRouter и DeepSeek присылают весь набор полей, заполняя
//!   отсутствующее явным `null`. `serde(default)` покрывает только отсутствие
//!   поля, но не `null`, — структурам и векторам нужен свой обходной путь.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

use crate::error::ProviderError;
use crate::event::StopReason;
use crate::provider::http::StreamDecoder;
use crate::provider::{BlockKind, ProviderEvent};
use crate::usage::TokenCounts;

/// Посылка, которой поток объявляет о своём конце.
const DONE: &str = "[DONE]";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Open {
    Text,
    Thinking,
}

#[derive(Default)]
pub(super) struct Decoder {
    started: bool,
    finished: bool,
    /// Открытый блок текста или рассуждения и его индекс.
    open: Option<(Open, u32)>,
    /// Индекс вызова в посылке → индекс блока у нас. Форма нумерует вызовы
    /// своим счётом, который начинается с нуля рядом с текстом.
    calls: BTreeMap<u32, u32>,
    next_index: u32,
    stop: Option<StopReason>,
    usage: TokenCounts,
}

impl StreamDecoder for Decoder {
    fn payload(&mut self, payload: &str) -> Result<Vec<ProviderEvent>, ProviderError> {
        if payload.trim() == DONE {
            return Ok(self.finish());
        }

        let chunk: Chunk = serde_json::from_str(payload)
            .map_err(|e| ProviderError::Protocol(format!("{e}: {payload}")))?;

        if let Some(error) = chunk.error {
            return Err(ProviderError::Protocol(error.message));
        }

        let mut out = Vec::new();

        if !self.started {
            self.started = true;
            out.push(ProviderEvent::MessageStart {
                model: chunk.model.clone().unwrap_or_default(),
                usage: TokenCounts::default(),
            });
        }

        // Счётчики приезжают отдельной посылкой уже без `choices`.
        if let Some(usage) = chunk.usage {
            self.usage = usage.into();
        }

        for choice in chunk.choices {
            if let Some(text) = choice.delta.reasoning() {
                out.extend(self.open_block(Open::Thinking));
                let index = self.open.expect("блок только что открыт").1;
                out.push(ProviderEvent::ThinkingDelta { index, text });
            }

            if let Some(text) = choice.delta.content.filter(|t| !t.is_empty()) {
                out.extend(self.open_block(Open::Text));
                let index = self.open.expect("блок только что открыт").1;
                out.push(ProviderEvent::TextDelta { index, text });
            }

            for call in choice.delta.tool_calls {
                out.extend(self.tool_call(call));
            }

            if let Some(reason) = choice.finish_reason.as_deref() {
                self.stop = parse_stop_reason(reason);
            }
        }

        Ok(out)
    }

    /// Байты кончились. Некоторые шлюзы просто закрывают соединение, не
    /// присылая `[DONE]`, — хвост дописывается и в этом случае.
    fn finish(&mut self) -> Vec<ProviderEvent> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;

        let mut out = self.close_open();
        for (_, index) in std::mem::take(&mut self.calls) {
            out.push(ProviderEvent::BlockStop { index });
        }
        out.push(ProviderEvent::MessageDelta {
            stop_reason: self.stop,
            usage: self.usage,
        });
        out.push(ProviderEvent::MessageStop);
        out
    }
}

impl Decoder {
    /// Открывает блок нужного рода, закрыв предыдущий, если он другого.
    fn open_block(&mut self, kind: Open) -> Vec<ProviderEvent> {
        if self.open.map(|(k, _)| k) == Some(kind) {
            return Vec::new();
        }

        let mut out = self.close_open();
        let index = self.take_index();
        self.open = Some((kind, index));
        out.push(ProviderEvent::BlockStart {
            index,
            kind: match kind {
                Open::Text => BlockKind::Text,
                Open::Thinking => BlockKind::Thinking,
            },
        });
        out
    }

    fn close_open(&mut self) -> Vec<ProviderEvent> {
        match self.open.take() {
            Some((_, index)) => vec![ProviderEvent::BlockStop { index }],
            None => Vec::new(),
        }
    }

    fn tool_call(&mut self, call: WireToolCall) -> Vec<ProviderEvent> {
        let mut out = Vec::new();

        let index = match self.calls.get(&call.index) {
            Some(index) => *index,
            None => {
                // Первая дельта вызова несёт имя и идентификатор; дальше
                // приходят только аргументы.
                out.extend(self.close_open());
                let index = self.take_index();
                self.calls.insert(call.index, index);
                out.push(ProviderEvent::BlockStart {
                    index,
                    kind: BlockKind::ToolUse {
                        id: call.id.unwrap_or_else(|| format!("call_{index}")),
                        name: call.function.name.unwrap_or_default(),
                    },
                });
                index
            }
        };

        if let Some(arguments) = call.function.arguments.filter(|a| !a.is_empty()) {
            out.push(ProviderEvent::ToolInputDelta {
                index,
                partial_json: arguments,
            });
        }
        out
    }

    fn take_index(&mut self) -> u32 {
        let index = self.next_index;
        self.next_index += 1;
        index
    }
}

fn parse_stop_reason(s: &str) -> Option<StopReason> {
    Some(match s {
        "stop" => StopReason::EndTurn,
        "length" => StopReason::MaxTokens,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "content_filter" => StopReason::Refusal,
        _ => return None,
    })
}

/// Явный `null` там, где по уму поле просто отсутствует: шлюзы нулями
/// заполняют и векторы, и вложенные структуры. Читается как `Option`, чтобы
/// и `null`, и отсутствие поля превратились в `Default`.
fn null_as_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct Chunk {
    #[serde(default)]
    model: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
    /// Некоторые шлюзы кладут ошибку прямо в поток вместо статуса.
    #[serde(default)]
    error: Option<WireError>,
}

#[derive(Debug, Deserialize)]
struct WireChoice {
    #[serde(default, deserialize_with = "null_as_default")]
    delta: WireDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
    /// DeepSeek, Qwen и совместимые.
    #[serde(default)]
    reasoning_content: Option<String>,
    /// OpenRouter и совместимые.
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    tool_calls: Vec<WireToolCall>,
}

impl WireDelta {
    /// Одно поле под двумя именами; пустая строка — не дельта.
    fn reasoning(&self) -> Option<String> {
        self.reasoning_content
            .clone()
            .or_else(|| self.reasoning.clone())
            .filter(|t| !t.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct WireToolCall {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    function: WireFunction,
}

#[derive(Debug, Default, Deserialize)]
struct WireFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireError {
    #[serde(default)]
    message: String,
}

#[derive(Debug, Default, Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<WirePromptDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct WirePromptDetails {
    #[serde(default)]
    cached_tokens: u32,
}

impl From<WireUsage> for TokenCounts {
    fn from(u: WireUsage) -> Self {
        let cache_read = u
            .prompt_tokens_details
            .map(|d| d.cached_tokens)
            .unwrap_or(0);
        TokenCounts {
            // `prompt_tokens` считает и прочитанное из кеша: вычитаем, иначе
            // ввод посчитается дважды и деньги §7.1 будут завышены.
            input: u.prompt_tokens.saturating_sub(cache_read),
            output: u.completion_tokens,
            // Записи в кеш эта форма не сообщает вовсе.
            cache_write: 0,
            cache_read,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(decoder: &mut Decoder, payload: &str) -> Vec<ProviderEvent> {
        decoder.payload(payload).unwrap()
    }

    #[test]
    fn текст_открывает_и_закрывает_один_блок() {
        let mut d = Decoder::default();
        let first = decode(
            &mut d,
            r#"{"model":"gpt-oss","choices":[{"delta":{"content":"при"}}]}"#,
        );
        assert_eq!(
            first,
            vec![
                ProviderEvent::MessageStart {
                    model: "gpt-oss".into(),
                    usage: TokenCounts::default()
                },
                ProviderEvent::BlockStart {
                    index: 0,
                    kind: BlockKind::Text
                },
                ProviderEvent::TextDelta {
                    index: 0,
                    text: "при".into()
                },
            ]
        );

        let second = decode(&mut d, r#"{"choices":[{"delta":{"content":"вет"}}]}"#);
        assert_eq!(
            second,
            vec![ProviderEvent::TextDelta {
                index: 0,
                text: "вет".into()
            }]
        );
    }

    #[test]
    fn рассуждение_читается_под_обоими_именами() {
        let mut d = Decoder::default();
        decode(
            &mut d,
            r#"{"choices":[{"delta":{"reasoning_content":"ага"}}]}"#,
        );
        let events = decode(&mut d, r#"{"choices":[{"delta":{"reasoning":"угу"}}]}"#);
        assert_eq!(
            events,
            vec![ProviderEvent::ThinkingDelta {
                index: 0,
                text: "угу".into()
            }]
        );
    }

    #[test]
    fn рассуждение_и_текст_это_разные_блоки() {
        let mut d = Decoder::default();
        decode(&mut d, r#"{"choices":[{"delta":{"reasoning":"думаю"}}]}"#);
        let events = decode(&mut d, r#"{"choices":[{"delta":{"content":"ответ"}}]}"#);
        assert_eq!(
            events,
            vec![
                ProviderEvent::BlockStop { index: 0 },
                ProviderEvent::BlockStart {
                    index: 1,
                    kind: BlockKind::Text
                },
                ProviderEvent::TextDelta {
                    index: 1,
                    text: "ответ".into()
                },
            ]
        );
    }

    #[test]
    fn вызов_тула_склеивается_из_дельт() {
        let mut d = Decoder::default();
        decode(&mut d, r#"{"choices":[{"delta":{"content":"сейчас"}}]}"#);
        let start = decode(
            &mut d,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1",
               "function":{"name":"Read","arguments":"{\"pa"}}]}}]}"#,
        );
        assert_eq!(
            start,
            vec![
                ProviderEvent::BlockStop { index: 0 },
                ProviderEvent::BlockStart {
                    index: 1,
                    kind: BlockKind::ToolUse {
                        id: "call_1".into(),
                        name: "Read".into()
                    }
                },
                ProviderEvent::ToolInputDelta {
                    index: 1,
                    partial_json: "{\"pa".into()
                },
            ]
        );

        let more = decode(
            &mut d,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,
               "function":{"arguments":"th\":\"a\"}"}}]}}]}"#,
        );
        assert_eq!(
            more,
            vec![ProviderEvent::ToolInputDelta {
                index: 1,
                partial_json: "th\":\"a\"}".into()
            }]
        );
    }

    #[test]
    fn два_вызова_получают_разные_блоки() {
        let mut d = Decoder::default();
        decode(
            &mut d,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a",
               "function":{"name":"Glob"}},{"index":1,"id":"b",
               "function":{"name":"Grep"}}]}}]}"#,
        );
        assert_eq!(d.calls.get(&0), Some(&0));
        assert_eq!(d.calls.get(&1), Some(&1));
    }

    #[test]
    fn конец_потока_закрывает_блоки_и_сообщение() {
        let mut d = Decoder::default();
        decode(&mut d, r#"{"choices":[{"delta":{"content":"ответ"}}]}"#);
        decode(
            &mut d,
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        );
        decode(
            &mut d,
            r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":7,
               "prompt_tokens_details":{"cached_tokens":40}}}"#,
        );

        let tail = decode(&mut d, "[DONE]");
        assert_eq!(
            tail,
            vec![
                ProviderEvent::BlockStop { index: 0 },
                ProviderEvent::MessageDelta {
                    stop_reason: Some(StopReason::ToolUse),
                    usage: TokenCounts {
                        input: 60,
                        output: 7,
                        cache_write: 0,
                        cache_read: 40,
                    },
                },
                ProviderEvent::MessageStop,
            ]
        );

        // Оборвавшийся после `[DONE]` поток не дописывает хвост второй раз.
        assert_eq!(d.finish(), vec![]);
    }

    #[test]
    fn поток_без_done_всё_равно_закрывается() {
        let mut d = Decoder::default();
        decode(&mut d, r#"{"choices":[{"delta":{"content":"ответ"}}]}"#);
        let tail = d.finish();
        assert_eq!(tail.last(), Some(&ProviderEvent::MessageStop));
    }

    #[test]
    fn ошибка_в_потоке_становится_ошибкой() {
        let mut d = Decoder::default();
        let err = d
            .payload(r#"{"error":{"message":"перегружено"}}"#)
            .unwrap_err();
        assert!(matches!(err, ProviderError::Protocol(m) if m == "перегружено"));
    }

    #[test]
    fn незнакомое_поле_не_роняет_поток() {
        let mut d = Decoder::default();
        assert!(d
            .payload(r#"{"choices":[{"delta":{"refusal":null}}],"чего":1}"#)
            .is_ok());
    }

    #[test]
    fn явные_нулы_вместо_структур_и_векторов_не_роняют_разбор() {
        let mut d = Decoder::default();
        // Шлюзы нулями заполняют всё, чего в посылке нет: и поля, и целые
        // структуры с векторами.
        assert!(d.payload(r#"{"choices":null}"#).is_ok());
        assert!(d
            .payload(r#"{"choices":[{"delta":null,"finish_reason":null}]}"#)
            .is_ok());
        assert!(d
            .payload(
                r#"{"choices":[{"delta":{"tool_calls":null,
                   "content":"ответ"}}]}"#
            )
            .is_ok());
    }

    #[test]
    fn посылка_openrouter_с_явными_нулями_разбирается() {
        // Реальные байты от шлюза: весь набор полей, заполненный явными
        // null, — включая `tool_calls` и пустые `prompt_tokens_details`.
        let mut d = Decoder::default();
        let events = decode(
            &mut d,
            r#"{"id":"router-d6d029170e66d258051ba7ea8c2098de","object":"chat.completion.chunk","created":1788115438,"model":"deepseek-v4-flash","choices":[{"index":0,"finish_reason":null,"logprobs":null,"delta":{"role":null,"content":"При","reasoning_content":null,"tool_calls":null}}],"usage":{"prompt_tokens":2858,"completion_tokens":29,"total_tokens":2887,"prompt_tokens_details":{}}}"#,
        );
        assert_eq!(
            events,
            vec![
                ProviderEvent::MessageStart {
                    model: "deepseek-v4-flash".into(),
                    usage: TokenCounts::default()
                },
                ProviderEvent::BlockStart {
                    index: 0,
                    kind: BlockKind::Text
                },
                ProviderEvent::TextDelta {
                    index: 0,
                    text: "При".into()
                },
            ]
        );

        // Счётчики из той же посылки доезжают до конца сообщения; кеш в
        // пустых деталях — ноль.
        let tail = d.finish();
        assert!(matches!(
            tail.as_slice(),
            [
                ProviderEvent::BlockStop { index: 0 },
                ProviderEvent::MessageDelta {
                    usage: TokenCounts {
                        input: 2858,
                        output: 29,
                        cache_write: 0,
                        cache_read: 0
                    },
                    ..
                },
                ProviderEvent::MessageStop,
            ]
        ));
    }
}

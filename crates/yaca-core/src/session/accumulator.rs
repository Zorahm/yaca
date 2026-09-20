//! Склейка потока провайдера в блоки ответа и в события ленты.
//!
//! Отдельно от цикла турна намеренно: это чистое преобразование, у него нет
//! ни сети, ни файлов, ни времени, и проверяется оно построчно.
//!
//! Вызовы тулов накопитель не превращает в `ToolCallStarted`: §5.4 требует
//! подпись в настоящем времени в момент выполнения, а не в момент разбора
//! ответа. Их выдаёт цикл турна, когда действительно берётся за работу.

use std::collections::BTreeMap;

use crate::event::{AgentEvent, StopReason};
use crate::id::{BlockId, TurnId};
use crate::message::ContentBlock;
use crate::provider::{BlockKind, ProviderEvent};
use crate::usage::TokenCounts;

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    /// Why the arguments cannot be used, when they cannot. Such a call is not
    /// run, but it is in the history all the same and still has to be
    /// answered: the provider rejects a conversation with a call left
    /// without its result.
    pub garbled: Option<Garbled>,
}

/// Arguments that did not arrive as a JSON object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Garbled {
    /// The response hit its output cap while they were being written.
    Cut,
    /// Complete, but not a JSON object.
    Invalid,
}

#[derive(Debug)]
pub struct Completion {
    /// Блоки ответа в том виде, в каком они уедут обратно в историю.
    pub blocks: Vec<ContentBlock>,
    pub calls: Vec<PendingCall>,
    pub stop_reason: StopReason,
    pub usage: TokenCounts,
}

#[derive(Debug)]
enum Partial {
    Text {
        id: BlockId,
        text: String,
    },
    Thinking {
        id: BlockId,
        text: String,
        signature: Option<String>,
    },
    Redacted {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        json: String,
    },
}

#[derive(Debug)]
pub struct Accumulator {
    turn_id: TurnId,
    blocks: BTreeMap<u32, Partial>,
    stop_reason: Option<StopReason>,
    usage: TokenCounts,
}

impl Accumulator {
    pub fn new(turn_id: TurnId) -> Self {
        Accumulator {
            turn_id,
            blocks: BTreeMap::new(),
            stop_reason: None,
            usage: TokenCounts::default(),
        }
    }

    /// Скармливает событие провайдера, отдаёт события ленты.
    pub fn push(&mut self, event: ProviderEvent) -> Vec<AgentEvent> {
        match event {
            ProviderEvent::MessageStart { usage, .. } => {
                self.usage.add(usage);
                vec![]
            }

            ProviderEvent::BlockStart { index, kind } => {
                let partial = match kind {
                    BlockKind::Text => Partial::Text {
                        id: BlockId::new(),
                        text: String::new(),
                    },
                    BlockKind::Thinking => Partial::Thinking {
                        id: BlockId::new(),
                        text: String::new(),
                        signature: None,
                    },
                    BlockKind::RedactedThinking { data } => Partial::Redacted { data },
                    BlockKind::ToolUse { id, name } => Partial::ToolUse {
                        id,
                        name,
                        json: String::new(),
                    },
                };
                self.blocks.insert(index, partial);
                vec![]
            }

            ProviderEvent::TextDelta { index, text } => {
                let Some(Partial::Text { id, text: buf }) = self.blocks.get_mut(&index) else {
                    // Дельта без начала блока: провайдер соврал про индексы.
                    // Терять текст нельзя, поэтому заводим блок на месте.
                    return self.orphan_text(index, text);
                };
                buf.push_str(&text);
                vec![AgentEvent::TextDelta {
                    turn_id: self.turn_id.clone(),
                    block_id: id.clone(),
                    text,
                }]
            }

            ProviderEvent::ThinkingDelta { index, text } => {
                let Some(Partial::Thinking { id, text: buf, .. }) = self.blocks.get_mut(&index)
                else {
                    return vec![];
                };
                buf.push_str(&text);
                vec![AgentEvent::ThinkingDelta {
                    turn_id: self.turn_id.clone(),
                    block_id: id.clone(),
                    text,
                }]
            }

            ProviderEvent::SignatureDelta { index, signature } => {
                if let Some(Partial::Thinking { signature: sig, .. }) = self.blocks.get_mut(&index)
                {
                    // Подпись обязана уехать обратно без изменений, иначе
                    // следующий запрос будет отвергнут.
                    sig.get_or_insert_with(String::new).push_str(&signature);
                }
                vec![]
            }

            ProviderEvent::ToolInputDelta {
                index,
                partial_json,
            } => {
                if let Some(Partial::ToolUse { json, .. }) = self.blocks.get_mut(&index) {
                    json.push_str(&partial_json);
                }
                vec![]
            }

            ProviderEvent::BlockStop { .. } => vec![],

            ProviderEvent::MessageDelta { stop_reason, usage } => {
                self.usage.add(usage);
                if let Some(sr) = stop_reason {
                    self.stop_reason = Some(sr);
                }
                vec![]
            }

            ProviderEvent::MessageStop => vec![],
        }
    }

    fn orphan_text(&mut self, index: u32, text: String) -> Vec<AgentEvent> {
        let id = BlockId::new();
        self.blocks.insert(
            index,
            Partial::Text {
                id: id.clone(),
                text: text.clone(),
            },
        );
        vec![AgentEvent::TextDelta {
            turn_id: self.turn_id.clone(),
            block_id: id,
            text,
        }]
    }

    pub fn finish(self) -> Completion {
        let mut blocks = Vec::new();
        let mut calls = Vec::new();

        // Only the block being written when the cap hit can have been cut
        // short; an earlier call that does not parse was sent that way.
        let cut_at = (self.stop_reason == Some(StopReason::MaxTokens))
            .then(|| self.blocks.keys().next_back().copied())
            .flatten();

        for (index, partial) in self.blocks {
            match partial {
                Partial::Text { text, .. } => {
                    if !text.is_empty() {
                        blocks.push(ContentBlock::Text { text });
                    }
                }
                Partial::Thinking {
                    text, signature, ..
                } => blocks.push(ContentBlock::Thinking {
                    thinking: text,
                    signature,
                }),
                Partial::Redacted { data } => blocks.push(ContentBlock::RedactedThinking { data }),
                Partial::ToolUse { id, name, json } => {
                    // Пустая строка — законный случай: тул без аргументов.
                    let parsed = if json.trim().is_empty() {
                        Ok(serde_json::json!({}))
                    } else {
                        serde_json::from_str::<serde_json::Value>(&json)
                    };
                    // The history still needs an object here: the provider
                    // refuses a `tool_use` whose input is anything else.
                    let (input, garbled) = match parsed {
                        Ok(value) if value.is_object() => (value, None),
                        _ if cut_at == Some(index) => (serde_json::json!({}), Some(Garbled::Cut)),
                        _ => (serde_json::json!({}), Some(Garbled::Invalid)),
                    };
                    blocks.push(ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                    calls.push(PendingCall {
                        id,
                        name,
                        input,
                        garbled,
                    });
                }
            }
        }

        Completion {
            blocks,
            calls,
            // Оборванный поток без `message_delta` — считаем, что ход кончился.
            stop_reason: self.stop_reason.unwrap_or(StopReason::EndTurn),
            usage: self.usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::mock;

    fn feed(events: Vec<ProviderEvent>) -> (Vec<AgentEvent>, Completion) {
        let mut acc = Accumulator::new(TurnId::from("turn_1"));
        let mut out = Vec::new();
        for ev in events {
            out.extend(acc.push(ev));
        }
        (out, acc.finish())
    }

    #[test]
    fn текст_склеивается_в_один_блок_и_течёт_дельтами() {
        let (events, done) = feed(mock::script_text("привет"));

        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], AgentEvent::TextDelta { text, .. } if text == "привет"));
        assert_eq!(
            done.blocks,
            vec![ContentBlock::Text {
                text: "привет".into()
            }]
        );
        assert_eq!(done.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn дельты_одного_блока_делят_один_block_id() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        acc.push(ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::Text,
        });
        let a = acc.push(ProviderEvent::TextDelta {
            index: 0,
            text: "раз".into(),
        });
        let b = acc.push(ProviderEvent::TextDelta {
            index: 0,
            text: "два".into(),
        });

        let id = |e: &AgentEvent| match e {
            AgentEvent::TextDelta { block_id, .. } => block_id.clone(),
            _ => panic!("не дельта"),
        };
        assert_eq!(id(&a[0]), id(&b[0]));
        assert_eq!(
            acc.finish().blocks[0],
            ContentBlock::Text {
                text: "раздва".into()
            }
        );
    }

    #[test]
    fn аргументы_тула_склеиваются_из_кусков() {
        let (_, done) = feed(mock::script_tool_call(
            "toolu_1",
            "Read",
            serde_json::json!({ "path": "src/main.rs", "description": "Прочитал точку входа" }),
        ));

        assert_eq!(done.stop_reason, StopReason::ToolUse);
        assert_eq!(done.calls.len(), 1);
        assert_eq!(done.calls[0].name, "Read");
        assert_eq!(done.calls[0].input["path"], "src/main.rs");
    }

    #[test]
    fn битый_json_аргументов_не_роняет_турн() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        acc.push(ProviderEvent::BlockStart {
            index: 0,
            kind: BlockKind::ToolUse {
                id: "t1".into(),
                name: "Grep".into(),
            },
        });
        acc.push(ProviderEvent::ToolInputDelta {
            index: 0,
            partial_json: "{\"pattern\": ".into(),
        });
        let done = acc.finish();
        assert_eq!(done.calls[0].input, serde_json::json!({}));
        assert_eq!(done.calls[0].garbled, Some(Garbled::Invalid));
    }

    fn tool_block(acc: &mut Accumulator, index: u32, id: &str, json: &str) {
        acc.push(ProviderEvent::BlockStart {
            index,
            kind: BlockKind::ToolUse {
                id: id.into(),
                name: "Write".into(),
            },
        });
        acc.push(ProviderEvent::ToolInputDelta {
            index,
            partial_json: json.into(),
        });
        acc.push(ProviderEvent::BlockStop { index });
    }

    #[test]
    fn only_the_last_call_can_be_cut_by_the_output_cap() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "whole", r#"{"path": "a.txt", "content": "a"}"#);
        tool_block(&mut acc, 1, "broken", r#"{"path": "b.txt"}}"#);
        tool_block(&mut acc, 2, "cut", r#"{"path": "c.txt", "content": "cc"#);
        acc.push(ProviderEvent::MessageDelta {
            stop_reason: Some(StopReason::MaxTokens),
            usage: TokenCounts::default(),
        });

        let garbled: Vec<_> = acc.finish().calls.iter().map(|c| c.garbled).collect();
        // The broken one was finished before the cap came, so it was sent
        // broken: telling the model it ran out of room would be untrue.
        assert_eq!(garbled, vec![None, Some(Garbled::Invalid), Some(Garbled::Cut)]);
    }

    #[test]
    fn arguments_that_are_json_but_not_an_object_are_garbled() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "t1", r#"["a.txt"]"#);
        let done = acc.finish();
        assert_eq!(done.calls[0].garbled, Some(Garbled::Invalid));
        // The history still gets an object: the provider accepts nothing else.
        assert_eq!(done.calls[0].input, serde_json::json!({}));
    }

    #[test]
    fn a_tool_without_arguments_is_not_garbled() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        tool_block(&mut acc, 0, "t1", "");
        assert_eq!(acc.finish().calls[0].garbled, None);
    }

    #[test]
    fn подпись_рассуждения_сохраняется_для_отправки_обратно() {
        let (events, done) = feed(mock::script_thinking("думаю", "ответ"));

        assert!(matches!(events[0], AgentEvent::ThinkingDelta { .. }));
        assert!(matches!(events[1], AgentEvent::TextDelta { .. }));
        assert_eq!(
            done.blocks[0],
            ContentBlock::Thinking {
                thinking: "думаю".into(),
                signature: Some("sig".into()),
            }
        );
    }

    #[test]
    fn счётчики_суммируются_из_начала_и_конца_сообщения() {
        let (_, done) = feed(mock::script_text("x"));
        assert_eq!(done.usage.input, 100);
        assert_eq!(done.usage.output, 20);
    }

    #[test]
    fn дельта_без_начала_блока_не_теряет_текст() {
        let mut acc = Accumulator::new(TurnId::from("t"));
        let out = acc.push(ProviderEvent::TextDelta {
            index: 7,
            text: "осиротевший".into(),
        });
        assert_eq!(out.len(), 1);
        assert_eq!(
            acc.finish().blocks[0],
            ContentBlock::Text {
                text: "осиротевший".into()
            }
        );
    }
}

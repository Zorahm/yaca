//! Токены и деньги. Кольцо состояния (§7.1) показывает и то и другое:
//! заполнение — занятость контекста, тултип — `24k/200k · $0.31`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::provider::registry::ANTHROPIC_ID;

/// Сырые счётчики одного ответа провайдера.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    pub input: u32,
    pub output: u32,
    /// Записано в кеш — считается дороже обычного ввода.
    pub cache_write: u32,
    /// Прочитано из кеша — считается почти даром.
    pub cache_read: u32,
}

impl TokenCounts {
    pub fn add(&mut self, other: TokenCounts) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }

    /// Сколько всего занято в окне контекста прямо сейчас.
    ///
    /// Считается по последнему ответу, а не суммой за сессию: история
    /// пересылается целиком каждый ход, и сумма ничего не значила бы.
    pub fn context_used(&self) -> u32 {
        self.input + self.cache_read + self.cache_write + self.output
    }
}

/// Что интерфейс кладёт в кольцо состояния.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    /// Токены последнего ответа.
    pub last: TokenCounts,
    /// Токены за всю сессию — для `/cost`.
    pub total: TokenCounts,
    /// Занято в окне контекста.
    pub context_used: u32,
    /// Размер окна выбранной модели.
    pub context_max: u32,
    /// Накопленная стоимость сессии в долларах.
    pub cost_usd: f64,
}

/// Цена модели, доллары за миллион токенов.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
}

impl Pricing {
    /// Множители кеша у Anthropic: запись ~1.25× от ввода, чтение ~0.1×.
    pub fn cost(&self, t: TokenCounts) -> f64 {
        let m = 1e-6;
        t.input as f64 * self.input * m
            + t.output as f64 * self.output * m
            + t.cache_write as f64 * self.input * 1.25 * m
            + t.cache_read as f64 * self.input * 0.10 * m
    }
}

/// Модель, доступная в переключателе (§7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub provider_id: String,
    pub display_name: String,
    pub context_max: u32,
    pub max_output: u32,
    pub pricing: Pricing,
    /// Умеет ли `output_config.effort`. Старые модели — нет.
    pub supports_effort: bool,
    /// Умеет ли адаптивное рассуждение — единственную форму, которую Yaca
    /// умеет просить. Отдельно от усилия, потому что провайдер сообщает их
    /// отдельно: Haiku 4.5 рассуждает по `budget_tokens`, но `effort` не
    /// понимает, и один флаг на двоих отправил бы ей то, чего она не примет.
    ///
    /// `default` — ради `providers.json`, написанного до того, как поле
    /// появилось: битый файл здесь останавливает запуск, и молча уронить
    /// чужую настройку на обновлении нельзя. Умолчание `false` — сторона
    /// отказа: недосланное рассуждение стоит глубины, лишнее — это 400.
    #[serde(default)]
    pub supports_thinking: bool,
}

/// Прайс-лист Anthropic, доллары за миллион токенов.
///
/// Снимок, а не истина: цены Models API не отдаёт вовсе (`provider::models`),
/// а платная модель, посчитанная бесплатной, — враньё в кольце §7.1. Модель
/// не из списка считается бесплатной, как и всякая неизвестная, — это видно
/// по нулю в тултипе, и это честнее выдуманной цифры.
///
/// Сверяется по началу идентификатора: датированный снимок
/// `claude-opus-4-5-20251101` попадает в строку `claude-opus-4-5`.
const PRICES: &[(&str, f64, f64)] = &[
    ("claude-fable-5-1", 10.0, 50.0),
    ("claude-fable-5", 10.0, 50.0),
    ("claude-mythos-5-1", 10.0, 50.0),
    ("claude-mythos-5", 10.0, 50.0),
    ("claude-opus-5", 5.0, 25.0),
    ("claude-opus-4-8", 5.0, 25.0),
    ("claude-opus-4-7", 5.0, 25.0),
    ("claude-opus-4-6", 5.0, 25.0),
    ("claude-sonnet-5", 2.0, 10.0),
    ("claude-sonnet-4-6", 3.0, 15.0),
    ("claude-haiku-4-5", 1.0, 5.0),
];

/// Цена модели по прайс-листу, если она там есть.
///
/// Самый длинный подходящий префикс, а не первый: `claude-fable-5` — начало
/// `claude-fable-5-1`, и короткая строка не должна перебивать точную.
pub fn price_of(id: &str) -> Option<Pricing> {
    PRICES
        .iter()
        .filter(|(prefix, _, _)| id.starts_with(prefix))
        .max_by_key(|(prefix, _, _)| prefix.len())
        .map(|(_, input, output)| Pricing {
            input: *input,
            output: *output,
        })
}

/// The models a session built by hand starts with: tests and the mock
/// provider. No provider offers them. A provider's models are what its address
/// answered (§10), and a list written here made a provider nobody had set up
/// look configured.
pub fn catalog() -> Vec<ModelInfo> {
    let m = |id: &str, name: &str, ctx: u32, out: u32| ModelInfo {
        id: id.to_string(),
        provider_id: ANTHROPIC_ID.to_string(),
        display_name: name.to_string(),
        context_max: ctx,
        max_output: out,
        pricing: price_of(id).unwrap_or(Pricing {
            input: 0.0,
            output: 0.0,
        }),
        supports_effort: true,
        supports_thinking: true,
    };
    vec![
        m("claude-opus-5", "Opus 5", 1_000_000, 128_000),
        m("claude-opus-4-8", "Opus 4.8", 1_000_000, 128_000),
        m("claude-sonnet-5", "Sonnet 5", 1_000_000, 128_000),
        // Haiku 4.5 — модель прошлого поколения: ни `output_config.effort`,
        // ни адаптивного рассуждения она не понимает и отвечает 400.
        ModelInfo {
            supports_effort: false,
            supports_thinking: false,
            ..m("claude-haiku-4-5", "Haiku 4.5", 200_000, 64_000)
        },
    ]
}

pub const DEFAULT_MODEL: &str = "claude-opus-5";

pub fn model_info(id: &str) -> Option<ModelInfo> {
    catalog().into_iter().find(|m| m.id == id)
}

/// Модель, которой нет в списке провайдера. Считаем бесплатной и с
/// консервативным окном, но не падаем: §8 обещает работу с любыми моделями,
/// включая те, о которых Yaca не слышал.
pub fn unknown_model(id: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        provider_id: String::new(),
        display_name: id.to_string(),
        context_max: 128_000,
        max_output: 8_192,
        pricing: price_of(id).unwrap_or(Pricing {
            input: 0.0,
            output: 0.0,
        }),
        supports_effort: false,
        supports_thinking: false,
    }
}

/// Накопитель за сессию.
#[derive(Debug, Clone, Default)]
pub struct UsageMeter {
    total: TokenCounts,
    last: TokenCounts,
    cost_usd: f64,
    /// Set by a compaction, cleared by the next response (§5.11).
    ///
    /// The ring shows how full the window is, and after a compaction it is not
    /// as full as it was. Waiting for the provider to confirm that would leave
    /// the ring — and §6.1.1's bar — insisting the context is nearly full for
    /// as long as the human takes to write the next message.
    estimated_context: Option<u32>,
}

impl UsageMeter {
    /// Continues the count of a session being reopened (§9).
    ///
    /// `last` stays empty: it is the size of the previous response, and until
    /// the model answers again there has been no response in this run. The
    /// ring fills up on the first answer; the money is right from the start.
    pub fn resumed(total: TokenCounts, cost_usd: f64) -> Self {
        UsageMeter {
            total,
            last: TokenCounts::default(),
            cost_usd,
            estimated_context: None,
        }
    }

    /// The window shrank and no response has measured it yet (§5.11).
    ///
    /// An estimate standing in for a fact, and only until the fact arrives:
    /// the money is untouched, and the next response replaces this outright.
    pub fn shrank_by(&mut self, freed: u32) {
        let now = self
            .estimated_context
            .unwrap_or_else(|| self.last.context_used());
        self.estimated_context = Some(now.saturating_sub(freed));
    }

    pub fn record(&mut self, counts: TokenCounts, pricing: Pricing) {
        self.last = counts;
        self.record_aside(counts, pricing);
        // The provider has now measured the window; the estimate is spent.
        self.estimated_context = None;
    }

    /// A request that is not the conversation — the summarizing pass of §5.11.
    ///
    /// Money and totals, but not `last`: the ring shows how full the window
    /// is, and a summarizer answering in two hundred tokens did not empty it.
    pub fn record_aside(&mut self, counts: TokenCounts, pricing: Pricing) {
        self.total.add(counts);
        self.cost_usd += pricing.cost(counts);
    }

    pub fn snapshot(&self, context_max: u32) -> UsageSnapshot {
        UsageSnapshot {
            last: self.last,
            total: self.total,
            context_used: self
                .estimated_context
                .unwrap_or_else(|| self.last.context_used()),
            context_max,
            cost_usd: self.cost_usd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn кеш_дешевле_свежего_ввода() {
        let p = Pricing {
            input: 5.0,
            output: 25.0,
        };
        let свежий = p.cost(TokenCounts {
            input: 1_000_000,
            ..Default::default()
        });
        let из_кеша = p.cost(TokenCounts {
            cache_read: 1_000_000,
            ..Default::default()
        });
        assert_eq!(свежий, 5.0);
        assert!((из_кеша - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_side_request_costs_money_without_moving_the_ring() {
        let p = Pricing {
            input: 5.0,
            output: 25.0,
        };
        let mut meter = UsageMeter::default();
        meter.record(
            TokenCounts {
                input: 100_000,
                ..Default::default()
            },
            p,
        );
        let before = meter.snapshot(200_000);

        // Summarizing is a request of its own, not the conversation.
        meter.record_aside(
            TokenCounts {
                output: 500,
                ..Default::default()
            },
            p,
        );
        let after = meter.snapshot(200_000);

        assert_eq!(after.context_used, before.context_used);
        assert!(after.cost_usd > before.cost_usd);
        assert_eq!(after.total.output, 500);
    }

    #[test]
    fn compaction_moves_the_ring_before_the_provider_confirms_it() {
        let mut meter = UsageMeter::default();
        meter.record(
            TokenCounts {
                input: 100_000,
                ..Default::default()
            },
            Pricing {
                input: 5.0,
                output: 25.0,
            },
        );
        let spent = meter.snapshot(200_000).cost_usd;

        meter.shrank_by(80_000);
        assert_eq!(meter.snapshot(200_000).context_used, 20_000);
        assert_eq!(
            meter.snapshot(200_000).cost_usd,
            spent,
            "деньги не возвращаются"
        );

        // The next answer is measured, not estimated.
        meter.record(
            TokenCounts {
                input: 25_000,
                ..Default::default()
            },
            Pricing {
                input: 5.0,
                output: 25.0,
            },
        );
        assert_eq!(meter.snapshot(200_000).context_used, 25_000);
    }

    #[test]
    fn неизвестная_модель_не_роняет_учёт() {
        let info = unknown_model("qwen2.5-coder:32b");
        assert_eq!(
            info.pricing.cost(TokenCounts {
                input: 1_000_000,
                ..Default::default()
            }),
            0.0
        );
        assert!(!info.supports_effort);
    }
}

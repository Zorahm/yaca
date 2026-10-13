//! Tokens and money. The state ring (§7.1) shows both: the fill is how much of
//! the context is occupied, the tooltip is `24k/200k · $0.31`.

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
    /// Written to the cache — billed higher than ordinary input.
    pub cache_write: u32,
    /// Read from the cache — billed at almost nothing.
    pub cache_read: u32,
}

impl TokenCounts {
    pub fn add(&mut self, other: TokenCounts) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }

    /// How much of the context window is occupied right now.
    ///
    /// Computed from the last response, not summed over the session: the history
    /// is sent again in full each turn, so a sum would mean nothing.
    pub fn context_used(&self) -> u32 {
        self.input + self.cache_read + self.cache_write + self.output
    }
}

/// What the occupied part of the context window is made of, in tokens.
///
/// The provider measures one number for the whole request, so the split is an
/// estimate: that number shared out by the size of each part's text
/// (`session::window`). The parts other than `reserved` add up to
/// `UsageSnapshot::context_used` exactly, so the list never disagrees with
/// the figure above it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ContextBreakdown {
    /// The core's own system prompt, without the sections counted below.
    pub system_prompt: u32,
    /// `AGENTS.md` at the root and in the directories below it.
    pub instructions: u32,
    /// The project's memory index.
    pub memory: u32,
    /// The project's skills list.
    pub skills: u32,
    /// Schemas of the core's own tools.
    pub tools: u32,
    /// Schemas of the tools the MCP servers brought.
    pub mcp_tools: u32,
    /// What the MCP servers say about themselves in the system prompt.
    pub mcp_notes: u32,
    /// The conversation: messages, calls and their results.
    pub messages: u32,
    /// Not occupied, but not free either: the room kept for the answer and
    /// the margin before compaction starts on its own (§5.11). Outside
    /// `context_used`.
    pub reserved: u32,
}

/// What the interface puts into the state ring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    /// Tokens of the last response.
    pub last: TokenCounts,
    /// Tokens for the whole session — for `/cost`.
    pub total: TokenCounts,
    /// Occupied in the context window.
    pub context_used: u32,
    /// Size of the selected model's window.
    pub context_max: u32,
    /// What `context_used` is made of. All zeroes in a snapshot recorded
    /// before the field existed, and until the session's worker fills it.
    #[serde(default)]
    pub breakdown: ContextBreakdown,
    /// Accumulated session cost in dollars.
    pub cost_usd: f64,
}

/// Model price, dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
}

impl Pricing {
    /// Anthropic's cache multipliers: writes ~1.25× the input price, reads ~0.1×.
    pub fn cost(&self, t: TokenCounts) -> f64 {
        let m = 1e-6;
        t.input as f64 * self.input * m
            + t.output as f64 * self.output * m
            + t.cache_write as f64 * self.input * 1.25 * m
            + t.cache_read as f64 * self.input * 0.10 * m
    }
}

/// Where the context window came from.
///
/// How the number was obtained, not the number itself: the difference between
/// "the server answered", "the core asked the local server what the OpenAI
/// shape leaves out" and "the number was made up by a default" is the visibility
/// of a fallback that §10 asks for. An old `providers.json` has no such field —
/// `default` keeps a guess recorded there a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// The server said nothing of what `provider::models` names — the
    /// `unknown_model`'s 128 000 stands, and it is a guess, not a fact.
    #[default]
    DefaultGuess,
    /// The provider answered with a number from its own model list.
    ProviderReported,
    /// The local server answered with how much it actually allocated to the
    /// loaded model (`provider::models::local`).
    DetectedAllocated,
    /// The local server answered with the model's architectural maximum: what a
    /// fresh load will get, but not necessarily what the session runs under.
    DetectedMaximum,
}

/// A model available in the switcher (§7.1).
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
    /// Whether it supports `output_config.effort`. Older models do not.
    pub supports_effort: bool,
    /// Whether it supports adaptive reasoning — the only form Yaca knows how to
    /// ask for. Kept apart from effort, because the provider reports them
    /// separately: Haiku 4.5 reasons by `budget_tokens` but does not understand
    /// `effort`, and one flag for both would send it something it refuses.
    ///
    /// `default` is for `providers.json` written before the field existed: a
    /// broken file stops the start here, and silently dropping someone's setting
    /// on an update is not allowed. The `false` default is the refusing side: a
    /// reasoning that was not sent costs depth, an extra one is a 400.
    #[serde(default)]
    pub supports_thinking: bool,
    /// How `context_max` came to be. A fallback stays visible: see
    /// [`ContextSource`].
    #[serde(default)]
    pub context_source: ContextSource,
    /// Whether the model can be shown an image.
    ///
    /// Silence is a yes here, the opposite of the two flags above, because
    /// the costs are the other way round. A model wrongly taken to see
    /// refuses one request, which is sent again without the images
    /// (`session::vision`); a model wrongly taken to be blind is never shown
    /// a screenshot it could have read, and nothing says why.
    #[serde(default = "yes")]
    pub supports_images: bool,
}

fn yes() -> bool {
    true
}

/// Anthropic's price list, dollars per million tokens.
///
/// A snapshot, not the truth: the Models API does not return prices at all
/// (`provider::models`), and a paid model counted as free is a lie in the §7.1
/// ring. A model not on the list counts as free, like any unknown one — this
/// shows as a zero in the tooltip, which is more honest than an invented figure.
///
/// Matched by the start of the identifier: a dated snapshot
/// `claude-opus-4-5-20251101` falls under the `claude-opus-4-5` row.
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

/// The model's price from the price list, if it is there.
///
/// The longest matching prefix, not the first: `claude-fable-5` is the start of
/// `claude-fable-5-1`, and the shorter string must not override the exact one.
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
        // Written down, not answered — but a stated constant, not a guess.
        context_source: ContextSource::ProviderReported,
        supports_images: true,
    };
    vec![
        m("claude-opus-5", "Opus 5", 1_000_000, 128_000),
        m("claude-opus-4-8", "Opus 4.8", 1_000_000, 128_000),
        m("claude-sonnet-5", "Sonnet 5", 1_000_000, 128_000),
        // Haiku 4.5 is a previous-generation model: it understands neither
        // `output_config.effort` nor adaptive reasoning, and answers 400.
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

/// A model that is not in the provider's list. Counted as free and with a
/// conservative window, but it does not fail: §8 promises to work with any
/// model, including ones Yaca has never heard of.
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
        context_source: ContextSource::DefaultGuess,
        supports_images: true,
    }
}

/// Accumulator for the session.
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
            // The meter counts tokens and knows nothing of what was sent;
            // the worker does (`session::window`).
            breakdown: ContextBreakdown::default(),
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

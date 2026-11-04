//! The session-level shapes: token counts and the context window, the models a
//! provider offers, the metadata a chat is listed by, the settings a chat shows
//! in its bottom row, and the chat summaries of the host.

use yaca_core::event::SessionState;
use yaca_core::isolation::Isolation;
use yaca_core::usage::{ContextBreakdown, ModelInfo, Pricing, TokenCounts, UsageSnapshot};
use yaca_core::SessionMeta;
use yaca_remote::protocol::ChatSummary;

use crate::ids::{self, PROJECT};
use crate::variants;

pub fn token_counts(n: u32) -> TokenCounts {
    TokenCounts {
        input: 1_200 * n,
        output: 340 * n,
        cache_write: 0,
        cache_read: 900 * n,
    }
}

/// A context window whose parts add up to the used figure: the reserved part
/// sits outside it, as the core reports it.
pub fn usage_snapshot() -> UsageSnapshot {
    UsageSnapshot {
        last: token_counts(1),
        total: token_counts(4),
        context_used: 48_220,
        context_max: 200_000,
        breakdown: ContextBreakdown {
            system_prompt: 3_100,
            instructions: 900,
            memory: 0,
            skills: 420,
            tools: 6_800,
            mcp_tools: 0,
            mcp_notes: 0,
            messages: 37_000,
            reserved: 32_000,
        },
        cost_usd: 0.0427,
    }
}

/// One model per context source, so each source is written. The two flags that
/// vary are set on alternating models.
pub fn model_infos() -> Vec<ModelInfo> {
    variants::context_sources()
        .into_iter()
        .enumerate()
        .map(|(i, context_source)| ModelInfo {
            id: format!("model-{i}"),
            provider_id: "anthropic".to_owned(),
            display_name: format!("Model {i}"),
            context_max: 200_000,
            max_output: 64_000,
            pricing: Pricing {
                input: 3.0,
                output: 15.0,
            },
            supports_effort: i % 2 == 0,
            supports_thinking: true,
            context_source,
            supports_images: i % 2 == 0,
            pricing_by_hand: false,
            supported_parameters: None,
            input_modalities: None,
            expiration_date: None,
            cache_read: None,
            cache_write: None,
            by_hand: false,
            fetched: None,
            overrides: None,
            enabled: true,
        })
        .collect()
}

pub fn session_metas() -> Vec<SessionMeta> {
    vec![
        SessionMeta {
            id: ids::session(1),
            title: "Greeting rename".to_owned(),
            title_custom: false,
            title_by_model: true,
            created_at: ids::at(0),
            updated_at: ids::at(600_000),
            provider: "anthropic".to_owned(),
            model: "model-0".to_owned(),
            messages: 3,
            cost_usd: 0.0427,
            tokens: token_counts(4),
            files: vec!["src/greet.rs".to_owned()],
            forked_from: None,
            additional_roots: vec![],
        },
        SessionMeta {
            id: ids::session(2),
            title: "Renamed by hand".to_owned(),
            title_custom: true,
            title_by_model: false,
            created_at: ids::at(900_000),
            updated_at: ids::at(950_000),
            provider: "anthropic".to_owned(),
            model: "model-1".to_owned(),
            messages: 1,
            cost_usd: 0.0,
            tokens: TokenCounts::default(),
            files: vec![],
            forked_from: Some(ids::session(1)),
            additional_roots: vec!["D:/shared-libs".to_owned()],
        },
    ]
}

/// The bottom row of a chat, one per mode with the efforts cycling through
/// theirs. Isolation, busy and bypass each come in both states.
pub fn session_states() -> Vec<SessionState> {
    variants::modes()
        .into_iter()
        .enumerate()
        .map(|(i, mode)| {
            let effort = variants::efforts()
                .into_iter()
                .nth(i)
                .expect("one effort per mode");
            let isolated = i % 2 == 1;
            SessionState {
                project_root: format!("C:/Projects/{PROJECT}"),
                provider: "anthropic".to_owned(),
                model: format!("model-{i}"),
                mode,
                // Every profile is written, alternating with the mode.
                profile: variants::profiles()[i % 2],
                effort,
                busy: i % 2 == 0,
                bypass: i == 3,
                isolation: isolated.then(|| Isolation {
                    branch: "yaca/session-2".to_owned(),
                    path: format!("C:/Projects/{PROJECT}-wt"),
                }),
            }
        })
        .collect()
}

/// The chats running on the host: one with its metadata, one without.
pub fn chat_summaries() -> Vec<ChatSummary> {
    session_states()
        .into_iter()
        .enumerate()
        .map(|(i, state)| {
            let n = i as u32 + 1;
            ChatSummary {
                id: ids::session(n),
                project: PROJECT.to_owned(),
                state,
                meta: (i % 2 == 0).then(|| session_metas().remove(0)),
            }
        })
        .collect()
}

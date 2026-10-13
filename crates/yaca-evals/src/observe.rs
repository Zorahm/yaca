//! Folding the event stream of one run into what the result reports.
//!
//! The stream is the only thing the core exposes, so every number here is
//! read from it. Tokens and cost come from `UsageUpdated.usage.total`, the
//! session total the core keeps itself, rather than from summing per-response
//! figures this file would have to get right on its own.

use std::collections::BTreeMap;

use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::usage::TokenCounts;
use yaca_core::{ProcessId, ProcessState};

/// What one run did, as the event stream shows it.
#[derive(Debug, Default)]
pub struct Observed {
    pub turns: u32,
    pub tool_calls: BTreeMap<String, u32>,
    pub tokens: TokenCounts,
    pub cost_usd: f64,
    pub errors: Vec<String>,
    pub permissions_denied: u32,
    pub questions_dismissed: u32,
    /// How the turn ended: a [`StopReason`] in its wire spelling, or
    /// `cancelled`. `None` while the turn is still going.
    pub ending: Option<String>,
    /// Background processes the core still reports as running. The run stops
    /// them before it lets go of the session, so a dev server the agent started
    /// does not outlive the tree it was started in.
    pub running_processes: Vec<ProcessId>,
    /// Every event as one JSON line, written next to the result for debugging.
    pub log: Vec<String>,
}

impl Observed {
    pub fn observe(&mut self, event: &AgentEvent) {
        if let Ok(line) = serde_json::to_string(event) {
            self.log.push(line);
        }
        match event {
            AgentEvent::TurnStarted { .. } => self.turns += 1,
            AgentEvent::ToolCallStarted { tool, .. } => {
                *self.tool_calls.entry(tool.clone()).or_default() += 1;
            }
            AgentEvent::UsageUpdated { usage } => {
                self.tokens = usage.total;
                self.cost_usd = usage.cost_usd;
            }
            AgentEvent::Error { code, message, .. } => {
                self.errors.push(format!("{code}: {message}"));
            }
            AgentEvent::ProcessesUpdated { processes } => {
                self.running_processes = processes
                    .iter()
                    .filter(|p| matches!(p.state, ProcessState::Running))
                    .map(|p| p.id)
                    .collect();
            }
            AgentEvent::TurnFinished { stop_reason, .. } => {
                self.ending = Some(stop_reason_name(*stop_reason));
            }
            AgentEvent::TurnCancelled { .. } => self.ending = Some("cancelled".into()),
            _ => {}
        }
    }

    pub fn total_tool_calls(&self) -> u32 {
        self.tool_calls.values().sum()
    }
}

/// The wire spelling of a stop reason (`endTurn`, `maxTokens`, ...), so the
/// result reads the way the event stream does.
pub fn stop_reason_name(reason: StopReason) -> String {
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{reason:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaca_core::id::{CallId, TurnId};
    use yaca_core::usage::UsageSnapshot;

    fn tool(name: &str) -> AgentEvent {
        AgentEvent::ToolCallStarted {
            turn_id: TurnId::from("t".to_string()),
            call_id: CallId::from("c".to_string()),
            tool: name.into(),
            description: None,
            target: None,
            input: serde_json::Value::Null,
            parent: None,
        }
    }

    #[test]
    fn counts_tools_by_name_and_keeps_the_last_usage() {
        let mut obs = Observed::default();
        obs.observe(&tool("Read"));
        obs.observe(&tool("Read"));
        obs.observe(&tool("Edit"));
        obs.observe(&AgentEvent::UsageUpdated {
            usage: UsageSnapshot {
                total: TokenCounts { input: 10, output: 2, ..Default::default() },
                cost_usd: 0.25,
                ..Default::default()
            },
        });
        obs.observe(&AgentEvent::UsageUpdated {
            usage: UsageSnapshot {
                total: TokenCounts { input: 30, output: 5, ..Default::default() },
                cost_usd: 0.5,
                ..Default::default()
            },
        });
        assert_eq!(obs.tool_calls.get("Read"), Some(&2));
        assert_eq!(obs.tool_calls.get("Edit"), Some(&1));
        assert_eq!(obs.total_tool_calls(), 3);
        assert_eq!(obs.tokens.input, 30);
        assert_eq!(obs.cost_usd, 0.5);
        assert_eq!(obs.log.len(), 5);
    }

    #[test]
    fn the_turn_ending_is_named_as_the_wire_names_it() {
        let mut obs = Observed::default();
        obs.observe(&AgentEvent::TurnFinished {
            turn_id: TurnId::from("t".to_string()),
            stop_reason: StopReason::EndTurn,
            at: Default::default(),
        });
        assert_eq!(obs.ending.as_deref(), Some("endTurn"));
        obs.observe(&AgentEvent::TurnCancelled {
            turn_id: TurnId::from("t".to_string()),
            at: Default::default(),
        });
        assert_eq!(obs.ending.as_deref(), Some("cancelled"));
    }
}

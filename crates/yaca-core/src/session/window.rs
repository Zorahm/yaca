//! What the context window is filled with (§7.1).
//!
//! The provider answers with one number for the whole request. The interface
//! shows what that number is made of, and the only thing known about the parts
//! is their text — so the measured total is shared out by the size of each
//! part's text. The total stays a fact and the split is an estimate; the parts
//! always add up to the total, so the two cannot disagree on screen.
//!
//! Requests and usage snapshots share the same prepared prompt and schemas.
//! The conversation's weight is still measured on every snapshot.

use crate::intent::Intent;
use crate::mcp::TOOL_PREFIX;
use crate::profile::Profile;
use crate::provider::ToolSchema;
use crate::usage::{ContextBreakdown, UsageSnapshot};

use super::binding::{self, Fingerprint};
use super::turn::Worker;
use super::{compact, prompt};

/// The size of each part of a request, in characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Weights {
    system_prompt: usize,
    instructions: usize,
    memory: usize,
    skills: usize,
    tools: usize,
    mcp_tools: usize,
    mcp_notes: usize,
    messages: usize,
}

pub(super) struct RequestHead {
    intent: Intent,
    profile: Profile,
    pub(super) system: String,
    pub(super) tools: Vec<ToolSchema>,
    pub(super) fingerprint: Fingerprint,
    weights: Weights,
}

impl Worker {
    /// The session context is fixed after startup. A refreshed registry and
    /// its MCP notes invalidate this slot together in `take_mcp`.
    pub(super) fn head(&mut self, intent: Intent, profile: Profile) -> &RequestHead {
        if self
            .request_head
            .as_ref()
            .is_none_or(|head| (head.intent, head.profile) != (intent, profile))
        {
            self.request_head = Some(self.build_head(intent, profile));
        }
        // The slot was either current or filled above.
        self.request_head.as_ref().unwrap()
    }

    fn build_head(&self, intent: Intent, profile: Profile) -> RequestHead {
        let system = prompt::build_with_memory(
            self.ws.root(),
            intent,
            profile,
            self.instructions.as_deref(),
            self.shared.shell,
            self.memory.as_deref(),
            self.skills.as_deref(),
        );
        let system = prompt::with_commit_attribution(system, self.shared.co_author_commits);
        let system = prompt::with_mcp(system, self.agents_section.as_deref());
        // After the project's own sections, ahead of the MCP servers': it is
        // the human's setup speaking, not an external program.
        let system = prompt::with_mcp(system, self.plugin_context.as_deref());
        let system = prompt::with_mcp(system, self.mcp_notes.as_deref());
        let tools = self.registry.schemas(intent, profile);
        let fingerprint = binding::fingerprint(&system, &tools);
        let weights = self.head_weights(&system, &tools);
        RequestHead {
            intent,
            profile,
            system,
            tools,
            fingerprint,
            weights,
        }
    }

    /// The meter's snapshot with the window taken apart.
    pub(super) fn usage_snapshot(&mut self) -> UsageSnapshot {
        let context_max = self.context_max();
        let mut usage = self.usage.snapshot(context_max);
        let (intent, profile) = {
            let state = self.shared.state.lock().unwrap();
            (state.mode.intent(), state.profile)
        };
        let mut weights = self.head(intent, profile).weights;
        weights.messages = compact::weight(&self.history);
        usage.breakdown = ContextBreakdown {
            reserved: self
                .compact_limit()
                .map_or(0, |limit| context_max.saturating_sub(limit)),
            ..split(usage.context_used, weights)
        };
        usage
    }

    fn head_weights(&self, system: &str, tools: &[ToolSchema]) -> Weights {
        // Each section goes into the prompt after a newline of its own
        // (`prompt::build_with_memory`), and that newline is the section's.
        let section = |text: Option<String>| text.map_or(0, |s| s.chars().count() + 1);
        // The plugins' start-up context is standing instructions too.
        let instructions = section(self.instructions.as_deref().map(prompt::project_section))
            + section(self.plugin_context.as_deref().map(str::to_string));
        let memory = section(self.memory.as_deref().map(prompt::memory_section));
        let skills = section(self.skills.as_deref().map(prompt::skills_section));
        let mcp_notes = section(self.mcp_notes.as_deref().map(str::to_string));

        let mut weights = Weights {
            system_prompt: system
                .chars()
                .count()
                .saturating_sub(instructions + memory + skills + mcp_notes),
            instructions,
            memory,
            skills,
            mcp_notes,
            ..Weights::default()
        };
        for tool in tools {
            let size = tool.name.chars().count()
                + tool.description.chars().count()
                + tool.input_schema.to_string().chars().count();
            if tool.name.starts_with(TOOL_PREFIX) {
                weights.mcp_tools += size;
            } else {
                weights.tools += size;
            }
        }
        weights
    }
}

/// Shares `used` tokens out by weight.
///
/// The head's parts are rounded down and the conversation takes the rest: it
/// is the part that grows, and a few tokens of rounding are lost in it.
fn split(used: u32, w: Weights) -> ContextBreakdown {
    let total = w.system_prompt
        + w.instructions
        + w.memory
        + w.skills
        + w.tools
        + w.mcp_tools
        + w.mcp_notes
        + w.messages;
    if used == 0 || total == 0 {
        return ContextBreakdown::default();
    }
    // `part <= total`, so the share is at most `used` and fits back in `u32`.
    let share = |part: usize| (used as u128 * part as u128 / total as u128) as u32;

    let mut out = ContextBreakdown {
        system_prompt: share(w.system_prompt),
        instructions: share(w.instructions),
        memory: share(w.memory),
        skills: share(w.skills),
        tools: share(w.tools),
        mcp_tools: share(w.mcp_tools),
        mcp_notes: share(w.mcp_notes),
        ..Default::default()
    };
    let head = out.system_prompt
        + out.instructions
        + out.memory
        + out.skills
        + out.tools
        + out.mcp_tools
        + out.mcp_notes;
    out.messages = used - head;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occupied(b: &ContextBreakdown) -> u32 {
        b.system_prompt
            + b.instructions
            + b.memory
            + b.skills
            + b.tools
            + b.mcp_tools
            + b.mcp_notes
            + b.messages
    }

    #[test]
    fn the_parts_add_up_to_the_measured_total() {
        let weights = Weights {
            system_prompt: 7_001,
            instructions: 2_333,
            memory: 97,
            skills: 311,
            tools: 19_013,
            mcp_tools: 12_007,
            mcp_notes: 3_301,
            messages: 335_917,
        };
        for used in [1, 999, 24_800, 388_901] {
            assert_eq!(occupied(&split(used, weights)), used);
        }
    }

    #[test]
    fn a_part_is_as_large_as_its_share_of_the_text() {
        let b = split(
            10_000,
            Weights {
                system_prompt: 100,
                tools: 300,
                messages: 600,
                ..Default::default()
            },
        );
        assert_eq!(b.system_prompt, 1_000);
        assert_eq!(b.tools, 3_000);
        assert_eq!(b.messages, 6_000);
        assert_eq!(b.mcp_tools, 0);
    }

    #[test]
    fn nothing_measured_is_nothing_split() {
        let weights = Weights {
            system_prompt: 100,
            ..Default::default()
        };
        assert_eq!(split(0, weights), ContextBreakdown::default());
        assert_eq!(split(500, Weights::default()), ContextBreakdown::default());
    }
}

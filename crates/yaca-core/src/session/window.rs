//! What the context window is filled with (§7.1).
//!
//! The provider answers with one number for the whole request. The interface
//! shows what that number is made of, and the only thing known about the parts
//! is their text — so the measured total is shared out by the size of each
//! part's text. The total stays a fact and the split is an estimate; the parts
//! always add up to the total, so the two cannot disagree on screen.
//!
//! Nothing here is cached: the prompt and the schemas are put together again
//! for every snapshot, from the same pieces the request is built from. A copy
//! kept on the worker would be a second version of what the request holds.

use crate::intent::Intent;
use crate::mcp::TOOL_PREFIX;
use crate::provider::ToolSchema;
use crate::usage::{ContextBreakdown, UsageSnapshot};

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

impl Worker {
    /// The system prompt and the tools: the cached head of every request.
    pub(super) fn head(&self, intent: Intent) -> (String, Vec<ToolSchema>) {
        let system = prompt::build_with_memory(
            self.ws.root(),
            intent,
            self.instructions.as_deref(),
            self.shared.shell,
            self.memory.as_deref(),
            self.skills.as_deref(),
        );
        let system = prompt::with_mcp(system, self.mcp_notes.as_deref());
        (system, self.registry.schemas(intent))
    }

    /// The meter's snapshot with the window taken apart.
    pub(super) fn usage_snapshot(&self) -> UsageSnapshot {
        let context_max = self.context_max();
        let mut usage = self.usage.snapshot(context_max);
        usage.breakdown = ContextBreakdown {
            reserved: self
                .compact_limit()
                .map_or(0, |limit| context_max.saturating_sub(limit)),
            ..split(usage.context_used, self.weights())
        };
        usage
    }

    fn weights(&self) -> Weights {
        let intent = self.shared.state.lock().unwrap().mode.intent();
        let (system, tools) = self.head(intent);

        // Each section goes into the prompt after a newline of its own
        // (`prompt::build_with_memory`), and that newline is the section's.
        let section = |text: Option<String>| text.map_or(0, |s| s.chars().count() + 1);
        let instructions = section(self.instructions.as_deref().map(prompt::project_section));
        let memory = section(self.memory.as_deref().map(prompt::memory_section));
        let skills = section(self.skills.as_deref().map(prompt::skills_section));
        let mcp_notes = section(self.mcp_notes.as_deref().map(str::to_string));

        let (mcp, own): (Vec<_>, Vec<_>) =
            tools.iter().partition(|t| t.name.starts_with(TOOL_PREFIX));
        let schemas = |tools: &[&ToolSchema]| -> usize {
            tools
                .iter()
                .map(|t| {
                    t.name.chars().count()
                        + t.description.chars().count()
                        + t.input_schema.to_string().chars().count()
                })
                .sum()
        };

        Weights {
            system_prompt: system
                .chars()
                .count()
                .saturating_sub(instructions + memory + skills + mcp_notes),
            instructions,
            memory,
            skills,
            tools: schemas(&own),
            mcp_tools: schemas(&mcp),
            mcp_notes,
            messages: compact::weight(&self.history),
        }
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

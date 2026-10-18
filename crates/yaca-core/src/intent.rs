//! The session mode and what follows from it (§7.1, §8).
//!
//! The human switches one axis — [`Mode`], four mutually exclusive items in the
//! permissions menu. [`Intent`] and [`TrustMode`] are derived from it and are not
//! stored state: a core that knew combinations the window cannot select would be
//! a second version of the truth, and it would drift apart on the first switch.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Session mode. The only thing the human switches (§8).
///
/// Declared from cautious to trusting: plan has no writing tools at all,
/// manual asks about them. The interface lists them in its own order, with the
/// default first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// Read and plan. The model gets no writing tools at all.
    Plan,
    /// Nothing without asking.
    Manual,
    /// Everything except running commands. The default: edits are checkpointed
    /// and can be rolled back, and what cannot be — a command — still asks.
    #[default]
    Auto,
    /// Apply edits, ask about the rest.
    Edits,
}

impl Mode {
    /// Which tools make it into the request schema at all.
    pub fn intent(self) -> Intent {
        match self {
            Mode::Plan => Intent::Plan,
            Mode::Manual | Mode::Auto | Mode::Edits => Intent::Agent,
        }
    }

    /// Which row of the §8 matrix decides the permission question.
    ///
    /// Plan has no row of its own, yet an answer is needed: no writing tools
    /// occur there, but `web_fetch` occurs in every mode and asks for permission.
    /// `Manual` is taken — plan is reconnaissance, and a question about the
    /// network fits in it. Remembering the trust chosen before plan would mean
    /// keeping under the chip a state that the chip does not show.
    pub fn trust(self) -> TrustMode {
        match self {
            Mode::Plan | Mode::Manual => TrustMode::Manual,
            Mode::Auto => TrustMode::Auto,
            Mode::Edits => TrustMode::Edits,
        }
    }
}

/// The intent axis: which tools make it into the request schema. Derived from [`Mode`].
///
/// §7.1: "Writing tools are not handed to the model at all: a weak model ignores
/// a ban in the prompt and spends a turn on the error." Hence this is a filter on
/// the registry, not a line in the system prompt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Reading tools plus `todo_write` and `exit_plan_mode`.
    Plan,
    /// The full set.
    #[default]
    Agent,
}

impl Intent {
    pub fn allows(self, allowed: &[Intent]) -> bool {
        allowed.contains(&self)
    }
}

/// A row of the §8 trust matrix. Derived from [`Mode`] and never sent outside:
/// the interface switches the mode, not this axis on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TrustMode {
    /// Nothing without asking.
    #[default]
    Manual,
    /// Everything except command execution.
    Auto,
    /// Apply edits, ask for the rest.
    Edits,
}

/// Reasoning effort level (§7.1, the bottom-right corner of the composer).
///
/// Maps one to one onto the Messages API's `output_config.effort`. A fixed
/// token budget (`thinking.budget_tokens`) has nothing to do with it: on Opus 5,
/// Sonnet 5 and Opus 4.7+ it has been removed and returns 400.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    /// «Extra» from §7.1. The best level for coding and agentic tasks.
    #[default]
    Extra,
    Max,
}

impl Effort {
    /// The value for `output_config.effort`.
    pub fn as_api(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Extra => "xhigh",
            Effort::Max => "max",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::policy::{Policy, Verdict};
    use crate::permission::PermissionKind;

    fn policy(mode: Mode) -> Policy {
        Policy {
            trust: mode.trust(),
            ..Policy::default()
        }
    }

    #[test]
    fn only_plan_removes_writing_tools_from_the_model() {
        assert_eq!(Mode::Plan.intent(), Intent::Plan);
        for mode in [Mode::Manual, Mode::Auto, Mode::Edits] {
            assert_eq!(mode.intent(), Intent::Agent, "{mode:?}");
        }
    }

    #[test]
    fn plan_asks_about_the_network() {
        // There are no writing tools in plan, but `web_fetch` is, and the §8 row
        // is needed for it. "Manual" is taken: plan is reconnaissance, and a
        // question about the network fits in it.
        let reach = PermissionKind::FetchUrl;
        assert_eq!(policy(Mode::Plan).verdict(reach), Verdict::Ask);
        assert_eq!(policy(Mode::Manual).verdict(reach), Verdict::Ask);
        // "Auto" asks too, and its reviewer model answers instead of the human.
        assert_eq!(policy(Mode::Auto).verdict(reach), Verdict::Ask);
    }

    #[test]
    fn plan_behaves_like_manual_also_for_edits() {
        // There are no writing tools in plan, but if it came to a question, the
        // answer is the same as for "Manual": ask.
        assert_eq!(
            policy(Mode::Plan).verdict(PermissionKind::EditFile),
            Verdict::Ask
        );
        assert_eq!(
            policy(Mode::Manual).verdict(PermissionKind::EditFile),
            Verdict::Ask
        );
        assert_eq!(
            policy(Mode::Auto).verdict(PermissionKind::EditFile),
            Verdict::Allow
        );
    }
}

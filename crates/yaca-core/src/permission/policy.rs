//! The trust matrix of §8, as a pure function.
//!
//! Kept separate from the broker on purpose: deciding *whether* to ask has no
//! channels, no waiting and no clock, so it can be read as a table and tested
//! as one. The broker deals with the asking.

use super::broker::Standing;
use super::{ActionClass, PermissionKind, PermissionNeed};
use crate::intent::TrustMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Go ahead without asking.
    Allow,
    /// Put the question to the human and wait. There is no timeout (§5.7).
    Ask,
}

/// What happens to one call before it runs (§8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Pass,
    Ask,
    /// A standing "no" covers the call. The note goes to the agent.
    Refuse(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub trust: TrustMode,
    /// Not a fifth mode but a flag over the current one (§8). Does not
    /// survive a restart and is cleared when the project changes.
    pub bypass: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            // Manual by default. Yaca runs with any model, weak local ones
            // included, so safe behaviour is the default and trust is
            // something the human hands out.
            trust: TrustMode::Manual,
            bypass: false,
        }
    }
}

impl Policy {
    /// The whole of §8 for one call, in the order it applies.
    ///
    /// 1. A standing refusal stops the call in every mode, bypass included:
    ///    bypass means "stop asking me", not "do what I said no to".
    /// 2. An invariant is asked about in every mode, bypass included. A grant
    ///    does not reach it: "always allow git" must not let a push through.
    /// 3. A standing grant lets the call through.
    /// 4. Bypass lets everything else through.
    /// 5. A deletion that loses what no snapshot keeps is asked about in every
    ///    mode but bypass, Auto included (§8).
    /// 6. The mode's row of the matrix decides the rest.
    pub fn decide(&self, need: &PermissionNeed, standing: Option<&Standing>) -> Decision {
        if let Some(Standing::Refused(refusal)) = standing {
            return Decision::Refuse(refusal.reason.clone());
        }
        if need.invariant.is_some() {
            return Decision::Ask;
        }
        if matches!(standing, Some(Standing::Granted)) || self.bypass {
            return Decision::Pass;
        }
        if need.unsaved.is_some() {
            return Decision::Ask;
        }
        match self.verdict(need.kind) {
            Verdict::Allow => Decision::Pass,
            Verdict::Ask => Decision::Ask,
        }
    }

    pub fn verdict(&self, kind: PermissionKind) -> Verdict {
        if self.bypass {
            return Verdict::Allow;
        }

        match (kind.class(), self.trust) {
            // An edit reaches the working tree at once, so Manual asks about
            // it — that question is the one confirmation there is, and there is
            // no second one further down (§16.37).
            (ActionClass::Edit, TrustMode::Manual) => Verdict::Ask,
            (ActionClass::Edit, _) => Verdict::Allow,

            // Commands are asked about in every mode, Auto included: a command
            // can do anything, and the matrix says so explicitly. In Auto a
            // reviewer model reads the call first (`permission::review`) and may
            // answer for the human; this table stays the question's source.
            (ActionClass::Command, _) => Verdict::Ask,

            (ActionClass::Destructive, TrustMode::Auto) => Verdict::Allow,
            (ActionClass::Destructive, _) => Verdict::Ask,

            // The network and writes through MCP are asked about in every mode
            // too, and in Auto the reviewer answers for the human here as well:
            // unlike a deletion, nothing in the core can undo what left the machine
            // or changed in someone else's service, so a model reads it first.
            (ActionClass::Reach, _) => Verdict::Ask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::{Invariant, Unsaved};

    fn policy(trust: TrustMode) -> Policy {
        Policy {
            trust,
            ..Default::default()
        }
    }

    /// The whole of §8 in one table. If a row here changes, the spec changed.
    #[test]
    fn trust_matrix_matches_the_spec() {
        use PermissionKind::*;
        use TrustMode::*;
        use Verdict::*;

        let cases = [
            //                       Manual  Auto   Edits
            (DeleteFile, [Ask, Allow, Ask]),
            (MoveFile, [Ask, Allow, Ask]),
            (RunCommand, [Ask, Ask, Ask]),
            // Asked in Auto as well: there the reviewer model answers (§8).
            (FetchUrl, [Ask, Ask, Ask]),
            (McpWrite, [Ask, Ask, Ask]),
            // An edit is written at once, and "Manual" is the only mode that asks
            // about it. This is the one confirmation itself (§16.37).
            (EditFile, [Ask, Allow, Allow]),
        ];

        for (kind, expected) in cases {
            for (mode, want) in [Manual, Auto, Edits].into_iter().zip(expected) {
                assert_eq!(
                    policy(mode).verdict(kind),
                    want,
                    "{kind:?} in {mode:?} should be {want:?}"
                );
            }
        }
    }

    #[test]
    fn bypass_allows_everything_including_commands() {
        let p = Policy {
            bypass: true,
            ..policy(TrustMode::Manual)
        };
        for kind in [
            PermissionKind::RunCommand,
            PermissionKind::DeleteFile,
            PermissionKind::DeleteDirectory,
            PermissionKind::MoveFile,
            PermissionKind::FetchUrl,
            PermissionKind::McpWrite,
            PermissionKind::EditFile,
        ] {
            assert_eq!(p.verdict(kind), Verdict::Allow, "{kind:?}");
        }
    }

    fn command(line: &str) -> PermissionNeed {
        PermissionNeed::new(PermissionKind::RunCommand, "Bash", line).command(line)
    }

    fn git_push() -> PermissionNeed {
        command("git push").invariant(Invariant::GitPush)
    }

    fn git_grant() -> Standing {
        Standing::Granted
    }

    #[test]
    fn an_invariant_is_asked_in_every_mode_and_under_bypass() {
        for trust in [TrustMode::Manual, TrustMode::Auto, TrustMode::Edits] {
            for bypass in [false, true] {
                let p = Policy { trust, bypass };
                assert_eq!(
                    p.decide(&git_push(), None),
                    Decision::Ask,
                    "{trust:?} bypass={bypass}"
                );
            }
        }
    }

    #[test]
    fn a_grant_for_the_command_does_not_reach_an_invariant() {
        // "Always allow git" covers `git push` as a command. It must not pass
        // the push, in Auto or under bypass.
        let grant = git_grant();
        for bypass in [false, true] {
            let p = Policy {
                trust: TrustMode::Auto,
                bypass,
            };
            assert_eq!(p.decide(&git_push(), Some(&grant)), Decision::Ask);
        }
    }

    #[test]
    fn a_standing_refusal_still_stops_an_invariant_without_a_question() {
        let refusal = Standing::Refused(crate::permission::Refusal::new(
            crate::permission::ScopeKey::CommandPrefix {
                prefix: "git".into(),
            },
            Some("no pushes today".into()),
        ));
        let p = Policy {
            trust: TrustMode::Auto,
            bypass: true,
        };
        assert_eq!(
            p.decide(&git_push(), Some(&refusal)),
            Decision::Refuse(Some("no pushes today".into()))
        );
    }

    #[test]
    fn an_unsaved_deletion_is_asked_in_auto_but_not_under_bypass() {
        let loses = PermissionNeed::new(PermissionKind::DeleteDirectory, "DeleteDir", "dist")
            .unsaved(Unsaved {
                roots: Vec::new(),
                dirs: vec!["dist".into()],
                dir_count: 1,
                files: 0,
            });
        let auto = Policy {
            trust: TrustMode::Auto,
            bypass: false,
        };
        assert_eq!(auto.decide(&loses, None), Decision::Ask);
        // Bypass turns off permissions (§8), and this question is one.
        let bypass = Policy {
            trust: TrustMode::Auto,
            bypass: true,
        };
        assert_eq!(bypass.decide(&loses, None), Decision::Pass);
        // A grant the human gave earlier is theirs to keep.
        assert_eq!(auto.decide(&loses, Some(&git_grant())), Decision::Pass);
    }

    #[test]
    fn a_git_configuration_write_is_asked_in_every_mode_and_under_bypass() {
        // The write is the step a later `git status` would run, so no mode and
        // no bypass lets it through unasked, and a grant does not either.
        let edit = PermissionNeed::new(PermissionKind::EditFile, "Edit", "config")
            .invariant(Invariant::GitConfig);
        let grant = git_grant();
        for trust in [TrustMode::Manual, TrustMode::Auto, TrustMode::Edits] {
            for bypass in [false, true] {
                let p = Policy { trust, bypass };
                assert_eq!(
                    p.decide(&edit, None),
                    Decision::Ask,
                    "{trust:?} bypass={bypass}"
                );
                assert_eq!(
                    p.decide(&edit, Some(&grant)),
                    Decision::Ask,
                    "{trust:?} bypass={bypass}"
                );
            }
        }
    }

    #[test]
    fn a_saved_deletion_keeps_the_auto_row_of_the_matrix() {
        let plain = PermissionNeed::new(PermissionKind::DeleteDirectory, "DeleteDir", "src");
        let auto = Policy {
            trust: TrustMode::Auto,
            bypass: false,
        };
        assert_eq!(auto.decide(&plain, None), Decision::Pass);
    }
}

//! The trust matrix of §8, as a pure function.
//!
//! Kept separate from the broker on purpose: deciding *whether* to ask has no
//! channels, no waiting and no clock, so it can be read as a table and tested
//! as one. The broker deals with the asking.

use super::{ActionClass, PermissionKind};
use crate::intent::TrustMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Go ahead without asking.
    Allow,
    /// Put the question to the human and wait. There is no timeout (§5.7).
    Ask,
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
            // can do anything, and the matrix says so explicitly.
            (ActionClass::Command, _) => Verdict::Ask,

            (ActionClass::Destructive, TrustMode::Auto) => Verdict::Allow,
            (ActionClass::Destructive, _) => Verdict::Ask,

            (ActionClass::Reach, TrustMode::Auto) => Verdict::Allow,
            (ActionClass::Reach, _) => Verdict::Ask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (FetchUrl, [Ask, Allow, Ask]),
            (McpWrite, [Ask, Allow, Ask]),
            // Правка пишется сразу, и «Ручной» — единственный, кто про неё
            // спрашивает. Это и есть то самое одно подтверждение (§16.37).
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
}

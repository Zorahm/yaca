//! Asking, waiting, and remembering the answer.
//!
//! Two things live here that the policy table cannot express. First, the wait:
//! §5.7 says there is no timeout, so a pending question blocks its own tool
//! call and nothing else — the composer keeps working, and `Esc` still stops
//! the turn. Second, the standing rules.
//!
//! The broker itself holds those rules in memory and nothing else: it neither
//! reads nor writes a file. Two roads lead into it. "Always allow" and
//! "Always deny" in the feed reach only this table and die with the session —
//! a rule granted in a hurry does not outlive the hurry (§5.7). What was put
//! in `.yaca/settings.json` through the settings window (§10) is poured in at
//! spawn, before the first question. Which road a rule came by does not change
//! how it is applied.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

use super::{PermissionAnswer, PermissionNeed, Refusal, ScopeKey};
use crate::id::RequestId;
use crate::pending::Pending;

#[derive(Debug, Default)]
struct Inner {
    /// Rules granted through "Always allow". Session-lived (§5.7).
    rules: Vec<ScopeKey>,
    /// Rules set through "Always deny". Session-lived, like the grants.
    refusals: Vec<Refusal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    Granted,
    Refused(Refusal),
}

#[derive(Debug, Clone)]
pub struct Broker {
    inner: Arc<Mutex<Inner>>,
    /// Questions currently on screen. A permission needs nothing beyond its
    /// id to judge an answer, hence the unit metadata.
    waiting: Pending<RequestId, (), PermissionAnswer>,
    /// The **primary** root. A relative directory rule is resolved against it
    /// and never against an additional root; an absolute rule (an additional
    /// root's directory) does not need it. See `PermissionNeed::covered_by`.
    root: PathBuf,
}

impl Broker {
    /// `root` is the primary root (`Workspace::root`), on purpose.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Broker {
            inner: Arc::new(Mutex::new(Inner::default())),
            waiting: Pending::default(),
            root: root.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What the human has already said about calls like this one, if anything.
    ///
    /// A refusal is looked for first: it outranks a grant that also covers
    /// the call. The caller asks this before the mode as well, so bypass
    /// means "stop asking me", not "do what I said no to".
    pub fn standing(&self, need: &PermissionNeed) -> Option<Standing> {
        let inner = self.inner.lock().unwrap();
        if let Some(refusal) = inner
            .refusals
            .iter()
            .find(|r| need.covered_by(&r.scope, &self.root))
        {
            return Some(Standing::Refused(refusal.clone()));
        }
        inner
            .rules
            .iter()
            .any(|rule| need.covered_by(rule, &self.root))
            .then_some(Standing::Granted)
    }

    pub fn grant(&self, scope: ScopeKey) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.rules.contains(&scope) {
            inner.rules.push(scope);
        }
    }

    pub fn rules(&self) -> Vec<ScopeKey> {
        self.inner.lock().unwrap().rules.clone()
    }

    /// A second refusal of the same scope replaces the note: the latest
    /// word on it is the one the agent should read.
    pub fn refuse(&self, scope: ScopeKey, reason: Option<String>) {
        let mut inner = self.inner.lock().unwrap();
        inner.refusals.retain(|r| r.scope != scope);
        inner.refusals.push(Refusal { scope, reason });
    }

    /// Registers a question and hands back the channel its answer arrives on.
    pub fn open(&self, id: RequestId) -> oneshot::Receiver<PermissionAnswer> {
        self.waiting.open(id, ())
    }

    /// Delivers an answer. `false` means nobody was waiting on that id —
    /// a stale click after the turn was cancelled, which is not an error.
    pub fn answer(&self, id: &RequestId, answer: PermissionAnswer) -> bool {
        self.waiting.answer(id, answer)
    }

    /// Drops every pending question. The waiters see a closed channel and
    /// treat it as cancellation.
    pub fn abandon_all(&self) {
        self.waiting.abandon_all();
    }

    /// How many questions are on screen right now. Feeds the `2 of 3` counter.
    pub fn pending(&self) -> u32 {
        self.waiting.len() as u32
    }

    pub fn forget(&self, id: &RequestId) {
        self.waiting.forget(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionKind;

    fn broker() -> Broker {
        Broker::new("/p")
    }

    fn need(path: &str) -> PermissionNeed {
        PermissionNeed::new(PermissionKind::DeleteFile, "Delete", path)
            .path(PathBuf::from("/p").join(path))
    }

    #[tokio::test]
    async fn answer_reaches_the_waiter() {
        let b = broker();
        let id = RequestId::new();
        let rx = b.open(id.clone());

        assert_eq!(b.pending(), 1);
        assert!(b.answer(&id, PermissionAnswer::Allow));
        assert_eq!(rx.await.unwrap(), PermissionAnswer::Allow);
        assert_eq!(b.pending(), 0);
    }

    #[tokio::test]
    async fn a_stale_answer_is_not_an_error() {
        let b = broker();
        // Nobody is waiting: the turn was cancelled before the click landed.
        assert!(!b.answer(&RequestId::new(), PermissionAnswer::Allow));
    }

    #[tokio::test]
    async fn abandoning_looks_like_cancellation_to_the_waiter() {
        let b = broker();
        let id = RequestId::new();
        let rx = b.open(id);

        b.abandon_all();
        assert!(rx.await.is_err(), "closed channel means cancelled");
    }

    #[test]
    fn a_granted_rule_covers_later_calls() {
        let b = broker();
        assert_eq!(b.standing(&need("src/a.rs")), None);

        b.grant(ScopeKey::Directory { path: "src".into() });
        assert_eq!(b.standing(&need("src/a.rs")), Some(Standing::Granted));
        assert_eq!(b.standing(&need("src/deep/b.rs")), Some(Standing::Granted));
        assert_eq!(b.standing(&need("tests/c.rs")), None);
    }

    #[test]
    fn a_refusal_outranks_a_grant_that_also_covers_the_call() {
        let b = broker();
        b.grant(ScopeKey::Tool {
            name: "Delete".into(),
        });
        b.refuse(ScopeKey::Directory { path: "src".into() }, None);

        assert!(matches!(
            b.standing(&need("src/a.rs")),
            Some(Standing::Refused(_))
        ));
        // Outside the refusal the grant still stands.
        assert_eq!(b.standing(&need("tests/a.rs")), Some(Standing::Granted));
    }

    #[test]
    fn refusing_the_same_scope_again_keeps_the_latest_note() {
        let b = broker();
        let scope = ScopeKey::Directory { path: "src".into() };
        b.refuse(scope.clone(), Some("generated".into()));
        b.refuse(scope, Some("vendored".into()));

        let Some(Standing::Refused(refusal)) = b.standing(&need("src/a.rs")) else {
            panic!("the refusal was lost");
        };
        assert_eq!(refusal.reason.as_deref(), Some("vendored"));
    }

    #[test]
    fn granting_the_same_rule_twice_keeps_one() {
        let b = broker();
        let rule = ScopeKey::Tool {
            name: "Delete".into(),
        };
        b.grant(rule.clone());
        b.grant(rule);
        assert_eq!(b.rules().len(), 1);
    }
}

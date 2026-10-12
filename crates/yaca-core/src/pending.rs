//! Waiting for an answer a human gives later (§5.7).
//!
//! A permission request and a question have the same shape: something is put
//! on screen under an id, the tool that asked waits on a channel, and an answer
//! arriving from the interface wakes it. Dropping every sender is how a turn's
//! cancellation reaches the waiters: they see a closed channel and read it as
//! "the human will not answer". That map lives here once, so both brokers keep
//! the same rules; what each one checks before it delivers stays with it.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;

/// The map behind [`Pending`]: each id with its metadata and its channel.
type Waiting<K, M, A> = HashMap<K, (M, oneshot::Sender<A>)>;

/// Questions on screen, by id. `M` is whatever the broker needs to judge an
/// answer before delivering it, kept beside the channel it belongs to.
pub struct Pending<K, M, A> {
    waiting: Arc<Mutex<Waiting<K, M, A>>>,
}

impl<K, M, A> Pending<K, M, A>
where
    K: Eq + Hash,
    M: Clone,
{
    pub fn open(&self, key: K, meta: M) -> oneshot::Receiver<A> {
        let (tx, rx) = oneshot::channel();
        self.waiting.lock().unwrap().insert(key, (meta, tx));
        rx
    }

    /// What was put on screen under this id, if it is still waiting.
    pub fn meta(&self, key: &K) -> Option<M> {
        self.waiting
            .lock()
            .unwrap()
            .get(key)
            .map(|(meta, _)| meta.clone())
    }

    /// Delivers an answer. `false` means nobody was waiting on that id: a
    /// stale click after the turn was cancelled, which is not an error.
    pub fn answer(&self, key: &K, answer: A) -> bool {
        let entry = self.waiting.lock().unwrap().remove(key);
        match entry {
            Some((_, tx)) => tx.send(answer).is_ok(),
            None => false,
        }
    }

    /// Drops every pending question. The waiters see a closed channel.
    pub fn abandon_all(&self) {
        self.waiting.lock().unwrap().clear();
    }

    pub fn forget(&self, key: &K) {
        self.waiting.lock().unwrap().remove(key);
    }

    pub fn len(&self) -> usize {
        self.waiting.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K, M, A> Default for Pending<K, M, A> {
    fn default() -> Self {
        Pending {
            waiting: Arc::default(),
        }
    }
}

impl<K, M, A> std::fmt::Debug for Pending<K, M, A> {
    /// Only the count: the senders and what is waiting on them mean nothing
    /// in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending")
            .field("waiting", &self.waiting.lock().unwrap().len())
            .finish()
    }
}

impl<K, M, A> Clone for Pending<K, M, A> {
    /// Clones share the map: every holder sees the same questions.
    fn clone(&self) -> Self {
        Pending {
            waiting: self.waiting.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_answer_reaches_the_waiter_and_leaves_the_map() {
        let p: Pending<u32, &str, u32> = Pending::default();
        let rx = p.open(1, "meta");

        assert_eq!(p.meta(&1), Some("meta"));
        assert!(p.answer(&1, 42));
        assert_eq!(rx.await.unwrap(), 42);
        assert!(p.is_empty());
    }

    #[tokio::test]
    async fn abandoning_closes_every_channel() {
        let p: Pending<u32, (), u32> = Pending::default();
        let rx = p.open(1, ());
        p.abandon_all();
        assert!(rx.await.is_err());
    }

    #[test]
    fn a_stale_answer_is_reported_not_delivered() {
        let p: Pending<u32, (), u32> = Pending::default();
        assert!(!p.answer(&7, 1));
    }

    #[test]
    fn clones_share_the_map() {
        let p: Pending<u32, (), u32> = Pending::default();
        let other = p.clone();
        let _rx = other.open(1, ());
        assert_eq!(p.len(), 1);
    }
}

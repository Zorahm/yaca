//! What the worker has been asked to do, and what it can take in mid-turn.
//!
//! A message sent while the agent works joins the conversation after the
//! current group of actions, not after the whole turn (§7.4). So the worker
//! looks into its queue between one batch of calls and the model's next step —
//! and has to leave everything else there in the order it came: a compaction
//! asked for before the message still runs before it.

use std::collections::VecDeque;

use tokio::sync::mpsc;

use super::queue::{Queue, Queued};
use super::Job;

pub(super) struct Inbox {
    rx: mpsc::Receiver<Job>,
    /// Taken off the channel while looking for messages, and not one.
    held: VecDeque<Job>,
}

impl Inbox {
    pub(super) fn new(rx: mpsc::Receiver<Job>) -> Self {
        Inbox {
            rx,
            held: VecDeque::new(),
        }
    }

    /// The next job, in the order jobs were sent.
    pub(super) async fn next(&mut self) -> Option<Job> {
        match self.held.pop_front() {
            Some(job) => Some(job),
            None => self.rx.recv().await,
        }
    }

    /// Messages waiting at the head of the queue, taken out of it.
    ///
    /// Only the head: a message sent after a compaction was asked for waits
    /// for the compaction, as it would have without a turn in the way. A
    /// message cancelled in the meantime is gone from `queue` and is skipped
    /// here, without holding up the ones behind it.
    pub(super) fn messages(&mut self, queue: &Queue) -> Vec<Queued> {
        while let Ok(job) = self.rx.try_recv() {
            self.held.push_back(job);
        }

        let mut out = Vec::new();
        while let Some(job) = self.held.pop_front() {
            match job {
                Job::Message { id } => out.extend(queue.take(&id)),
                other => {
                    self.held.push_front(other);
                    break;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventSink;
    use crate::id::QueuedId;

    fn queue() -> Queue {
        Queue::new(EventSink::new(tokio::sync::broadcast::channel(64).0))
    }

    /// What the dispatcher does with a message: queue it, then send its job.
    async fn send(tx: &mpsc::Sender<Job>, queue: &Queue, text: &str) -> QueuedId {
        let id = queue.push(text.into(), Vec::new(), true);
        tx.send(Job::Message { id: id.clone() }).await.unwrap();
        id
    }

    fn texts(queued: Vec<Queued>) -> Vec<String> {
        queued.into_iter().map(|q| q.text).collect()
    }

    #[tokio::test]
    async fn messages_at_the_head_are_taken_and_the_rest_keeps_its_order() {
        let (tx, rx) = mpsc::channel(8);
        let queue = queue();
        let mut inbox = Inbox::new(rx);
        send(&tx, &queue, "one").await;
        send(&tx, &queue, "two").await;
        tx.send(Job::Compact).await.unwrap();
        send(&tx, &queue, "three").await;

        assert_eq!(texts(inbox.messages(&queue)), vec!["one", "two"]);
        // The compaction was asked for first, so it still comes first.
        assert!(matches!(inbox.next().await, Some(Job::Compact)));
        assert!(matches!(inbox.next().await, Some(Job::Message { .. })));
        assert_eq!(texts(inbox.messages(&queue)), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_cancelled_message_is_skipped_and_holds_up_nothing() {
        let (tx, rx) = mpsc::channel(8);
        let queue = queue();
        let mut inbox = Inbox::new(rx);
        send(&tx, &queue, "one").await;
        let two = send(&tx, &queue, "two").await;
        send(&tx, &queue, "three").await;
        queue.cancel(&two);

        assert_eq!(texts(inbox.messages(&queue)), vec!["one", "three"]);
    }

    #[tokio::test]
    async fn nothing_queued_is_nothing_taken() {
        let (_tx, rx) = mpsc::channel(8);
        let queue = queue();
        let mut inbox = Inbox::new(rx);
        assert!(inbox.messages(&queue).is_empty());
    }
}

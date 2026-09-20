//! What the worker has been asked to do, and what it can take in mid-turn.
//!
//! A message sent while the agent works joins the conversation after the
//! current group of actions, not after the whole turn (§7.4). So the worker
//! looks into its queue between one batch of calls and the model's next step —
//! and has to leave everything else there in the order it came: a compaction
//! asked for before the message still runs before it.

use std::collections::VecDeque;

use tokio::sync::mpsc;

use super::Job;

/// A message sent while a turn was running.
pub(super) struct Queued {
    pub(super) text: String,
    pub(super) attachments: Vec<String>,
}

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
    /// for the compaction, as it would have without a turn in the way.
    pub(super) fn messages(&mut self) -> Vec<Queued> {
        while let Ok(job) = self.rx.try_recv() {
            self.held.push_back(job);
        }

        let mut out = Vec::new();
        while let Some(job) = self.held.pop_front() {
            match job {
                Job::Message { text, attachments } => out.push(Queued { text, attachments }),
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

    fn message(text: &str) -> Job {
        Job::Message {
            text: text.into(),
            attachments: Vec::new(),
        }
    }

    fn texts(queued: Vec<Queued>) -> Vec<String> {
        queued.into_iter().map(|q| q.text).collect()
    }

    #[tokio::test]
    async fn messages_at_the_head_are_taken_and_the_rest_keeps_its_order() {
        let (tx, rx) = mpsc::channel(8);
        let mut inbox = Inbox::new(rx);
        for job in [message("one"), message("two"), Job::Compact, message("three")] {
            tx.send(job).await.unwrap();
        }

        assert_eq!(texts(inbox.messages()), vec!["one", "two"]);
        // The compaction was asked for first, so it still comes first.
        assert!(matches!(inbox.next().await, Some(Job::Compact)));
        assert!(matches!(inbox.next().await, Some(Job::Message { text, .. }) if text == "three"));
    }

    #[tokio::test]
    async fn nothing_queued_is_nothing_taken() {
        let (_tx, rx) = mpsc::channel(8);
        let mut inbox = Inbox::new(rx);
        assert!(inbox.messages().is_empty());
    }
}

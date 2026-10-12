//! Messages the human sent while a turn was running, until the turn takes them
//! in (§7.4).
//!
//! The queue is the one place a waiting message lives. The channel to the worker
//! carries only `Job::Message { id }`, which says "look in the queue for this".
//! So a cancel is the removal of an entry, and the worker's take is the removal
//! of the same entry: both happen under one lock, whichever comes first wins,
//! and the other finds nothing. A cancelled message therefore never reaches the
//! model, and one already taken in cannot be cancelled.
//!
//! Every change is announced as the whole list, under the same lock. The order
//! the interface sees is the order the changes were made in, and a list kept in
//! the interface would be a second copy of this one.

use std::sync::Mutex;

use crate::event::{AgentEvent, EventSink, QueuedMessage};
use crate::id::QueuedId;

/// A message taken off the queue, to be said to the model.
pub(super) struct Queued {
    pub(super) text: String,
    pub(super) attachments: Vec<String>,
}

struct Entry {
    id: QueuedId,
    text: String,
    attachments: Vec<String>,
    /// The interface has been sent a list that holds this entry. Taking an
    /// entry the interface never saw is silent: there is nothing to take back.
    shown: bool,
}

pub(super) struct Queue {
    /// Oldest first: the order the messages are taken in.
    entries: Mutex<Vec<Entry>>,
    events: EventSink,
}

impl Queue {
    pub(super) fn new(events: EventSink) -> Self {
        Queue {
            entries: Mutex::new(Vec::new()),
            events,
        }
    }

    /// Puts a message in the queue and returns its id.
    ///
    /// `announce` is false when the worker is idle: it takes the message at once,
    /// and a chip shown for the instant before that would tell the human that
    /// their message waits when it does not. Whatever the worker does next is
    /// announced by the take, so the interface still ends up with the truth.
    pub(super) fn push(&self, text: String, attachments: Vec<String>, announce: bool) -> QueuedId {
        let id = QueuedId::new();
        let mut entries = self.entries.lock().unwrap();
        entries.push(Entry {
            id: id.clone(),
            text,
            attachments,
            shown: false,
        });
        if announce {
            self.announce(&mut entries);
        }
        id
    }

    /// Removes a message that is still waiting. A message that is not there —
    /// already taken in, or never queued — is not an error: the list is sent
    /// regardless, so an interface that asked gets the core's answer back.
    pub(super) fn cancel(&self, id: &QueuedId) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|entry| &entry.id != id);
        self.announce(&mut entries);
    }

    /// Takes a message out for the turn. `None` once it has been cancelled.
    pub(super) fn take(&self, id: &QueuedId) -> Option<Queued> {
        let mut entries = self.entries.lock().unwrap();
        let at = entries.iter().position(|entry| &entry.id == id)?;
        let entry = entries.remove(at);
        if entry.shown {
            self.announce(&mut entries);
        }
        Some(Queued {
            text: entry.text,
            attachments: entry.attachments,
        })
    }

    /// Sends the whole list and marks every entry in it as shown.
    fn announce(&self, entries: &mut [Entry]) {
        let queued = entries
            .iter_mut()
            .map(|entry| {
                entry.shown = true;
                QueuedMessage {
                    id: entry.id.clone(),
                    text: entry.text.clone(),
                    attachments: entry.attachments.clone(),
                }
            })
            .collect();
        self.events.emit(AgentEvent::QueueUpdated { queued });
    }
}

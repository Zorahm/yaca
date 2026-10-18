//! The page's console and network, as `logs` reads them.
//!
//! Kept for the current document only. A new document starts a new page as
//! far as the model is concerned, and a stack of yesterday's errors under
//! today's would be read as today's. The document's own request is the first
//! entry of the new page: it is what failed when a page does not load.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::Value;

/// How many entries of each kind are kept: enough for a busy page, little
/// enough that one stuck in a loop cannot grow without end.
const KEEP: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleEntry {
    /// `log`, `info`, `warning`, `error`, `debug`, or `exception`.
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkEntry {
    pub id: String,
    pub method: String,
    pub url: String,
    pub kind: String,
    pub status: Option<u16>,
    pub failed: Option<String>,
}

impl NetworkEntry {
    pub fn is_error(&self) -> bool {
        self.failed.is_some() || self.status.is_some_and(|status| status >= 400)
    }
}

#[derive(Default)]
struct Book {
    console: VecDeque<ConsoleEntry>,
    network: VecDeque<NetworkEntry>,
    /// Things that happened to the page between two tool calls and that the
    /// next result should mention: a dialog answered, a download refused.
    notes: Vec<String>,
}

#[derive(Default)]
pub struct Logs(Mutex<Book>);

impl Logs {
    /// A new document begins: what was logged belongs to the old one.
    pub fn new_page(&self) {
        let mut book = self.0.lock().unwrap();
        book.console.clear();
        book.network.clear();
    }

    pub fn console(&self, level: &str, text: String) {
        let mut book = self.0.lock().unwrap();
        push(
            &mut book.console,
            ConsoleEntry {
                level: level.to_string(),
                text,
            },
        );
    }

    pub fn request(&self, entry: NetworkEntry) {
        push(&mut self.0.lock().unwrap().network, entry);
    }

    pub fn response(&self, id: &str, status: u16) {
        let mut book = self.0.lock().unwrap();
        if let Some(entry) = book.network.iter_mut().rev().find(|e| e.id == id) {
            entry.status = Some(status);
        }
    }

    pub fn failure(&self, id: &str, why: String) {
        let mut book = self.0.lock().unwrap();
        if let Some(entry) = book.network.iter_mut().rev().find(|e| e.id == id) {
            entry.failed = Some(why);
        }
    }

    pub fn note(&self, text: String) {
        self.0.lock().unwrap().notes.push(text);
    }

    /// The notes since the last call, which are then forgotten.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap().notes)
    }

    pub fn console_entries(&self) -> Vec<ConsoleEntry> {
        self.0.lock().unwrap().console.iter().cloned().collect()
    }

    pub fn network_entries(&self) -> Vec<NetworkEntry> {
        self.0.lock().unwrap().network.iter().cloned().collect()
    }
}

fn push<T>(queue: &mut VecDeque<T>, entry: T) {
    if queue.len() == KEEP {
        queue.pop_front();
    }
    queue.push_back(entry);
}

/// The text of a `Runtime.consoleAPICalled` argument list, the way the
/// DevTools console would print it on one line.
pub fn console_text(args: &[Value]) -> String {
    args.iter()
        .map(|arg| match arg.get("value") {
            Some(Value::String(text)) => text.clone(),
            Some(value) if !value.is_null() => value.to_string(),
            _ => arg
                .get("description")
                .and_then(Value::as_str)
                .or_else(|| arg.get("type").and_then(Value::as_str))
                .unwrap_or_default()
                .to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(id: &str) -> NetworkEntry {
        NetworkEntry {
            id: id.into(),
            method: "GET".into(),
            url: format!("https://example.com/{id}"),
            kind: "Fetch".into(),
            status: None,
            failed: None,
        }
    }

    #[test]
    fn answers_find_their_requests_and_a_new_page_starts_clean() {
        let logs = Logs::default();
        logs.request(request("1"));
        logs.request(request("2"));
        logs.response("1", 200);
        logs.failure("2", "net::ERR_FAILED".into());
        let entries = logs.network_entries();
        assert_eq!(entries[0].status, Some(200));
        assert!(!entries[0].is_error());
        assert!(entries[1].is_error());

        logs.console("error", "boom".into());
        logs.new_page();
        assert!(logs.network_entries().is_empty());
        assert!(logs.console_entries().is_empty());
    }

    #[test]
    fn only_the_newest_entries_are_kept() {
        let logs = Logs::default();
        for n in 0..KEEP + 3 {
            logs.console("log", n.to_string());
        }
        let entries = logs.console_entries();
        assert_eq!(entries.len(), KEEP);
        assert_eq!(entries[0].text, "3");
    }

    #[test]
    fn notes_are_read_once() {
        let logs = Logs::default();
        logs.note("a dialog was accepted".into());
        assert_eq!(logs.take_notes().len(), 1);
        assert!(logs.take_notes().is_empty());
    }

    #[test]
    fn console_arguments_read_like_the_console() {
        let args = [
            json!({ "type": "string", "value": "count:" }),
            json!({ "type": "number", "value": 3 }),
            json!({ "type": "object", "description": "Object" }),
            json!({ "type": "undefined" }),
        ];
        assert_eq!(console_text(&args), "count: 3 Object undefined");
    }
}

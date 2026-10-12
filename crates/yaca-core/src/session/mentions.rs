//! The `@` search as the session answers it (§7.2).
//!
//! The walk is blocking and can be long, so it runs on a task of its own: the
//! dispatcher goes on answering `Esc` while a project is being walked. An
//! answer that comes back after the human has typed on carries the query it
//! answers, and the interface drops it.

use std::sync::Arc;

use crate::event::AgentEvent;

use super::Shared;

pub(super) fn answer(shared: &Arc<Shared>, query: String) {
    let shared = shared.clone();
    tokio::spawn(async move {
        let ws = shared.ws.clone();
        let searched = query.clone();
        let paths = tokio::task::spawn_blocking(move || crate::mention::search(&ws, &searched))
            .await
            .unwrap_or_default();
        shared.emit(AgentEvent::FilesFound { query, paths });
    });
}

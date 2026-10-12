//! What a server thinks of a file the agent has just written.
//!
//! This is the part of the integration that works without being asked: the
//! model changes a file and, in the same result, reads what broke. Only when a
//! server for that file is already up — an edit does not start one, because a
//! one-line fix is not worth `rust-analyzer` loading a workspace.

use std::path::Path;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::format::render_diagnostics;
use super::hub::Lsp;

/// How long an edit waits for the server's verdict. Longer is a model idle
/// behind a compiler; the explicit `diagnostics` action waits longer.
const WAIT: Duration = Duration::from_secs(2);

/// A file bigger than this is not sent to a server after every edit.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

const MAX_LISTED: usize = 10;
const MESSAGE_CHARS: usize = 300;

impl Lsp {
    /// The note to add to an edit's result, or `None` when there is nothing
    /// to say: no server for the file, no server up, no errors.
    ///
    /// `shown` is how the feed names the file.
    pub async fn after_edit(
        &self,
        path: &Path,
        shown: &str,
        cancel: &CancellationToken,
    ) -> Option<String> {
        let name = self.server_for(path)?;
        let client = self.ready(&name)?;
        if std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
            return None;
        }
        let text = String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned();

        let before = client.published(&crate::uri::file_uri(path));
        let (uri, changed) = client.sync(path, &text).await.ok()?;
        let wait = if changed { WAIT } else { Duration::ZERO };
        let (items, arrived) = client.diagnostics(&uri, before, wait, cancel).await;

        if !arrived && changed {
            return Some(format!(
                "Диагностика {name} не пришла за {} с: сервер ещё не ответил. \
                 Запроси её через Lsp (action: diagnostics).",
                WAIT.as_secs()
            ));
        }
        let (listed, rest) = render_diagnostics(&items, shown, path, 1, MAX_LISTED, MESSAGE_CHARS);
        if listed.is_empty() {
            return None;
        }
        let total = listed.lines().count() + rest;
        let mut note = format!("Диагностика {name}: ошибок {total}\n{listed}");
        if rest > 0 {
            note.push_str(&format!("[и ещё {rest}]\n"));
        }
        Some(note.trim_end().to_string())
    }
}

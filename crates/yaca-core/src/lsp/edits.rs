//! What a server thinks of a file the agent has just written.
//!
//! This is the part of the integration that works without being asked: the
//! model changes a file and, in the same result, reads what broke. Only when a
//! server for that file is already up — an edit does not start one, because a
//! one-line fix is not worth `rust-analyzer` loading a workspace.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::client::Client;
use super::format::{render_diagnostics, severity};
use super::hub::Lsp;

/// How long an edit waits for the server's verdict. Longer is a model idle
/// behind a compiler; the explicit `diagnostics` action waits longer.
const WAIT: Duration = Duration::from_secs(2);
const FOLLOWUP_WAIT: Duration = Duration::from_secs(15);

/// A file bigger than this is not sent to a server after every edit.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

const MAX_LISTED: usize = 10;
const MESSAGE_CHARS: usize = 300;

pub(crate) struct PendingEditCheck {
    client: Arc<Client>,
    path: PathBuf,
    shown: String,
    name: String,
    uri: String,
    before: u64,
    version: i64,
}

impl PendingEditCheck {
    pub(crate) async fn finish(self, cancel: &CancellationToken) -> Option<String> {
        if !self
            .client
            .is_current_version(&self.uri, self.version)
            .await
        {
            return None;
        }
        let (items, arrived) = self
            .client
            .diagnostics(
                &self.uri,
                self.before,
                FOLLOWUP_WAIT,
                cancel,
                Some(self.version),
            )
            .await;
        if !arrived
            || cancel.is_cancelled()
            || !self
                .client
                .is_current_version(&self.uri, self.version)
                .await
        {
            return None;
        }
        Some(edit_report(&self.name, &self.shown, &self.path, &items))
    }
}

impl Lsp {
    /// The note to add to an edit's result, or `None` when there is nothing
    /// to say: no server for the file or no server up.
    ///
    /// `shown` is how the feed names the file.
    pub async fn after_edit(
        &self,
        path: &Path,
        shown: &str,
        cancel: &CancellationToken,
    ) -> Option<String> {
        self.check_after_edit(path, shown, cancel)
            .await
            .map(|(note, _)| note)
    }

    pub(crate) async fn check_after_edit(
        &self,
        path: &Path,
        shown: &str,
        cancel: &CancellationToken,
    ) -> Option<(String, Option<PendingEditCheck>)> {
        let name = self.server_for(path)?;
        let client = self.ready(&name)?;
        if std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
            return None;
        }
        let text = String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned();

        let before = client.published(&crate::uri::file_uri(path));
        let (uri, changed, version) = client.sync_versioned(path, &text).await.ok()?;
        let wait = if changed { WAIT } else { Duration::ZERO };
        let before = if changed { before } else { 0 };
        let (items, arrived) = client
            .diagnostics(&uri, before, wait, cancel, Some(version))
            .await;

        if !client.is_current_version(&uri, version).await {
            return None;
        }
        if !arrived {
            let note = format!(
                "Diagnostics from {name} did not arrive within {} s: server has not replied yet. \
                 Request them via Lsp (action: diagnostics).\nFile: {shown}",
                WAIT.as_secs()
            );
            return Some((
                note,
                Some(PendingEditCheck {
                    client,
                    path: path.to_path_buf(),
                    shown: shown.to_owned(),
                    name,
                    uri,
                    before,
                    version,
                }),
            ));
        }
        Some((edit_report(&name, shown, path, &items), None))
    }
}

fn edit_report(name: &str, shown: &str, path: &Path, items: &[Value]) -> String {
    let errors = items.iter().filter(|item| severity(item) == 1).count();
    let warnings = items.iter().filter(|item| severity(item) == 2).count();
    let (listed, rest) = render_diagnostics(items, shown, path, 2, MAX_LISTED, MESSAGE_CHARS);
    // Counts describe the complete report even when the listing is capped.
    let mut note = format!(
        "Diagnostics from {name}: {errors} errors, {warnings} warnings in {shown}\n{listed}"
    );
    if rest > 0 {
        note.push_str(&format!("[and {rest} more]\n"));
    }
    note.trim_end().to_string()
}

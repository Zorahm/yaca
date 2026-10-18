//! The human's "yes" to a server the project's `.mcp.json` declares.
//!
//! Why such a server needs one is in [`config`](super::config): the file
//! arrives with the repository, and a stdio server is a command. The approval
//! is for one declaration of one project, by fingerprint, so a changed command
//! is a different thing to approve.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::config::ServerSpec;

/// Which servers are approved, in the configuration directory.
const APPROVALS_FILE: &str = "mcp-approved.json";

/// The human's "yes" to servers a project file declares.
///
/// Kept outside the project, in the configuration directory: an approval
/// stored next to `.mcp.json` could be committed along with it.
#[derive(Debug)]
pub(crate) struct Approvals {
    /// `None`: nothing is kept between sessions, and an approval lasts as long
    /// as the session that was given it.
    file: Option<PathBuf>,
    project: String,
}

type ApprovalBook = BTreeMap<String, BTreeMap<String, String>>;

impl Approvals {
    pub(crate) fn new(config_dir: Option<&Path>, project: &Path) -> Self {
        Approvals {
            file: config_dir.map(|dir| dir.join(APPROVALS_FILE)),
            project: project.to_string_lossy().to_string(),
        }
    }

    fn book(&self) -> ApprovalBook {
        self.file
            .as_ref()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub(crate) fn covers(&self, spec: &ServerSpec) -> bool {
        self.book()
            .get(&self.project)
            .and_then(|servers| servers.get(&spec.name))
            .is_some_and(|print| *print == spec.settings.fingerprint())
    }

    /// Writes the approval down. A failure to write is not a failure to
    /// approve: the session goes on with the server, and the next one asks
    /// again.
    pub(crate) fn record(&self, spec: &ServerSpec) {
        let Some(file) = &self.file else {
            return;
        };
        let mut book = self.book();
        book.entry(self.project.clone())
            .or_default()
            .insert(spec.name.clone(), spec.settings.fingerprint());
        let written = file
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| {
                let text = serde_json::to_string_pretty(&book).map_err(std::io::Error::other)?;
                std::fs::write(file, text)
            });
        if let Err(err) = written {
            tracing::warn!(%err, file = %file.display(), "MCP approval was not written down");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::{McpServerSettings, McpSource};
    use crate::testutil::TempDir;

    fn stdio(command: &str) -> McpServerSettings {
        McpServerSettings {
            command: Some(command.into()),
            ..Default::default()
        }
    }

    #[test]
    fn an_approval_holds_until_the_declaration_changes() {
        let dir = TempDir::new();
        let approvals = Approvals::new(Some(dir.path()), Path::new("/p"));
        let mut spec = ServerSpec {
            name: "files".into(),
            source: McpSource::ProjectFile,
            plugin: None,
            app: None,
            settings: stdio("server-files"),
        };
        assert!(!approvals.covers(&spec));
        approvals.record(&spec);
        assert!(approvals.covers(&spec));

        // The same name in another project is another server.
        assert!(!Approvals::new(Some(dir.path()), Path::new("/q")).covers(&spec));

        spec.settings.args.push("--write".into());
        assert!(!approvals.covers(&spec), "a changed command asks again");
    }

    #[test]
    fn without_a_place_to_keep_them_approvals_are_not_kept() {
        let approvals = Approvals::new(None, Path::new("/p"));
        let spec = ServerSpec {
            name: "files".into(),
            source: McpSource::ProjectFile,
            plugin: None,
            app: None,
            settings: stdio("server-files"),
        };
        approvals.record(&spec);
        assert!(!approvals.covers(&spec));
    }
}

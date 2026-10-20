//! MCP servers, as the settings window and the chat need them (§10).
//!
//! Three lists, and this file is the door to two of them. The human's own
//! servers live in `mcp.json` in the configuration directory, next to the
//! provider list and for the same reason: the core reads them. A project's
//! `.mcp.json` is shown and never written — it belongs to the repository and
//! to whatever other tool put it there. The third list is `mcpServers` in the
//! project's `.yaca/settings.json`, which the window already reads and writes
//! whole through `project`.
//!
//! What is declared is not what is running. The state of a chat's servers
//! comes with its events (`McpUpdated`), and [`session_mcp`] is the same list
//! for a window that was not listening when it last changed.

use std::collections::BTreeMap;

use yaca_core::mcp::config;
use yaca_core::{McpServerSettings, McpServerSummary};

use crate::Host;

type Servers = BTreeMap<String, McpServerSettings>;

/// The human's own servers. A file that does not parse is an error, not an
/// empty list: the window must not offer to overwrite servers it failed to
/// read (§0).
pub fn global_mcp_servers() -> Result<Servers, String> {
    config::load_global(&crate::config_dir()?).map_err(|e| e.to_string())
}

/// Writes the human's own servers whole. They reach the next chat that
/// opens: a running one keeps the servers it started with.
pub fn save_global_mcp_servers(servers: Servers) -> Result<(), String> {
    config::save_global(&crate::config_dir()?, &servers).map_err(|e| e.to_string())
}

/// What the project's `.mcp.json` declares, for showing beside the project's
/// own list.
pub fn project_mcp_file(project: String) -> Result<Servers, String> {
    config::load_project_file(&crate::project_path(&project)?).map_err(|e| e.to_string())
}

/// The servers of an open chat as they are now.
pub fn session_mcp(host: &Host, session: &str) -> Result<Vec<McpServerSummary>, String> {
    Ok(host.chat(session)?.mcp())
}

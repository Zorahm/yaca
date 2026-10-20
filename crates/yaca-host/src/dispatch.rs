//! Commands by name.
//!
//! A shell that is not in the same process cannot call a Rust function; it
//! sends a name and arguments. The names are the ones the interface has always
//! used (`open_project`, `send_command`…) and the arguments are the object it
//! has always passed, keys spelled as the webview spells them. One table, so
//! the desktop and a remote client reach the same commands the same way.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::{files, mcp, plugins, project, providers, routing, sessions, update, Host};

/// The arguments of one call.
struct Args(Value);

impl Args {
    /// One argument by key. A missing key reads as `null`, so an `Option`
    /// argument may be left out and a required one says which it was.
    fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, String> {
        let value = self.0.get(key).cloned().unwrap_or(Value::Null);
        serde_json::from_value(value).map_err(|e| format!("argument \"{key}\": {e}"))
    }
}

/// A command's answer as the wire carries it.
fn wire<T: Serialize>(answer: T) -> Result<Value, String> {
    serde_json::to_value(answer).map_err(|e| e.to_string())
}

fn wire_result<T: Serialize>(answer: Result<T, String>) -> Result<Value, String> {
    answer.and_then(wire)
}

impl Host {
    /// Runs the command `cmd`. The error is the text the window shows.
    pub async fn call(self: &Arc<Self>, cmd: &str, args: Value) -> Result<Value, String> {
        let a = Args(args);
        match cmd {
            // Projects and chats.
            "projects" => wire_result(self.projects()),
            "add_project" => wire_result(self.add_project(&a.get::<String>("path")?)),
            "forget_project" => wire_result(self.forget_project(&a.get::<String>("path")?)),
            "project_startup" => wire_result(self.project_startup(&a.get::<String>("path")?)),
            "open_project" => wire_result(
                self.open_project(a.get("path")?, a.get("resume")?, a.get("startup")?)
                    .await,
            ),
            "close_chat" => wire_result(self.close_chat(&a.get::<String>("session")?).await),
            "attach_chats" => wire_result(self.attach_chats().await),
            "send_command" => wire_result(
                self.send_command(&a.get::<String>("session")?, a.get("command")?)
                    .await,
            ),
            "session_state" => wire(self.session_state(&a.get::<String>("session")?)),
            "session_meta" => wire(self.session_meta(&a.get::<String>("session")?)),
            "user_name" => wire(self.user_name()),

            // Recorded sessions.
            "list_sessions" => wire_result(sessions::list_sessions(&a.get::<String>("project")?)),
            "search_sessions" => wire_result(sessions::search_sessions(
                &a.get::<String>("project")?,
                &a.get::<String>("query")?,
                a.get("limit")?,
            )),
            "rename_session" => wire_result(self.rename_session(
                &a.get::<String>("project")?,
                a.get("id")?,
                &a.get::<String>("title")?,
            )),
            "delete_session" => wire_result(
                self.delete_session(&a.get::<String>("project")?, a.get("id")?)
                    .await,
            ),
            "duplicate_session" => wire_result(sessions::duplicate_session(
                &a.get::<String>("project")?,
                a.get("id")?,
                a.get("through")?,
            )),
            "export_session" => wire_result(sessions::export_session(
                &a.get::<String>("project")?,
                a.get("id")?,
                &a.get::<String>("path")?,
            )),
            "export_session_text" => wire_result(sessions::export_session_text(
                &a.get::<String>("project")?,
                a.get("id")?,
            )),
            "session_feed" => wire_result(sessions::session_feed(&a.get::<String>("project")?, a.get("id")?)),
            "project_commands" => wire_result(sessions::project_commands(&a.get::<String>("project")?)),
            "project_skills" => wire_result(sessions::project_skills(&a.get::<String>("project")?)),

            // Plugins.
            "list_plugins" => wire_result(plugins::list_plugins()),
            "set_plugin_enabled" => {
                wire_result(plugins::set_plugin_enabled(&a.get::<String>("id")?, a.get("enabled")?))
            }
            "list_marketplaces" => wire_result(plugins::list_marketplaces().await),
            "add_marketplace" => wire_result(plugins::add_marketplace(a.get("source")?).await),
            "remove_marketplace" => wire_result(plugins::remove_marketplace(a.get("name")?).await),
            "refresh_marketplace" => wire_result(plugins::refresh_marketplace(a.get("name")?).await),
            "marketplace_catalog" => wire_result(plugins::marketplace_catalog(a.get("name")?).await),
            "install_plugin" => wire_result(plugins::install_plugin(a.get("id")?).await),
            "update_plugin" => wire_result(plugins::update_plugin(a.get("id")?).await),
            "uninstall_plugin" => wire_result(plugins::uninstall_plugin(a.get("id")?).await),

            // The host's disk.
            "installed_shells" => wire(yaca_core::process::shell::installed()),
            "attachment_look" => wire_result(files::attachment_look(a.get("project")?, a.get("path")?).await),
            "image_full" => wire_result(files::image_full(a.get("project")?, a.get("path")?).await),
            "stash_image" => wire_result(files::stash_image(a.get("project")?, a.get("data")?).await),
            "list_dir" => wire_result(files::list_dir(a.get("path")?, a.get::<Option<bool>>("hidden")?.unwrap_or(false))),
            "make_dir" => wire_result(files::make_dir(&a.get::<String>("parent")?, &a.get::<String>("name")?)),

            // Providers, keys, models.
            "providers" => wire_result(providers::providers()),
            "save_provider" => wire_result(providers::save_provider(self, a.get("spec")?).await),
            "delete_provider" => wire_result(providers::delete_provider(self, a.get("id")?)),
            "discover_models" => wire_result(providers::discover_models(a.get("spec")?).await),
            "set_api_key" => wire_result(providers::set_api_key(self, a.get("provider")?, a.get("key")?).await),
            "forget_api_key" => wire_result(providers::forget_api_key(a.get("provider")?)),
            "api_key_source" => wire_result(providers::api_key_source(a.get("provider")?)),
            "search_key_source" => wire(providers::search_key_source()),
            "models" => wire_result(providers::models()),
            "model_residency" => wire_result(providers::model_residency(a.get("provider")?).await),
            "routing_status" => wire_result(routing::routing_status()),
            "routing_text" => wire_result(routing::routing_text()),
            "save_routing" => wire_result(routing::save_routing(self, a.get("text")?)),
            "remove_routing" => wire_result(routing::remove_routing(self)),

            // Preferences and updates.
            "preferences" => wire(self.preferences()),
            "set_preferences" => wire_result(self.set_preferences(a.get("prefs")?)),
            "check_for_updates" => wire_result(update::check_for_updates(self).await),
            "update_status" => wire(update::update_status(self)),

            // One project's settings and MCP servers.
            "project_settings" => wire_result(project::project_settings(a.get("project")?)),
            "save_project_settings" => {
                wire_result(project::save_project_settings(a.get("project")?, a.get("settings")?))
            }
            "project_instructions" => wire_result(project::project_instructions(a.get("project")?)),
            "shadow_excluded_dirs" => wire(project::shadow_excluded_dirs()),
            "global_mcp_servers" => wire_result(mcp::global_mcp_servers()),
            "save_global_mcp_servers" => wire_result(mcp::save_global_mcp_servers(a.get("servers")?)),
            "project_mcp_file" => wire_result(mcp::project_mcp_file(a.get("project")?)),
            "session_mcp" => wire_result(mcp::session_mcp(self, &a.get::<String>("session")?)),

            other => Err(format!("unknown command \"{other}\"")),
        }
    }
}

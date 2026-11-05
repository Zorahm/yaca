//! Explicit installation and remembered per-project switches.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use super::config::{self, ServerSpec};
use super::hub::{Lsp, State};
use super::runtime::{read_preferences, use_managed_program};
use crate::event::AgentEvent;

pub(super) struct Installation {
    program: &'static str,
    args: Vec<String>,
    bin: Option<std::path::PathBuf>,
}

impl Installation {
    pub(super) fn display(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|arg| {
                if arg.contains(char::is_whitespace) {
                    format!("\"{arg}\"")
                } else {
                    arg.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("{} {args}", self.program)
    }
}

pub(super) fn installation(spec: &ServerSpec, dir: &Path) -> Option<Installation> {
    let defaults = config::builtin(&spec.name)?;
    let managed = super::runtime::managed_bin(dir, &spec.name, &defaults.command);
    if spec.settings.command != defaults.command && Path::new(&spec.settings.command) != managed {
        return None;
    }
    let prefix = dir.join("lsp").join("tools").join(&spec.name);
    let (program, args, bin) = match spec.name.as_str() {
        "rust-analyzer" => (
            "rustup",
            vec!["component".into(), "add".into(), "rust-analyzer".into()],
            None,
        ),
        "typescript-language-server" | "pyright" => {
            let mut args = vec![
                "install".into(),
                "--prefix".into(),
                prefix.to_string_lossy().into_owned(),
                "--no-audit".into(),
                "--no-fund".into(),
            ];
            if spec.name == "pyright" {
                args.push("pyright".into());
            } else {
                args.extend(["typescript-language-server".into(), "typescript@5".into()]);
            }
            ("npm", args, None)
        }
        "gopls" => (
            "go",
            vec!["install".into(), "golang.org/x/tools/gopls@latest".into()],
            Some(prefix),
        ),
        _ => return None,
    };
    Some(Installation { program, args, bin })
}

impl Lsp {
    pub fn set_enabled(&self, name: &str, enabled: bool) {
        let result = (|| -> Result<(), String> {
            let mut servers = self.0.servers.lock().unwrap();
            let server = servers
                .get_mut(name)
                .ok_or_else(|| format!("no server \"{name}\""))?;
            if let Some((_, file)) = &self.0.managed {
                let mut disabled = read_preferences(file).map_err(|e| e.to_string())?;
                disabled.insert(name.to_string(), !enabled);
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let text = serde_json::to_vec_pretty(&disabled).map_err(|e| e.to_string())?;
                let temp = file.with_extension("tmp");
                std::fs::write(&temp, text).map_err(|e| e.to_string())?;
                std::fs::rename(&temp, file).map_err(|e| e.to_string())?;
            }
            server.spec.settings.disabled = !enabled;
            server.cancel.cancel();
            server.attempt += 1;
            if let State::Ready(client) = &server.state {
                client.stop();
            }
            server.state = State::Idle;
            Ok(())
        })();
        if let Err(reason) = result {
            self.control_error(reason);
            return;
        }
        self.0.announce();
        if enabled {
            self.start();
        }
    }

    pub fn install(&self, name: &str) {
        let Some((dir, _)) = &self.0.managed else {
            return;
        };
        let (plan, attempt, cancel) = {
            let mut servers = self.0.servers.lock().unwrap();
            let Some(server) = servers.get_mut(name) else {
                return;
            };
            if server.spec.settings.disabled
                || matches!(
                    server.state,
                    State::Starting | State::Installing | State::Ready(_)
                )
            {
                return;
            }
            let Some(plan) = installation(&server.spec, dir) else {
                return;
            };
            server.cancel.cancel();
            server.cancel = tokio_util::sync::CancellationToken::new();
            server.attempt += 1;
            server.state = State::Installing;
            (plan, server.attempt, server.cancel.clone())
        };
        self.0.announce();
        let lsp = self.clone();
        let name = name.to_string();
        let dir = dir.clone();
        let root = self.0.root.clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                result = run_installation(&plan, &dir, &root) => result,
                _ = cancel.cancelled() => return,
            };
            {
                let mut servers = lsp.0.servers.lock().unwrap();
                let Some(server) = servers.get_mut(&name) else {
                    return;
                };
                if server.attempt != attempt {
                    return;
                }
                match result {
                    Ok(()) => {
                        use_managed_program(&mut server.spec, &dir);
                        server.state = if config::on_path(&server.spec.settings.command) {
                            State::Idle
                        } else {
                            State::Failed(format!(
                                "Installation completed but {} was not found",
                                server.spec.settings.command
                            ))
                        };
                    }
                    Err(reason) => {
                        server.state = State::Failed(reason);
                    }
                }
            }
            lsp.0.announce();
            lsp.start();
        });
    }

    fn control_error(&self, message: String) {
        // Use the same sinks as status updates, including every open chat.
        self.0.emit(AgentEvent::Error {
            code: "lsp_control".into(),
            message,
            retryable: false,
        });
    }
}

async fn run_installation(plan: &Installation, dir: &Path, root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if let Some(bin) = &plan.bin {
        std::fs::create_dir_all(bin).map_err(|e| e.to_string())?;
    }
    let mut command =
        tokio::process::Command::new(crate::mcp::transport::stdio::resolve_program(plan.program));
    command
        .args(&plan.args)
        // Rustup must use the project's toolchain override, just like the
        // rust-analyzer shim will when the server starts in this tree.
        .current_dir(if plan.program == "rustup" { root } else { dir })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(bin) = &plan.bin {
        command.env("GOBIN", bin);
    }
    #[cfg(windows)]
    command.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
    #[cfg(unix)]
    command.process_group(0);
    let child = command.spawn().map_err(|e| {
        format!(
            "Cannot start {}: {e}. Install the required toolchain and try again.",
            plan.program
        )
    })?;
    let mut tree = InstallationTree(child.id());
    let output = tokio::time::timeout(Duration::from_secs(300), child.wait_with_output())
        .await
        .map_err(|_| "Language server installation timed out after 300 s".to_string())?
        .map_err(|e| {
            format!(
                "Cannot start {}: {e}. Install the required toolchain and try again.",
                plan.program
            )
        })?;
    tree.0 = None;
    if output.status.success() {
        return Ok(());
    }
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    Err(format!(
        "{} failed ({}):\n{}",
        plan.display(),
        output.status,
        log.chars().take(8000).collect::<String>()
    ))
}

struct InstallationTree(Option<u32>);

impl Drop for InstallationTree {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            std::thread::spawn(move || crate::process::kill::tree(pid));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn installation_keeps_packages_in_the_application_and_preserves_custom_commands() {
        let specs = config::resolve(&BTreeMap::new(), |_| true);
        let dir = Path::new("application config");
        let mut typescript = specs
            .into_iter()
            .find(|spec| spec.name == "typescript-language-server")
            .unwrap();
        let plan = installation(&typescript, dir).unwrap();
        assert_eq!(plan.program, "npm");
        assert!(plan.args.contains(&"--prefix".into()));
        assert!(plan.args.contains(&"typescript@5".into()));
        assert!(!plan.args.contains(&"-g".into()));
        typescript.settings.command = "custom-typescript".into();
        assert!(installation(&typescript, dir).is_none());
    }
}

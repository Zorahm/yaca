//! Starting one plugin hook and waiting for its answer.
//!
//! Its own runner rather than `process::shell`, which merges the two streams
//! the way a terminal shows them. A Claude Code hook answers on stdout (often
//! JSON) and explains a refusal on stderr; merged, neither can be read.
//!
//! Bash for a command line, on Windows too: that is what Claude Code runs them
//! through, and the hooks on a real machine are written for it (`[ -f … ] ||`,
//! `"${CLAUDE_PLUGIN_ROOT}/…"`). On Windows that is Git Bash, never the WSL
//! launcher in System32, which would run the hook in another machine's
//! filesystem. No bash, and the hook does not start; the reason says so.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use super::{HookOwner, HookRun, PluginHook};
use crate::process::kill;
use crate::workspace::roots::plain;

/// What of each stream is kept. A hook's answer is a line of JSON or a
/// paragraph; past this it is a log, and the rest is read and dropped.
const CAPTURE_BYTES: usize = 64 * 1024;
const REAP_SECS: u64 = 5;

/// How a hook ended.
pub enum Ran {
    Exited {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut(u64),
    Broken(String),
    Stopped,
    /// An `async` hook: started and left to finish on its own.
    Detached,
}

pub async fn run(
    hook: &PluginHook,
    owner: &HookOwner,
    project: &Path,
    stdin: String,
    cancel: &CancellationToken,
) -> Ran {
    let mut cmd = match command(&hook.run, owner) {
        Ok(cmd) => cmd,
        Err(why) => return Ran::Broken(why),
    };
    let unix_style = matches!(hook.run, HookRun::Shell { powershell: false, .. });
    let root = path_for(&owner.root, unix_style);
    let data = path_for(&owner.data, unix_style);
    let _ = std::fs::create_dir_all(&owner.data);
    cmd.current_dir(project)
        .env("CLAUDE_PLUGIN_ROOT", &root)
        .env("PLUGIN_ROOT", &root)
        .env("CLAUDE_PLUGIN_DATA", &data)
        .env("PLUGIN_DATA", &data)
        .env("CLAUDE_PROJECT_DIR", path_for(project, unix_style))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return Ran::Broken(e.to_string()),
    };
    let pid = child.id();
    if let Some(mut pipe) = child.stdin.take() {
        // Its own task: a hook that answers before reading everything would
        // otherwise wait on us while we wait on it.
        tokio::spawn(async move {
            let _ = pipe.write_all(stdin.as_bytes()).await;
            let _ = pipe.shutdown().await;
        });
    }
    let out = tokio::spawn(read_all(child.stdout.take()));
    let err = tokio::spawn(read_all(child.stderr.take()));
    let limit = hook.timeout;
    let deadline = tokio::time::Instant::now() + limit;

    if hook.background {
        // Bounded all the same: a background hook that never ends must not
        // outlive its timeout.
        tokio::spawn(async move {
            if tokio::time::timeout(limit, child.wait()).await.is_err() {
                if let Some(pid) = pid {
                    let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
                }
            }
        });
        return Ran::Detached;
    }

    let halt = tokio::select! {
        biased;
        _ = cancel.cancelled() => Some(Ran::Stopped),
        _ = tokio::time::sleep_until(deadline) => Some(Ran::TimedOut(limit.as_secs())),
        status = child.wait() => match status {
            Ok(status) => {
                // Up to the deadline, not to EOF: something the hook left
                // running in the background can hold its pipes open forever.
                let stdout = tokio::time::timeout_at(deadline, out).await.ok().and_then(Result::ok);
                let stderr = tokio::time::timeout_at(deadline, err).await.ok().and_then(Result::ok);
                return Ran::Exited {
                    code: status.code(),
                    stdout: stdout.unwrap_or_default(),
                    stderr: stderr.unwrap_or_default(),
                };
            }
            Err(e) => Some(Ran::Broken(e.to_string())),
        },
    };
    // The tree, not the shell: what the hook started must not run on after the
    // answer has stopped mattering.
    if let Some(pid) = pid {
        let _ = tokio::task::spawn_blocking(move || kill::tree(pid)).await;
    }
    let _ = tokio::time::timeout(std::time::Duration::from_secs(REAP_SECS), child.wait()).await;
    halt.unwrap_or(Ran::Stopped)
}

fn command(run: &HookRun, owner: &HookOwner) -> Result<tokio::process::Command, String> {
    let root = plain(&owner.root);
    let fill = |text: &str| super::super::claude::substitute(text, &root);
    let mut cmd = match run {
        HookRun::Exec { program, args } => {
            let mut cmd = std::process::Command::new(fill(program));
            cmd.args(args.iter().map(|a| fill(a)));
            cmd
        }
        HookRun::Shell { command, powershell: true } => {
            let mut cmd = std::process::Command::new(if cfg!(windows) { "powershell.exe" } else { "pwsh" });
            // PowerShell reads `${NAME}` as one of its own variables, not the
            // environment's, so the plugin's paths go in as text.
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", &fill(command)]);
            cmd
        }
        HookRun::Shell { command, powershell: false } => {
            let bash = bash().ok_or("bash was not found (on Windows, Git Bash is needed)")?;
            let mut cmd = std::process::Command::new(bash);
            cmd.arg("-c").arg(command);
            cmd
        }
    };
    hide(&mut cmd);
    Ok(tokio::process::Command::from(cmd))
}

#[cfg(windows)]
fn hide(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn hide(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // Its own group, so `kill::tree` reaches whatever it starts.
    cmd.process_group(0);
}

/// Forward slashes on Windows for what bash reads: `C:/Users/…` works in Git
/// Bash where a backslash would start an escape.
fn path_for(path: &Path, unix_style: bool) -> String {
    let text = plain(path);
    if unix_style || !cfg!(windows) {
        text
    } else {
        text.replace('/', "\\")
    }
}

async fn read_all(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> String {
    let Some(mut pipe) = pipe else { return String::new() };
    let mut kept = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = CAPTURE_BYTES.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(room)]);
            }
        }
    }
    String::from_utf8_lossy(&kept).replace("\r\n", "\n").trim_end().to_string()
}

/// Where bash is, looked for once per process.
pub fn bash() -> Option<&'static Path> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND.get_or_init(find_bash).as_deref()
}

#[cfg(unix)]
fn find_bash() -> Option<PathBuf> {
    on_path("bash")
}

#[cfg(windows)]
fn find_bash() -> Option<PathBuf> {
    // The variable Claude Code itself reads, so a machine set up for it works.
    if let Some(p) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let program_files = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .map(|base| base.join("Git").join("bin").join("bash.exe"));
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("Programs").join("Git").join("bin").join("bash.exe"));
    // `git.exe` lives in `<Git>\cmd`; bash is beside it in `<Git>\bin`.
    let beside_git = on_path("git.exe")
        .and_then(|git| git.parent()?.parent().map(|g| g.join("bin").join("bash.exe")));
    program_files
        .chain(local)
        .chain(beside_git)
        .find(|p| p.is_file())
        .or_else(|| on_path("bash.exe").filter(|p| !is_wsl_launcher(p)))
}

#[cfg(windows)]
fn is_wsl_launcher(path: &Path) -> bool {
    let text = path.display().to_string().to_ascii_lowercase();
    text.contains("\\windows\\system32\\") || text.contains("\\windowsapps\\")
}

fn on_path(program: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|p| p.is_file())
}

//! A server that is a child process: one JSON message per line on its standard
//! input and output, and whatever it likes on its standard error.
//!
//! Started directly rather than through a shell. A declaration is a program
//! and its arguments, already split, and a shell in between would split them
//! again by its own rules.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::process::Stdio as Pipe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::{Inbox, Incoming, Transport};
use crate::mcp::McpError;
use crate::process::kill;

/// How much of what the server wrote to its standard error is kept for the
/// message about its death.
const STDERR_TAIL_LINES: usize = 12;

type Tail = Arc<Mutex<VecDeque<String>>>;

pub(crate) struct Stdio {
    /// Lines for the writer task. A queue rather than the pipe itself, so that
    /// a caller that gives up mid-send cannot leave half a line in the pipe.
    out: mpsc::UnboundedSender<String>,
    pid: Option<u32>,
    /// The task that owns the child. Aborting it drops the child, and the
    /// child is killed on drop.
    owner: AbortHandle,
}

impl Stdio {
    pub(crate) fn spawn(
        program: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: &Path,
        inbox: Inbox,
    ) -> Result<Self, McpError> {
        let mut cmd = Command::new(resolve_program(program));
        cmd.args(args)
            .envs(env)
            .current_dir(cwd)
            .stdin(Pipe::piped())
            .stdout(Pipe::piped())
            .stderr(Pipe::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
        // Its own group, so that stopping it reaches what it started
        // (`process::kill`).
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Transport(format!("cannot start «{program}»: {e}")))?;
        let pid = child.id();
        // All three were asked for as pipes just above.
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(McpError::Transport(format!("«{program}» has no standard streams")));
        };

        let tail = Tail::default();
        tokio::spawn(keep_tail(stderr, tail.clone()));
        let (out, lines) = mpsc::unbounded_channel();
        tokio::spawn(write_lines(stdin, lines));
        let owner = tokio::spawn(read_lines(child, stdout, tail, inbox)).abort_handle();

        Ok(Stdio { out, pid, owner })
    }
}

#[async_trait]
impl Transport for Stdio {
    async fn send(&self, message: Value) -> Result<(), McpError> {
        // One message is one line: the encoder never writes a raw newline.
        self.out
            .send(message.to_string())
            .map_err(|_| McpError::Closed("the server process exited".into()))
    }
}

impl Drop for Stdio {
    fn drop(&mut self) {
        let owner = self.owner.clone();
        let pid = self.pid;
        // Off this thread: on Windows the tree is killed by running `taskkill`.
        // The tree first and the child after — once the parent is gone its
        // descendants can no longer be found through it, and `npx` leaves the
        // real server running as one of those.
        std::thread::spawn(move || {
            if let Some(pid) = pid {
                kill::tree(pid);
            }
            owner.abort();
        });
    }
}

async fn write_lines(mut stdin: ChildStdin, mut lines: mpsc::UnboundedReceiver<String>) {
    while let Some(mut line) = lines.recv().await {
        line.push('\n');
        if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
            return;
        }
    }
}

/// The next line, whatever bytes it holds. `None` at the end of the stream.
///
/// Bytes and not `lines()`: that one stops at the first byte that is not
/// UTF-8, and on Windows a program's complaint arrives in the console's
/// codepage. A line of mojibake is worth more than a stream cut short.
async fn next_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Option<String> {
    let mut raw = Vec::new();
    match reader.read_until(b'\n', &mut raw).await {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(String::from_utf8_lossy(&raw).trim().to_string()),
    }
}

async fn read_lines(mut child: Child, stdout: ChildStdout, tail: Tail, inbox: Inbox) {
    let mut stdout = BufReader::new(stdout);
    while let Some(line) = next_line(&mut stdout).await {
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(&line) {
            Ok(message) => {
                if inbox.send(Incoming::Message(message)).is_err() {
                    return;
                }
            }
            // A server that prints a banner to its standard output is breaking
            // the protocol, but one stray line is no reason to lose the server.
            Err(_) => tracing::debug!(line, "MCP server wrote a line that is not JSON"),
        }
    }

    // The output closed: the process is ending or has ended. A moment for the
    // exit code and for the last of its standard error to be read.
    let code = tokio::time::timeout(Duration::from_secs(1), child.wait())
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|status| status.code());
    let said = tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join("\n");

    let mut reason = match code {
        Some(code) => format!("the server process exited with code {code}"),
        None => "the server process exited".to_string(),
    };
    if !said.is_empty() {
        reason.push_str(":\n");
        reason.push_str(&said);
    }
    let _ = inbox.send(Incoming::Closed(reason));
}

async fn keep_tail(stderr: tokio::process::ChildStderr, tail: Tail) {
    let mut stderr = BufReader::new(stderr);
    while let Some(line) = next_line(&mut stderr).await {
        if line.is_empty() {
            continue;
        }
        tracing::debug!(line, "MCP server stderr");
        let mut tail = tail.lock().unwrap();
        if tail.len() == STDERR_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
}

/// The program as the system can start it.
///
/// On Windows a process is started from a file with its extension, and the
/// usual servers are `npx` and `uvx` — `npx.cmd` on disk. A shell would find
/// it through `PATHEXT`; started directly, it has to be found here.
#[cfg(windows)]
pub(crate) fn resolve_program(program: &str) -> std::path::PathBuf {
    use std::path::PathBuf;

    let given = Path::new(program);
    if given.extension().is_some() || given.components().count() > 1 {
        return given.to_path_buf();
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    let Some(path) = std::env::var_os("PATH") else {
        return given.to_path_buf();
    };
    for dir in std::env::split_paths(&path) {
        for ext in extensions.split(';').filter(|ext| !ext.is_empty()) {
            let candidate: PathBuf = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    given.to_path_buf()
}

#[cfg(not(windows))]
pub(crate) fn resolve_program(program: &str) -> std::path::PathBuf {
    Path::new(program).to_path_buf()
}

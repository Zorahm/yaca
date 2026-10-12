//! A language server as a child process: JSON-RPC messages framed by a
//! `Content-Length` header, on its standard input and output.
//!
//! This is not the framing of MCP (one message per line), which is why the
//! transport is not shared. What is shared is how the child is started and
//! stopped: directly, without a shell, and ended together with its tree.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::LspError;
use crate::mcp::transport::stdio::resolve_program;
use crate::process::kill;

/// How much of the server's standard error is kept for the message about its
/// death.
const STDERR_TAIL_LINES: usize = 12;

/// A header line longer than this is not a header.
const MAX_HEADER_BYTES: usize = 8_192;

/// A single message bigger than this is a server gone wrong. Workspace symbol
/// answers on a large project are megabytes, so the ceiling is generous.
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

/// What the wire hands up.
#[derive(Debug)]
pub(crate) enum Incoming {
    Message(Value),
    /// The server is gone, with why as far as it can be told. Nothing arrives
    /// after this.
    Closed(String),
}

pub(crate) type Inbox = mpsc::UnboundedSender<Incoming>;

type Tail = Arc<Mutex<VecDeque<String>>>;

pub(crate) struct Wire {
    out: mpsc::UnboundedSender<Vec<u8>>,
    pid: Option<u32>,
    owner: AbortHandle,
}

impl Wire {
    pub(crate) fn spawn(
        program: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: &Path,
        inbox: Inbox,
    ) -> Result<Wire, LspError> {
        let mut cmd = Command::new(resolve_program(program));
        cmd.args(args)
            .envs(env)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(crate::process::shell::CREATE_NO_WINDOW);
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd
            .spawn()
            .map_err(|e| LspError::Transport(format!("не запустить «{program}»: {e}")))?;
        let pid = child.id();
        // All three were asked for as pipes just above.
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(LspError::Transport(format!("у «{program}» нет стандартных потоков")));
        };

        let tail = Tail::default();
        tokio::spawn(keep_tail(stderr, tail.clone()));
        let (out, frames) = mpsc::unbounded_channel();
        tokio::spawn(write_frames(stdin, frames));
        let owner = tokio::spawn(read_frames(child, stdout, tail, inbox)).abort_handle();
        Ok(Wire { out, pid, owner })
    }

    pub(crate) fn send(&self, message: &Value) -> Result<(), LspError> {
        self.out
            .send(encode(message))
            .map_err(|_| LspError::Closed("процесс сервера завершился".into()))
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        let owner = self.owner.clone();
        let pid = self.pid;
        // Off this thread: on Windows the tree is killed by running `taskkill`.
        std::thread::spawn(move || {
            if let Some(pid) = pid {
                kill::tree(pid);
            }
            owner.abort();
        });
    }
}

/// One message with its header.
pub(crate) fn encode(message: &Value) -> Vec<u8> {
    let body = message.to_string();
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(body.as_bytes());
    frame
}

/// The next message. `Ok(None)` is the end of the stream; `Err` is a stream
/// that cannot be read on from, because there is no telling where the next
/// message starts.
///
/// A body that is not JSON is skipped rather than fatal: its length was
/// known, so the stream is still in step.
pub(crate) async fn read_message(
    reader: &mut (impl AsyncBufRead + Unpin),
) -> Result<Option<Value>, String> {
    loop {
        let mut length: Option<usize> = None;
        let mut seen_any = false;
        loop {
            let mut line = Vec::new();
            let n = reader
                .take(MAX_HEADER_BYTES as u64)
                .read_until(b'\n', &mut line)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return if seen_any {
                    Err("поток оборвался посреди заголовка".into())
                } else {
                    Ok(None)
                };
            }
            seen_any = true;
            let text = String::from_utf8_lossy(&line);
            let text = text.trim_end_matches(['\r', '\n']);
            if text.is_empty() {
                break;
            }
            if !line.ends_with(b"\n") {
                return Err("заголовок длиннее допустимого".into());
            }
            if let Some((name, value)) = text.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().ok();
                }
            }
        }

        let Some(length) = length else {
            return Err("в заголовке нет Content-Length".into());
        };
        if length > MAX_MESSAGE_BYTES {
            return Err(format!("сообщение в {length} байт"));
        }
        let mut body = vec![0; length];
        reader
            .read_exact(&mut body)
            .await
            .map_err(|_| "поток оборвался посреди сообщения".to_string())?;
        match serde_json::from_slice(&body) {
            Ok(value) => return Ok(Some(value)),
            Err(_) => tracing::debug!("LSP server sent a body that is not JSON"),
        }
    }
}

async fn write_frames(mut stdin: ChildStdin, mut frames: mpsc::UnboundedReceiver<Vec<u8>>) {
    while let Some(frame) = frames.recv().await {
        if stdin.write_all(&frame).await.is_err() || stdin.flush().await.is_err() {
            return;
        }
    }
}

async fn read_frames(mut child: Child, stdout: ChildStdout, tail: Tail, inbox: Inbox) {
    let mut stdout = BufReader::new(stdout);
    let mut broken = None;
    loop {
        match read_message(&mut stdout).await {
            Ok(Some(message)) => {
                if inbox.send(Incoming::Message(message)).is_err() {
                    return;
                }
            }
            Ok(None) => break,
            Err(why) => {
                broken = Some(why);
                break;
            }
        }
    }

    let code = tokio::time::timeout(Duration::from_secs(1), child.wait())
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(|status| status.code());
    let said = tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join("\n");

    let mut reason = match (broken, code) {
        (Some(why), _) => format!("сервер нарушил протокол: {why}"),
        (None, Some(code)) => format!("процесс сервера завершился с кодом {code}"),
        (None, None) => "процесс сервера завершился".to_string(),
    };
    if !said.is_empty() {
        reason.push_str(":\n");
        reason.push_str(&said);
    }
    let _ = inbox.send(Incoming::Closed(reason));
}

async fn keep_tail(stderr: tokio::process::ChildStderr, tail: Tail) {
    let mut stderr = BufReader::new(stderr);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match stderr.read_until(b'\n', &mut raw).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        tracing::debug!(line, "LSP server stderr");
        let mut tail = tail.lock().unwrap();
        if tail.len() == STDERR_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    async fn read_all(bytes: &[u8]) -> Vec<Result<Option<Value>, String>> {
        let mut reader = BufReader::new(bytes);
        let mut out = Vec::new();
        loop {
            let next = read_message(&mut reader).await;
            let done = !matches!(next, Ok(Some(_)));
            out.push(next);
            if done {
                return out;
            }
        }
    }

    #[tokio::test]
    async fn a_message_survives_the_trip() {
        let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "ё", "params": { "x": "日本" } });
        let got = read_all(&encode(&message)).await;
        assert_eq!(got[0], Ok(Some(message)));
        assert_eq!(got[1], Ok(None));
    }

    #[tokio::test]
    async fn the_length_counts_bytes_not_characters() {
        let frame = encode(&json!({ "t": "ёё" }));
        let text = String::from_utf8(frame).unwrap();
        let (header, body) = text.split_once("\r\n\r\n").unwrap();
        assert_eq!(header, format!("Content-Length: {}", body.len()));
    }

    #[tokio::test]
    async fn two_messages_back_to_back_are_two_messages() {
        let mut bytes = encode(&json!({ "n": 1 }));
        bytes.extend(encode(&json!({ "n": 2 })));
        let got = read_all(&bytes).await;
        assert_eq!(got[0], Ok(Some(json!({ "n": 1 }))));
        assert_eq!(got[1], Ok(Some(json!({ "n": 2 }))));
    }

    #[tokio::test]
    async fn other_headers_and_header_case_are_not_in_the_way() {
        let body = r#"{"ok":true}"#;
        let bytes = format!(
            "content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH: {}\r\n\r\n{body}",
            body.len()
        );
        let got = read_all(bytes.as_bytes()).await;
        assert_eq!(got[0], Ok(Some(json!({ "ok": true }))));
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_skipped() {
        let mut bytes = b"Content-Length: 3\r\n\r\nabc".to_vec();
        bytes.extend(encode(&json!({ "n": 2 })));
        let got = read_all(&bytes).await;
        assert_eq!(got[0], Ok(Some(json!({ "n": 2 }))));
    }

    #[tokio::test]
    async fn a_header_without_a_length_ends_the_stream_with_a_reason() {
        let got = read_all(b"X-Other: 1\r\n\r\n{}").await;
        assert!(matches!(&got[0], Err(why) if why.contains("Content-Length")));
    }

    #[tokio::test]
    async fn a_cut_body_is_an_error_not_a_message() {
        let got = read_all(b"Content-Length: 50\r\n\r\n{\"a\":1}").await;
        assert!(got[0].is_err());
    }
}

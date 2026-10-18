//! Scaffolding shared by the end-to-end tests.
//!
//! Every test here drives the core through its public surface only — `spawn`,
//! `Command`, `AgentEvent` — because that is exactly the surface the interface
//! has. If a question can be answered from here, it can be answered from the
//! feed.

// Each test binary links its own copy and uses a subset of this.
#![allow(dead_code)]

pub mod wire;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast::Receiver;
use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::provider::mock::MockProvider;
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::usage::TokenCounts;
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

/// A throwaway project directory. Own implementation rather than a crate:
/// integration tests cannot reach the unit-test helpers, and this is 20 lines.
pub struct TempProject(PathBuf);

impl TempProject {
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("yaca-it-{}", unique()));
        std::fs::create_dir_all(&dir).unwrap();
        TempProject(std::fs::canonicalize(&dir).unwrap())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        write_file(&self.0.join(rel), contents)
    }

    /// The chats folder this project's sessions are journaled into — beside
    /// the project rather than the human's own, which a test must not touch.
    pub fn chats(&self) -> PathBuf {
        let mut name = self.0.as_os_str().to_owned();
        name.push(".chats");
        PathBuf::from(name)
    }

    /// The folder the project's journals land in under [`TempProject::chats`].
    pub fn journals(&self) -> PathBuf {
        yaca_core::store::location::project_dir(&self.chats(), &self.0)
    }

    /// Writes a journal file as an earlier run would have left it.
    pub fn write_journal(&self, file: &str, contents: &str) -> PathBuf {
        write_file(&self.journals().join(file), contents)
    }

    pub fn store(&self) -> yaca_core::SessionStore {
        yaca_core::SessionStore::open(&self.chats(), &self.0)
    }
}

fn write_file(path: &Path, contents: &str) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
    path.to_path_buf()
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        let _ = std::fs::remove_dir_all(self.chats());
    }
}

/// Unique per call, not merely per instant. Tests run in parallel in one
/// process, and the Windows clock is coarse enough that two of them can read
/// the same nanosecond — which meant two tests sharing a directory and
/// deleting each other's files on drop.
pub fn unique() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{:x}-{seq:x}", std::process::id())
}

pub fn session(dir: &TempProject, provider: Arc<MockProvider>, mode: Mode) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = mode;
    cfg.journal = None;
    yaca_core::spawn(cfg, provider).unwrap()
}

/// A session over `dir`, with `extra` as its additional roots (`--add-dir`).
pub fn session_with_roots(
    dir: &TempProject,
    extra: &[&TempProject],
    provider: Arc<MockProvider>,
    mode: Mode,
) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = mode;
    cfg.journal = None;
    cfg.additional_roots = extra.iter().map(|d| d.path().to_path_buf()).collect();
    yaca_core::spawn(cfg, provider).unwrap()
}

pub async fn ask(handle: &SessionHandle, text: &str) {
    handle
        .send(Command::SendMessage {
            text: text.into(),
            attachments: vec![],
        })
        .await
        .unwrap();
}

/// Collects events until `hit` matches. Fails loudly rather than hanging:
/// a permission that is never asked looks exactly like a deadlock.
pub async fn until(
    rx: &mut Receiver<AgentEvent>,
    hit: impl Fn(&AgentEvent) -> bool,
) -> Vec<AgentEvent> {
    until_within(rx, Duration::from_secs(5), hit).await
}

/// The same with a longer rope, for tests that wait on a real process.
pub async fn until_within(
    rx: &mut Receiver<AgentEvent>,
    limit: Duration,
    hit: impl Fn(&AgentEvent) -> bool,
) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    let outcome = tokio::time::timeout(limit, async {
        while let Ok(event) = rx.recv().await {
            let done = hit(&event);
            seen.push(event);
            if done {
                return;
            }
        }
    })
    .await;
    assert!(outcome.is_ok(), "timed out; got {} events", seen.len());
    seen
}

pub fn is_turn_over(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }
    )
}

pub fn tool_call(index: u32, id: &str, name: &str, input: serde_json::Value) -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::BlockStart {
            index,
            kind: BlockKind::ToolUse {
                id: id.into(),
                name: name.into(),
            },
        },
        ProviderEvent::ToolInputDelta {
            index,
            partial_json: input.to_string(),
        },
        ProviderEvent::BlockStop { index },
    ]
}

/// One assistant message asking for several tools at once — the case the
/// `2 of 3` counter exists for.
pub fn calls(list: &[(&str, &str, serde_json::Value)]) -> Vec<ProviderEvent> {
    let mut out = vec![ProviderEvent::MessageStart {
        model: "mock".into(),
        usage: TokenCounts::default(),
    }];
    for (index, (id, name, input)) in list.iter().enumerate() {
        out.extend(tool_call(index as u32, id, name, input.clone()));
    }
    out.push(ProviderEvent::MessageDelta {
        stop_reason: Some(StopReason::ToolUse),
        usage: TokenCounts::default(),
    });
    out.push(ProviderEvent::MessageStop);
    out
}

/// The result the model read for `id` in its last request: its text, and
/// whether the call was marked as an error.
pub fn tool_result_of(
    provider: &yaca_core::provider::mock::MockProvider,
    id: &str,
) -> (String, bool) {
    use yaca_core::message::ContentBlock;

    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    sent.messages
        .last()
        .expect("запрос без сообщений")
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } if tool_use_id == id => Some((content.clone(), *is_error)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no result for {id}"))
}

/// A program built from `examples/`, by name. `cargo test` builds the examples
/// of the package next to its test binaries: `target/<profile>/examples`
/// beside `target/<profile>/deps`, where this binary is.
pub fn fixture(name: &str) -> PathBuf {
    let me = std::env::current_exe().unwrap();
    let profile = me.parent().and_then(Path::parent).expect("a test binary lives in deps/");
    let path = profile
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));

    // `cargo test --test <one>` builds that one target and no examples. Built
    // here then, once per test binary; the build directory is not locked while
    // tests run. Also when the binary is older than its source: a fixture
    // edited since the last build would otherwise answer as the old one did.
    // Not on every run, because the build here goes without the caller's
    // `--config`, and would rebuild the library under another profile.
    static BUILT: std::sync::Once = std::sync::Once::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(format!("{name}.rs"));
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let stale = match (modified(&path), modified(&source)) {
        (Some(built), Some(written)) => built < written,
        (None, _) => true,
        _ => false,
    };
    if stale {
        BUILT.call_once(|| {
            let mut cargo = std::process::Command::new(env!("CARGO"));
            cargo.args(["build", "--quiet", "-p", "yaca-core", "--example", name]);
            if profile.file_name().is_some_and(|dir| dir == "release") {
                cargo.arg("--release");
            }
            let status = cargo.status().expect("cargo did not start");
            assert!(status.success(), "the example {name} did not build");
        });
    }
    assert!(path.exists(), "{} is not built", path.display());
    path
}

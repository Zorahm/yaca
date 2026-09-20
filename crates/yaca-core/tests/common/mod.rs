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

use yaca_core::event::{AgentEvent, StopReason};
use yaca_core::provider::mock::MockProvider;
use yaca_core::provider::{BlockKind, ProviderEvent};
use yaca_core::usage::TokenCounts;
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};
use tokio::sync::broadcast::Receiver;

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
        let p = self.0.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, contents).unwrap();
        p
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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
    cfg.journal = false;
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

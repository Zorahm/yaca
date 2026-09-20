//! Standing refusals: "Always deny" (§5.7, §8).
//!
//! The pair of "Always allow". Without it a model that keeps reaching for an
//! action the human refused asks about it again and again: on a local model
//! that was nine questions in one turn about the same file, each one a block
//! in the feed.
//!
//! Driven through the public surface only, like `permissions.rs`.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::id::RequestId;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::provider::ProviderEvent;
use yaca_core::{Command, Mode, PermissionAnswer, ScopeKey, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

fn delete(id: &str, path: &str) -> Vec<ProviderEvent> {
    calls(&[(id, "Delete", serde_json::json!({ "path": path }))])
}

/// Two deletions in `src/`, one per model turn, then a closing line.
fn two_deletions() -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![
        Turn::Events(delete("t1", "src/a.rs")),
        Turn::Events(delete("t2", "src/b.rs")),
        Turn::Events(script_text("Leaving src alone.")),
    ]))
}

async fn first_question(rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>) -> RequestId {
    let events = until(rx, |e| matches!(e, AgentEvent::PermissionRequested { .. })).await;
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::PermissionRequested { request } => Some(request.id.clone()),
            _ => None,
        })
        .expect("no permission was requested")
}

async fn refuse_src(handle: &SessionHandle, id: RequestId) {
    handle
        .send(Command::AnswerPermission {
            request_id: id,
            answer: PermissionAnswer::DenyAlways {
                scope: ScopeKey::Directory { path: "src".into() },
                reason: Some("src is generated, leave it".into()),
            },
        })
        .await
        .unwrap();
}

fn refusals(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolCallDenied { stopped: false, .. }))
        .count()
}

#[tokio::test]
async fn a_standing_refusal_answers_later_calls_without_asking() {
    let dir = TempProject::new();
    let a = dir.write("src/a.rs", "one\n");
    let b = dir.write("src/b.rs", "two\n");

    let provider = two_deletions();
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete both").await;
    let id = first_question(&mut rx).await;
    refuse_src(&handle, id).await;

    let rest = until(&mut rx, is_turn_over).await;
    let asked_again = rest
        .iter()
        .filter(|e| matches!(e, AgentEvent::PermissionRequested { .. }))
        .count();
    assert_eq!(asked_again, 0, "the refusal should have covered src/b.rs");
    assert_eq!(refusals(&rest), 2, "each refused call still gets its own row (§5.5)");
    assert!(a.exists() && b.exists());

    // Not a stop: refusing one action is no reason to end a turn that may
    // still have other work in it.
    let names: Vec<_> = rest.iter().map(AgentEvent::kind).collect();
    assert!(names.contains(&"turnFinished"), "{names:?}");

    // The call nobody was asked about still tells the model why.
    let seen = provider.seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    let last = format!("{:?}", seen[2].messages.last().unwrap());
    assert!(last.contains("src is generated"), "{last}");
}

/// Bypass means "stop asking me", not "do what I said no to".
#[tokio::test]
async fn a_standing_refusal_holds_under_bypass() {
    let dir = TempProject::new();
    dir.write("src/a.rs", "one\n");
    let b = dir.write("src/b.rs", "two\n");

    let handle = session(&dir, two_deletions(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "delete both").await;
    let id = first_question(&mut rx).await;

    handle.send(Command::SetBypass { on: true }).await.unwrap();
    refuse_src(&handle, id).await;

    let rest = until(&mut rx, is_turn_over).await;
    assert_eq!(refusals(&rest), 2);
    assert!(b.exists(), "bypass waved through a call the human had refused");
}

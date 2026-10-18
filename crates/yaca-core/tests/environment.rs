//! The date and the state of the working tree reach the model in the first
//! message of a session, and nowhere else.
//!
//! What is checked is the request the model was sent: the system prompt is the
//! cached prefix and must not carry anything that moves.

mod common;

use std::process::Command;
use std::sync::Arc;

use yaca_core::message::{ContentBlock, Role};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, is_turn_over, session, until, TempProject};

fn replying(turns: usize) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(
        (0..turns).map(|_| Turn::Events(script_text("ok"))).collect(),
    ))
}

fn git(dir: &TempProject, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir.path())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn first_text(provider: &MockProvider, request: usize) -> String {
    let seen = provider.seen.lock().unwrap();
    let first = &seen[request].messages[0];
    assert_eq!(first.role, Role::User);
    first
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn the_first_message_carries_the_date_and_the_tree() {
    let dir = TempProject::new();
    if !git(&dir, &["init", "-q", "-b", "trunk"]) {
        eprintln!("git is not available; skipped");
        return;
    }
    dir.write("tracked.txt", "x");
    dir.write("new.txt", "y");

    let provider = replying(1);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let text = first_text(&provider, 0);
    assert!(text.contains("<environment_context>"), "{text}");
    assert!(text.contains("Today's date: 20"), "{text}");
    assert!(text.contains("Git branch: ") && text.contains("trunk"), "{text}");
    assert!(text.contains("?? tracked.txt"), "{text}");
    // Not an octal escape: the model would copy it into a path.
    assert!(text.contains("?? new.txt"), "{text}");
    assert!(!text.contains(".yaca"), "{text}");
    assert!(text.ends_with("hello"), "{text}");

    let seen = provider.seen.lock().unwrap();
    assert!(
        !seen[0].system.contains("environment_context"),
        "the system prompt is the cached prefix"
    );
}

#[tokio::test]
async fn the_note_is_written_once_and_stays_put() {
    let dir = TempProject::new();
    if !git(&dir, &["init", "-q"]) {
        eprintln!("git is not available; skipped");
        return;
    }

    let provider = replying(2);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "one").await;
    until(&mut rx, is_turn_over).await;
    // The tree changes between the messages; the note must not follow it,
    // or the prefix of the second request differs from the first.
    dir.write("later.txt", "z");
    ask(&handle, "two").await;
    until(&mut rx, is_turn_over).await;

    let (first, second) = (first_text(&provider, 0), first_text(&provider, 1));
    assert_eq!(first, second);
    assert!(!second.contains("later.txt"));

    let seen = provider.seen.lock().unwrap();
    let notes = seen[1]
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter(|b| matches!(b, ContentBlock::Text { text } if text.contains("<environment_context>")))
        .count();
    assert_eq!(notes, 1);
}

#[tokio::test]
async fn outside_a_repository_the_date_is_still_there() {
    let dir = TempProject::new();

    let provider = replying(1);
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();
    ask(&handle, "hello").await;
    until(&mut rx, is_turn_over).await;

    let text = first_text(&provider, 0);
    assert!(text.contains("Today's date: 20"), "{text}");
    assert!(text.ends_with("hello"), "{text}");
}

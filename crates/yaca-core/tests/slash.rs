//! Project slash commands end to end (§7.3).
//!
//! Real files under `.yaca/commands/`, and the core driven through the surface
//! the interface has. The point of the slice is a split: the feed keeps the
//! name the human typed, the model gets the prompt behind it.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, is_turn_over, session, until, TempProject};

fn replying(text: &str) -> Arc<MockProvider> {
    Arc::new(MockProvider::new(vec![Turn::Events(script_text(text))]))
}

/// The message that opened the last request — what the model was actually
/// given for the line just typed.
fn opening(provider: &MockProvider) -> String {
    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    sent.messages
        .last()
        .expect("запрос без сообщений")
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
async fn the_model_gets_the_prompt_and_the_feed_keeps_the_name() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/commands/review.md",
        "---\ndescription: Просмотреть изменения\n---\nПросмотри $ARGUMENTS и скажи, что не так.\n",
    );

    let provider = replying("смотрю");
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "/review src/main.rs").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert_eq!(
        opening(&provider),
        "Просмотри src/main.rs и скажи, что не так."
    );

    // §5.1 draws what the human typed, and §9 replays it — a feed showing the
    // expansion would show a wall of text nobody wrote.
    let card = seen
        .iter()
        .find_map(|e| match e {
            AgentEvent::UserMessage { text, .. } => Some(text.clone()),
            _ => None,
        })
        .expect("карточка человека должна быть в ленте");
    assert_eq!(card, "/review src/main.rs");
}

#[tokio::test]
async fn a_command_written_while_the_session_is_open_works_in_it() {
    let dir = TempProject::new();

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_text("раз")),
        Turn::Events(script_text("два")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "/notes").await;
    until(&mut rx, is_turn_over).await;
    // Nothing answers to the name yet, so it goes to the model as written.
    assert_eq!(opening(&provider), "/notes");

    dir.write(".yaca/commands/notes.md", "Собери заметки по проекту.");

    ask(&handle, "/notes").await;
    until(&mut rx, is_turn_over).await;
    // The directory is read per message: a file written now works now.
    assert_eq!(opening(&provider), "Собери заметки по проекту.");
}

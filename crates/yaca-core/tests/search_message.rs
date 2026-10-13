//! `SearchMessage`: the conversation's own messages, from the session's
//! journal rather than from the context window.
//!
//! The journal is what makes the tool worth having: it still holds what
//! compaction took out of the window, so the model can get the exact words
//! back instead of working from a summary of them.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, tool_result_of, until, TempProject};

fn recorded(dir: &TempProject, provider: Arc<MockProvider>) -> SessionHandle {
    let mut config = SessionConfig::new(dir.path());
    config.mode = Mode::Manual;
    config.journal = true;
    yaca_core::spawn(config, provider).unwrap()
}

async fn say(handle: &SessionHandle, text: &str) {
    let mut rx = handle.subscribe();
    ask(handle, text).await;
    until(&mut rx, is_turn_over).await;
}

fn search(id: &str, input: serde_json::Value) -> [Turn; 2] {
    [
        Turn::Events(calls(&[(id, "SearchMessage", input)])),
        Turn::Events(script_text("нашёл")),
    ]
}

#[tokio::test]
async fn a_phrase_is_found_on_both_sides_with_the_number_of_each_message() {
    let dir = TempProject::new();
    let mut script = vec![Turn::Events(script_text("Запомнил: пароль от стенда — Ананас."))];
    script.extend(search("s1", json!({ "query": "ананас" })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "Запомни слово АНАНАС, оно понадобится").await;
    say(&handle, "что я просил запомнить?").await;

    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
    let lines: Vec<&str> = result.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(lines.len(), 2, "{result}");
    // Case does not matter, and each hit says who spoke.
    assert!(lines[0].starts_with("#1 · человек · "), "{result}");
    assert!(lines[0].contains("АНАНАС"), "{result}");
    assert!(lines[1].starts_with("#2 · агент · "), "{result}");
}

#[tokio::test]
async fn a_role_narrows_the_search_to_one_side() {
    let dir = TempProject::new();
    let mut script = vec![Turn::Events(script_text("Про ананас записал."))];
    script.extend(search("s1", json!({ "query": "ананас", "role": "assistant" })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "ананас").await;
    say(&handle, "найди").await;

    let (result, _) = tool_result_of(&provider, "s1");
    let lines: Vec<&str> = result.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(lines.len(), 1, "{result}");
    assert!(lines[0].starts_with("#2 · агент"), "{result}");
}

#[tokio::test]
async fn a_number_gives_the_message_back_word_for_word() {
    let dir = TempProject::new();
    let original = "Первая строка.\n\n  Вторая, с отступом и `кодом`.";
    let mut script = vec![Turn::Events(script_text("ок"))];
    script.extend(search("s1", json!({ "number": 1 })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    say(&handle, original).await;
    say(&handle, "верни моё первое сообщение").await;

    let (result, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{result}");
    assert!(result.contains(original), "{result}");
}

#[tokio::test]
async fn a_message_compaction_took_out_of_the_window_is_still_found() {
    let dir = TempProject::new();
    let long = |prefix: &str| {
        format!(
            "{prefix}: {}",
            "разбор дат ломается на границе часового пояса. ".repeat(20)
        )
    };
    let mut script = vec![
        Turn::Events(script_text("ответ 0")),
        Turn::Events(script_text("ответ 1")),
        Turn::Events(script_text("ответ 2")),
        Turn::Events(script_text("ответ 3")),
        // The summary itself.
        Turn::Events(script_text("Чинили разбор дат.")),
    ];
    script.extend(search("s1", json!({ "query": "кодовое слово" })));
    script.extend(search("s2", json!({ "number": 1 })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    let first = long("кодовое слово — гранат");
    say(&handle, &first).await;
    say(&handle, &long("два")).await;
    say(&handle, "три").await;
    say(&handle, "четыре").await;

    let mut rx = handle.subscribe();
    handle.send(Command::Compact).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::ContextCompacted { .. })).await;

    say(&handle, "какое было кодовое слово?").await;
    // The window no longer holds the first message: that is the premise.
    // Four answers and the summary came before; this is the request the
    // search was made from, before its result put the word back.
    let sent = provider.seen.lock().unwrap()[5].clone();
    let window = serde_json::to_string(&sent.messages).unwrap();
    assert!(!window.contains("гранат"), "the summary was supposed to replace it");

    let (result, _) = tool_result_of(&provider, "s1");
    let lines: Vec<&str> = result.lines().filter(|l| l.starts_with('#')).collect();
    // The question that triggered the search matches too; the first hit is
    // the message from before the summary, under the number it always had.
    assert!(lines[0].starts_with("#1 · человек · "), "{result}");
    assert!(lines[0].contains("гранат"), "{result}");

    say(&handle, "покажи его целиком").await;
    let (result, _) = tool_result_of(&provider, "s2");
    assert!(result.contains(&first), "{result}");
}

#[tokio::test]
async fn messages_kept_through_a_compaction_keep_their_numbers() {
    let dir = TempProject::new();
    let long = |prefix: &str| format!("{prefix}: {}", "слова ".repeat(200));
    let mut script = vec![
        Turn::Events(script_text("ответ 0")),
        Turn::Events(script_text("ответ 1")),
        Turn::Events(script_text("ответ 2")),
        Turn::Events(script_text("уникальный-ответ-три")),
        Turn::Events(script_text("Сводка.")),
    ];
    script.extend(search("s1", json!({ "query": "уникальный-ответ-три" })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    say(&handle, &long("раз")).await;
    say(&handle, &long("два")).await;
    say(&handle, "три").await;
    say(&handle, "четыре").await;

    let mut rx = handle.subscribe();
    handle.send(Command::Compact).await.unwrap();
    until(&mut rx, |e| matches!(e, AgentEvent::ContextCompacted { .. })).await;
    say(&handle, "найди").await;

    // Compaction writes the turns it keeps into the journal a second time.
    // The copy is not a second message: one hit, under the original number.
    let (result, _) = tool_result_of(&provider, "s1");
    let lines: Vec<&str> = result.lines().filter(|l| l.starts_with('#')).collect();
    assert_eq!(lines.len(), 1, "{result}");
    assert!(lines[0].starts_with("#8 · агент · "), "{result}");
}

#[tokio::test]
async fn nothing_found_and_a_number_that_does_not_exist_say_so() {
    let dir = TempProject::new();
    let mut script = Vec::from(search("s1", json!({ "query": "такого здесь нет нигде" })));
    script.extend(search("s2", json!({ "number": 99 })));
    let provider = Arc::new(MockProvider::new(script));
    let handle = recorded(&dir, provider.clone());

    say(&handle, "привет").await;
    let (result, is_error) = tool_result_of(&provider, "s1");
    // The query is in the tool call, not in a message, so it finds nothing.
    assert!(!is_error, "{result}");
    assert!(!result.lines().any(|l| l.starts_with('#')), "{result}");

    say(&handle, "ещё").await;
    let (result, is_error) = tool_result_of(&provider, "s2");
    assert!(is_error, "{result}");
}

#[tokio::test]
async fn a_session_that_keeps_no_journal_does_not_offer_the_tool() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("ок"))]));
    let mut config = SessionConfig::new(dir.path());
    config.journal = false;
    let handle = yaca_core::spawn(config, provider.clone()).unwrap();

    say(&handle, "привет").await;

    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    assert!(!sent.tools.iter().any(|t| t.name == "SearchMessage"));
}

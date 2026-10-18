//! Images end to end: what the human attaches, what the agent goes and looks
//! at, and what a model that cannot see is given instead.
//!
//! Checked on what the provider was sent, because that is the only place the
//! difference shows: the feed draws a chip either way.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::error::ProviderError;
use yaca_core::event::AgentEvent;
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::usage::unknown_model;
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, session, until, TempProject};

/// A real PNG: the core tells an image by its bytes, so a fake would not do.
fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([30, 90, 160, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn picture_in(dir: &TempProject, rel: &str, width: u32, height: u32) {
    std::fs::write(dir.path().join(rel), png(width, height)).unwrap();
}

async fn send(handle: &SessionHandle, text: &str, attachments: &[&str]) {
    handle
        .send(Command::SendMessage {
            text: text.into(),
            attachments: attachments.iter().map(|a| a.to_string()).collect(),
        })
        .await
        .unwrap();
}

/// What a message is made of, in order: `text`, `image`, `result`.
fn shape(message: &Message) -> Vec<&'static str> {
    message
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { .. } => "text",
            ContentBlock::Image { .. } => "image",
            ContentBlock::ToolResult { .. } => "result",
            _ => "other",
        })
        .collect()
}

fn texts(message: &Message) -> Vec<&str> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn images(messages: &[Message]) -> usize {
    messages
        .iter()
        .flat_map(|m| &m.content)
        .filter(|b| matches!(b, ContentBlock::Image { .. }))
        .count()
}

/// The history of a request, without the environment note on the first
/// message: what these tests look at is the human's message and its images.
fn sent(provider: &MockProvider, request: usize) -> Vec<Message> {
    let mut messages = provider.seen.lock().unwrap()[request].messages.clone();
    if let Some(first) = messages.first_mut() {
        first.content.retain(|block| {
            !matches!(block, ContentBlock::Text { text } if text.starts_with("<environment_context>"))
        });
    }
    messages
}

/// A session on a model the provider described itself.
fn session_on(dir: &TempProject, provider: Arc<MockProvider>, sees: bool) -> SessionHandle {
    let mut model = unknown_model("local-model");
    model.supports_images = sees;
    let mut cfg = SessionConfig::new(dir.path());
    cfg.model = model.id.clone();
    cfg.models = vec![model];
    cfg.journal = None;
    yaca_core::spawn(cfg, provider).unwrap()
}

#[tokio::test]
async fn an_attached_image_is_shown_and_not_just_named() {
    let dir = TempProject::new();
    picture_in(&dir, "shot.png", 64, 48);
    dir.write("notes.md", "# notes\n");

    let provider = Arc::new(MockProvider::text("I see"));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    send(&handle, "what is here?", &["notes.md", "shot.png"]).await;
    until(&mut rx, is_turn_over).await;

    let message = &sent(&provider, 0)[0];
    // What the human said, then each image under the line that names it.
    assert_eq!(shape(message), vec!["text", "text", "image"]);
    let said = texts(message);
    assert!(said[0].contains("- notes.md\n"), "{}", said[0]);
    assert!(said[0].contains("- shot.png (image"), "{}", said[0]);
    assert!(said[0].ends_with("what is here?"), "{}", said[0]);
    assert_eq!(said[1], "Image: shot.png");
    assert!(matches!(
        &message.content[2],
        ContentBlock::Image { media_type, width: 64, height: 48, .. } if media_type == "image/png"
    ));
}

#[tokio::test]
async fn an_image_may_be_attached_from_outside_the_project() {
    let dir = TempProject::new();
    let elsewhere = TempProject::new();
    let shot = elsewhere.path().join("desktop.png");
    std::fs::write(&shot, png(32, 32)).unwrap();
    let text = elsewhere.write("secrets.env", "KEY=1\n");

    let provider = Arc::new(MockProvider::text("I see"));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    let paths = [shot.to_str().unwrap(), text.to_str().unwrap()];
    send(&handle, "look", &paths).await;
    until(&mut rx, is_turn_over).await;

    let message = &sent(&provider, 0)[0];
    // A screenshot lives on the desktop, and pointing at it is the permission.
    assert_eq!(images(std::slice::from_ref(message)), 1);
    // That is for images only: the project boundary (§8) holds for the rest.
    let said = texts(message)[0];
    assert!(said.contains("secrets.env (not found)"), "{said}");
    assert!(!said.contains("KEY=1"));
}

#[tokio::test]
async fn a_file_that_only_claims_to_be_an_image_is_not_sent_as_one() {
    let dir = TempProject::new();
    dir.write("fake.png", "<html>not a picture</html>");

    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    send(&handle, "here", &["fake.png"]).await;
    until(&mut rx, is_turn_over).await;

    // An image block the provider cannot decode is a 400 on this request and
    // on every one after it.
    assert_eq!(images(&sent(&provider, 0)), 0);
}

#[tokio::test]
async fn an_image_the_agent_reads_follows_the_results_of_the_batch() {
    let dir = TempProject::new();
    picture_in(&dir, "shot.png", 64, 48);
    dir.write("a.rs", "fn main() {}\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("i1", "ReadImage", json!({ "path": "shot.png" })),
            ("r1", "Read", json!({ "path": "a.rs" })),
        ])),
        Turn::Events(script_text("looked")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "look at the screenshot").await;
    let seen = until(&mut rx, is_turn_over).await;

    let results = sent(&provider, 1).pop().unwrap();
    // Results at the head of the message, as a provider wants them; what was
    // read to be looked at comes after, under its name.
    assert_eq!(shape(&results), vec!["result", "result", "text", "image"]);
    assert_eq!(texts(&results), vec!["Image: shot.png"]);
    assert!(matches!(
        &results.content[0],
        ContentBlock::ToolResult { content, is_error: false, .. } if content == "shot.png: 64×48"
    ));

    // The feed gets the line, not a megabyte of base64.
    let shown: String = seen
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallProgress { call_id, chunk } if call_id.0 == "i1" => {
                Some(chunk.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(shown, "shot.png: 64×48");
}

#[tokio::test]
async fn a_model_said_to_be_blind_gets_a_note_instead_of_the_image() {
    let dir = TempProject::new();
    picture_in(&dir, "shot.png", 64, 48);

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "i1",
            "ReadImage",
            json!({ "path": "shot.png" }),
        )),
        Turn::Events(script_text("I do not see it")),
    ]));
    let handle = session_on(&dir, provider.clone(), false);
    let mut rx = handle.subscribe();

    send(&handle, "what is here?", &["shot.png"]).await;
    until(&mut rx, is_turn_over).await;

    // Never sent, so never refused: the note stands where the image was.
    let first = sent(&provider, 0);
    assert_eq!(images(&first), 0);
    assert!(texts(&first[0]).iter().any(|t| t.contains("does not accept images")));

    // And going to look at it is refused with the reason, not answered with
    // a result that promises an image and delivers none.
    let last = sent(&provider, 1).pop().unwrap();
    assert!(matches!(
        &last.content[0],
        ContentBlock::ToolResult { content, is_error: true, .. }
            if content.contains("does not accept images")
    ));
    assert_eq!(shape(&last), vec!["result"]);
}

fn refusal(message: &str) -> Turn {
    Turn::Fail(ProviderError::Http {
        status: 400,
        message: message.into(),
        retry_after_s: None,
    })
}

#[tokio::test]
async fn a_model_that_refuses_an_image_is_asked_again_without_it() {
    let dir = TempProject::new();
    picture_in(&dir, "shot.png", 64, 48);

    let provider = Arc::new(MockProvider::new(vec![
        refusal("image input is not supported by this model"),
        Turn::Events(script_text("I do not see the picture")),
        Turn::Events(script_text("still the same")),
    ]));
    // Nothing was said about vision, which is the usual case for a local
    // server: the model is taken to see until it shows otherwise.
    let handle = session_on(&dir, provider.clone(), true);
    let mut rx = handle.subscribe();

    send(&handle, "what is here?", &["shot.png"]).await;
    let seen = until(&mut rx, is_turn_over).await;

    // The conversation goes on: an image in the history would otherwise be
    // refused on every request from here.
    assert!(!seen.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    assert_eq!(images(&sent(&provider, 0)), 1);
    assert_eq!(images(&sent(&provider, 1)), 0);

    // Once is enough to know: the next message does not try the image again.
    ask(&handle, "and like this?").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(provider.seen.lock().unwrap().len(), 3);
    assert_eq!(images(&sent(&provider, 2)), 0);
}

#[tokio::test]
async fn a_request_refused_either_way_fails_with_the_first_answer() {
    let dir = TempProject::new();
    picture_in(&dir, "shot.png", 64, 48);

    let provider = Arc::new(MockProvider::new(vec![
        refusal("messages: roles must alternate"),
        refusal("the same, without images"),
        Turn::Events(script_text("now I see it")),
    ]));
    let handle = session_on(&dir, provider.clone(), true);
    let mut rx = handle.subscribe();

    send(&handle, "what is here?", &["shot.png"]).await;
    let seen = until(&mut rx, is_turn_over).await;

    // The images were not why, so the error is the one about the request as
    // it was meant to be sent.
    assert!(seen.iter().any(|e| matches!(
        e,
        AgentEvent::Error { message, .. } if message.contains("roles must alternate")
    )));

    // And the model is not written off as blind for it.
    ask(&handle, "once more").await;
    until(&mut rx, is_turn_over).await;
    assert_eq!(images(&sent(&provider, 2)), 1);
}

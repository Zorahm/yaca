//! Skills (`tool::skill`), end to end: the list reaches the system prompt the
//! model is sent, and the `Skill` tool hands a body over by name or fails with
//! the names that exist.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;

use common::{ask, calls, is_turn_over, session, tool_result_of, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(30);

fn release_skill(dir: &TempProject) {
    dir.write(
        ".yaca/skills/release/SKILL.md",
        "---\nname: release\ndescription: Выпустить новую версию\n---\n\nСначала тег, затем сборка.\n",
    );
}

#[tokio::test]
async fn the_skills_list_is_in_the_prompt_the_model_is_sent() {
    let dir = TempProject::new();
    release_skill(&dir);
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "привет").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    let sent = provider.seen.lock().unwrap();
    let system = &sent[0].system;
    assert!(system.contains("<project_skills>"), "{system}");
    assert!(
        system.contains("- release — Выпустить новую версию"),
        "{system}"
    );
}

#[tokio::test]
async fn a_project_without_skills_sends_no_skills_section() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "привет").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    assert!(!provider.seen.lock().unwrap()[0]
        .system
        .contains("project_skills"));
}

#[tokio::test]
async fn the_model_pulls_a_skill_by_name_and_gets_its_body() {
    let dir = TempProject::new();
    release_skill(&dir);
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "release" }))])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "выпусти версию").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert_eq!(text, "Сначала тег, затем сборка.");
}

#[tokio::test]
async fn an_unknown_skill_fails_and_the_model_is_told_the_names_that_exist() {
    let dir = TempProject::new();
    release_skill(&dir);
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "deploy" }))])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "задеплой").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(is_error);
    assert!(text.contains("release"), "{text}");
}

#[tokio::test]
async fn a_name_that_climbs_out_of_the_skills_directory_is_refused() {
    let dir = TempProject::new();
    dir.write(".yaca/secret.md", "тайна");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "../secret" }))])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "прочитай").await;
    until_within(&mut rx, LIMIT, is_turn_over).await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(is_error);
    assert!(!text.contains("тайна"), "{text}");
}

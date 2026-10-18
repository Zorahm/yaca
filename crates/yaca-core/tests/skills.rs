//! Skills (`skills`, `tool::skill`), end to end: the catalog reaches the system
//! prompt the model is sent, the `Skill` tool hands a skill over by name, a
//! human's `/name` brings one in, and compaction keeps an activated skill.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use yaca_core::event::AgentEvent;
use yaca_core::message::{ContentBlock, Message};
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::session::compact::{KEPT_SKILLS_HEADER, SUMMARY_HEADER};
use yaca_core::{Command, Mode, SessionConfig, SessionHandle};

use common::{ask, calls, is_turn_over, session, tool_result_of, until_within, TempProject};

const LIMIT: Duration = Duration::from_secs(30);

fn release_skill(dir: &TempProject, root: &str) {
    dir.write(
        &format!("{root}/release/SKILL.md"),
        "---\nname: release\ndescription: Release a new version\n---\n\nFirst the tag, then the build for $ARGUMENTS.\n",
    );
}

/// A session that also sees `user` as the human's own skills directory.
fn session_with_user_skills(
    dir: &TempProject,
    user: &TempProject,
    provider: Arc<MockProvider>,
) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Auto;
    cfg.journal = None;
    cfg.user_skills = vec![user.path().to_path_buf()];
    yaca_core::spawn(cfg, provider).unwrap()
}

async fn one_turn(handle: &SessionHandle, text: &str) {
    let mut rx = handle.subscribe();
    ask(handle, text).await;
    until_within(&mut rx, LIMIT, is_turn_over).await;
}

fn texts(message: &Message) -> Vec<&str> {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// What the human's message said, after the environment block the core puts
/// in front of the first one.
fn last_text(sent: &[yaca_core::provider::ProviderRequest]) -> String {
    texts(sent[0].messages.last().unwrap()).last().unwrap().to_string()
}

#[tokio::test]
async fn the_catalog_is_in_the_prompt_the_model_is_sent() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "hello").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("<available_skills>"), "{system}");
    assert!(system.contains("- release — Release a new version"), "{system}");
}

#[tokio::test]
async fn a_project_without_skills_sends_no_skills_section() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "hello").await;

    assert!(!provider.seen.lock().unwrap()[0].system.contains("available_skills"));
}

#[tokio::test]
async fn skills_in_the_shared_directories_are_found_too() {
    let dir = TempProject::new();
    dir.write(".agents/skills/audit/SKILL.md", "---\nname: audit\ndescription: Audit\n---\ntext");
    dir.write(".claude/skills/docs/SKILL.md", "---\nname: docs\ndescription: Documentation\n---\ntext");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "hello").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("- audit — Audit"), "{system}");
    assert!(system.contains("- docs — Documentation"), "{system}");
}

#[tokio::test]
async fn the_humans_own_skills_are_listed_and_the_projects_shadow_them() {
    let dir = TempProject::new();
    let user = TempProject::new();
    dir.write(".yaca/skills/release/SKILL.md", "---\nname: release\ndescription: Project release\n---\nproject");
    user.write("release/SKILL.md", "---\nname: release\ndescription: Own release\n---\nown");
    user.write("notes/SKILL.md", "---\nname: notes\ndescription: Notes\n---\ntext");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session_with_user_skills(&dir, &user, provider.clone());

    one_turn(&handle, "hello").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("- release — Project release"), "{system}");
    assert!(!system.contains("Own release"), "{system}");
    assert!(system.contains("- notes — Notes"), "{system}");
}

#[tokio::test]
async fn the_model_pulls_a_skill_by_name_and_gets_it_wrapped() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "release" }))])),
        Turn::Events(script_text("got it")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Plan);

    one_turn(&handle, "ship a version").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert!(text.starts_with("<skill_content name=\"release\">\nFirst the tag"), "{text}");
    assert!(text.contains("Skill directory:"), "{text}");
}

#[tokio::test]
async fn a_skill_outside_the_project_reads_its_own_files() {
    let dir = TempProject::new();
    let user = TempProject::new();
    user.write("release/SKILL.md", "---\nname: release\ndescription: d\n---\nsee references/steps.md");
    user.write("release/references/steps.md", "1. tag");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "s1",
            "Skill",
            json!({ "name": "release", "file": "references/steps.md" }),
        )])),
        Turn::Events(script_text("got it")),
    ]));
    let handle = session_with_user_skills(&dir, &user, provider.clone());

    one_turn(&handle, "ship").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert_eq!(text, "1. tag");
}

#[tokio::test]
async fn an_unknown_skill_fails_and_the_model_is_told_the_names_that_exist() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    dir.write(".yaca/secret.md", "confidential");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("s1", "Skill", json!({ "name": "deploy" })),
            ("s2", "Skill", json!({ "name": "../secret" })),
        ])),
        Turn::Events(script_text("got it")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "deploy it").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(is_error);
    assert!(text.contains("release"), "{text}");
    let (text, is_error) = tool_result_of(&provider, "s2");
    assert!(is_error);
    assert!(!text.contains("confidential"), "{text}");
}

#[tokio::test]
async fn a_typed_slash_name_hands_the_skill_to_the_model_with_its_arguments() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/release v2.0").await;

    let sent = provider.seen.lock().unwrap();
    let message = last_text(&sent);
    assert!(message.starts_with("<skill_content name=\"release\">"), "{message}");
    assert!(message.contains("the build for v2.0."), "{message}");
}

#[tokio::test]
async fn a_skill_kept_from_the_model_is_not_listed_but_answers_to_its_slash_name() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/skills/deploy/SKILL.md",
        "---\nname: deploy\ndescription: Roll out\ndisable-model-invocation: true\n---\nrolling out",
    );
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/deploy").await;

    let sent = provider.seen.lock().unwrap();
    assert!(!sent[0].system.contains("deploy"), "{}", sent[0].system);
    assert!(last_text(&sent).contains("rolling out"));
}

#[tokio::test]
async fn a_project_command_shadows_a_skill_of_the_same_name() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    dir.write(".yaca/commands/release.md", "the release command");
    let provider = Arc::new(MockProvider::text("ok"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/release").await;

    let sent = provider.seen.lock().unwrap();
    assert_eq!(last_text(&sent), "the release command");
}

#[tokio::test]
async fn compaction_brings_an_activated_skill_back_after_the_summary() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let long = |s: &str| format!("{s}: {}", "details ".repeat(200));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "release" }))])),
        Turn::Events(script_text(&long("did"))),
        Turn::Events(script_text(&long("next"))),
        Turn::Events(script_text("three")),
        Turn::Events(script_text("four")),
        Turn::Events(script_text("We shipped a version.")),
        Turn::Events(script_text("carrying on")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    for text in ["ship", "next", "three", "four"] {
        one_turn(&handle, text).await;
    }

    let mut rx = handle.subscribe();
    handle.send(Command::Compact).await.unwrap();
    until_within(&mut rx, LIMIT, |e| matches!(e, AgentEvent::ContextCompacted { .. })).await;
    one_turn(&handle, "continue").await;

    let sent = provider.seen.lock().unwrap();
    let head = texts(&sent.last().unwrap().messages[0]);
    assert!(head[0].starts_with(SUMMARY_HEADER), "{head:?}");
    assert_eq!(head[1], KEPT_SKILLS_HEADER);
    assert!(head[2].starts_with("<skill_content name=\"release\">"), "{head:?}");
}

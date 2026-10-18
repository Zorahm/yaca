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
        "---\nname: release\ndescription: Выпустить новую версию\n---\n\nСначала тег, затем сборка для $ARGUMENTS.\n",
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
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "привет").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("<available_skills>"), "{system}");
    assert!(system.contains("- release — Выпустить новую версию"), "{system}");
}

#[tokio::test]
async fn a_project_without_skills_sends_no_skills_section() {
    let dir = TempProject::new();
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "привет").await;

    assert!(!provider.seen.lock().unwrap()[0].system.contains("available_skills"));
}

#[tokio::test]
async fn skills_in_the_shared_directories_are_found_too() {
    let dir = TempProject::new();
    dir.write(".agents/skills/audit/SKILL.md", "---\nname: audit\ndescription: Аудит\n---\nтекст");
    dir.write(".claude/skills/docs/SKILL.md", "---\nname: docs\ndescription: Документация\n---\nтекст");
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "привет").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("- audit — Аудит"), "{system}");
    assert!(system.contains("- docs — Документация"), "{system}");
}

#[tokio::test]
async fn the_humans_own_skills_are_listed_and_the_projects_shadow_them() {
    let dir = TempProject::new();
    let user = TempProject::new();
    dir.write(".yaca/skills/release/SKILL.md", "---\nname: release\ndescription: Проектный выпуск\n---\nпроект");
    user.write("release/SKILL.md", "---\nname: release\ndescription: Свой выпуск\n---\nсвой");
    user.write("notes/SKILL.md", "---\nname: notes\ndescription: Заметки\n---\nтекст");
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session_with_user_skills(&dir, &user, provider.clone());

    one_turn(&handle, "привет").await;

    let system = &provider.seen.lock().unwrap()[0].system;
    assert!(system.contains("- release — Проектный выпуск"), "{system}");
    assert!(!system.contains("Свой выпуск"), "{system}");
    assert!(system.contains("- notes — Заметки"), "{system}");
}

#[tokio::test]
async fn the_model_pulls_a_skill_by_name_and_gets_it_wrapped() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "release" }))])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Plan);

    one_turn(&handle, "выпусти версию").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert!(text.starts_with("<skill_content name=\"release\">\nСначала тег"), "{text}");
    assert!(text.contains("Skill directory:"), "{text}");
}

#[tokio::test]
async fn a_skill_outside_the_project_reads_its_own_files() {
    let dir = TempProject::new();
    let user = TempProject::new();
    user.write("release/SKILL.md", "---\nname: release\ndescription: d\n---\nсм. references/steps.md");
    user.write("release/references/steps.md", "1. тег");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[(
            "s1",
            "Skill",
            json!({ "name": "release", "file": "references/steps.md" }),
        )])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session_with_user_skills(&dir, &user, provider.clone());

    one_turn(&handle, "выпусти").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(!is_error, "{text}");
    assert_eq!(text, "1. тег");
}

#[tokio::test]
async fn an_unknown_skill_fails_and_the_model_is_told_the_names_that_exist() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    dir.write(".yaca/secret.md", "тайна");
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("s1", "Skill", json!({ "name": "deploy" })),
            ("s2", "Skill", json!({ "name": "../secret" })),
        ])),
        Turn::Events(script_text("понял")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "задеплой").await;

    let (text, is_error) = tool_result_of(&provider, "s1");
    assert!(is_error);
    assert!(text.contains("release"), "{text}");
    let (text, is_error) = tool_result_of(&provider, "s2");
    assert!(is_error);
    assert!(!text.contains("тайна"), "{text}");
}

#[tokio::test]
async fn a_typed_slash_name_hands_the_skill_to_the_model_with_its_arguments() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/release v2.0").await;

    let sent = provider.seen.lock().unwrap();
    let message = last_text(&sent);
    assert!(message.starts_with("<skill_content name=\"release\">"), "{message}");
    assert!(message.contains("сборка для v2.0."), "{message}");
}

#[tokio::test]
async fn a_skill_kept_from_the_model_is_not_listed_but_answers_to_its_slash_name() {
    let dir = TempProject::new();
    dir.write(
        ".yaca/skills/deploy/SKILL.md",
        "---\nname: deploy\ndescription: Выкатить\ndisable-model-invocation: true\n---\nвыкатываем",
    );
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/deploy").await;

    let sent = provider.seen.lock().unwrap();
    assert!(!sent[0].system.contains("deploy"), "{}", sent[0].system);
    assert!(last_text(&sent).contains("выкатываем"));
}

#[tokio::test]
async fn a_project_command_shadows_a_skill_of_the_same_name() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    dir.write(".yaca/commands/release.md", "команда выпуска");
    let provider = Arc::new(MockProvider::text("ок"));
    let handle = session(&dir, provider.clone(), Mode::Auto);

    one_turn(&handle, "/release").await;

    let sent = provider.seen.lock().unwrap();
    assert_eq!(last_text(&sent), "команда выпуска");
}

#[tokio::test]
async fn compaction_brings_an_activated_skill_back_after_the_summary() {
    let dir = TempProject::new();
    release_skill(&dir, ".yaca/skills");
    let long = |s: &str| format!("{s}: {}", "подробности ".repeat(200));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("s1", "Skill", json!({ "name": "release" }))])),
        Turn::Events(script_text(&long("сделал"))),
        Turn::Events(script_text(&long("дальше"))),
        Turn::Events(script_text("три")),
        Turn::Events(script_text("четыре")),
        Turn::Events(script_text("Выпускали версию.")),
        Turn::Events(script_text("продолжаю")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    for text in ["выпусти", "дальше", "три", "четыре"] {
        one_turn(&handle, text).await;
    }

    let mut rx = handle.subscribe();
    handle.send(Command::Compact).await.unwrap();
    until_within(&mut rx, LIMIT, |e| matches!(e, AgentEvent::ContextCompacted { .. })).await;
    one_turn(&handle, "продолжаем").await;

    let sent = provider.seen.lock().unwrap();
    let head = texts(&sent.last().unwrap().messages[0]);
    assert!(head[0].starts_with(SUMMARY_HEADER), "{head:?}");
    assert_eq!(head[1], KEPT_SKILLS_HEADER);
    assert!(head[2].starts_with("<skill_content name=\"release\">"), "{head:?}");
}

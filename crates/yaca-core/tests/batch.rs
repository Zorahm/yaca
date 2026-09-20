//! Пачка вызовов из одного ответа модели (§5.4), от начала до результатов.
//!
//! Модель зовёт тулы пачкой затем, чтобы они шли разом, и это проверяется по
//! потоку событий: читающие вызовы обязаны начаться все, прежде чем закончится
//! первый. Пишущие — наоборот, строго по одному: одновременная правка одного
//! файла обходит инвариант свежести §8, где обе проверки проходят, а
//! переживает одна.

mod common;

use std::sync::Arc;

use yaca_core::event::AgentEvent;
use yaca_core::message::ContentBlock;
use yaca_core::provider::mock::{script_text, MockProvider, Turn};
use yaca_core::Mode;
use serde_json::json;

use common::{ask, calls, is_turn_over, session, until, TempProject};

/// Порядок событий хода: что началось и что закончилось, по идентификаторам.
///
/// Ход одного сообщения бывает из нескольких ответов модели, и `only` отбирает
/// ту пачку, про которую тест: чтения-подготовки к делу не относятся.
fn timeline(seen: &[AgentEvent], only: &[&str]) -> Vec<String> {
    seen.iter()
        .filter_map(|event| match event {
            AgentEvent::ToolCallStarted { call_id, .. } => Some(("начал", call_id)),
            AgentEvent::ToolCallFinished { call_id, .. } => Some(("кончил", call_id)),
            _ => None,
        })
        .filter(|(_, call_id)| only.is_empty() || only.contains(&call_id.0.as_str()))
        .map(|(verb, call_id)| format!("{verb} {call_id}"))
        .collect()
}

/// Идентификаторы результатов в том порядке, в каком они уехали модели.
fn results(provider: &MockProvider) -> Vec<String> {
    let sent = provider.seen.lock().unwrap().last().unwrap().clone();
    sent.messages
        .last()
        .expect("запрос без сообщений")
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolResult { tool_use_id, .. } => Some(tool_use_id.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn читающие_вызовы_идут_одновременно() {
    let dir = TempProject::new();
    dir.write("a.rs", "раз\n");
    dir.write("b.rs", "два\n");
    dir.write("c.rs", "три\n");

    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[
            ("t1", "Read", json!({ "path": "a.rs" })),
            ("t2", "Read", json!({ "path": "b.rs" })),
            ("t3", "Read", json!({ "path": "c.rs" })),
        ])),
        Turn::Events(script_text("прочитал")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Manual);
    let mut rx = handle.subscribe();

    ask(&handle, "прочитай три файла").await;
    let seen = until(&mut rx, is_turn_over).await;

    let line = timeline(&seen, &[]);
    let first_finish = line
        .iter()
        .position(|s| s.starts_with("кончил"))
        .expect("хоть один должен был закончиться");
    assert_eq!(
        first_finish, 3,
        "все три начались раньше, чем кончился первый: {line:?}"
    );

    // Порядок результатов — тот, в каком модель написала вызовы, а не тот,
    // в каком диск успел ответить.
    assert_eq!(results(&provider), vec!["t1", "t2", "t3"]);
}

#[tokio::test]
async fn пишущие_вызовы_идут_по_одному() {
    let dir = TempProject::new();
    dir.write("a.rs", "раз\n");
    dir.write("b.rs", "два\n");

    let provider = Arc::new(MockProvider::new(vec![
        // Сперва прочитать: без этого §8 не даст править.
        Turn::Events(calls(&[
            ("r1", "Read", json!({ "path": "a.rs" })),
            ("r2", "Read", json!({ "path": "b.rs" })),
        ])),
        Turn::Events(calls(&[
            (
                "w1",
                "Edit",
                json!({ "path": "a.rs", "old_string": "раз", "new_string": "one" }),
            ),
            (
                "w2",
                "Edit",
                json!({ "path": "b.rs", "old_string": "два", "new_string": "two" }),
            ),
        ])),
        Turn::Events(script_text("поправил")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "почини оба").await;
    let seen = until(&mut rx, is_turn_over).await;

    assert_eq!(
        timeline(&seen, &["w1", "w2"]),
        vec!["начал w1", "кончил w1", "начал w2", "кончил w2"],
        "правки не перемешиваются даже в разных файлах"
    );
    assert_eq!(results(&provider), vec!["w1", "w2"]);
}

#[tokio::test]
async fn читающие_не_обгоняют_пишущего_соседа() {
    let dir = TempProject::new();
    dir.write("a.rs", "раз\n");
    dir.write("b.rs", "два\n");

    // Порядок в ответе — это порядок хода, который модель написала. Пустить
    // чтения вперёд записи значило бы прочитать файл до того, как его
    // поправили, и отдать модели вчерашний текст.
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(calls(&[("r0", "Read", json!({ "path": "a.rs" }))])),
        Turn::Events(calls(&[
            (
                "w1",
                "Edit",
                json!({ "path": "a.rs", "old_string": "раз", "new_string": "one" }),
            ),
            ("r2", "Read", json!({ "path": "a.rs" })),
            ("r3", "Read", json!({ "path": "b.rs" })),
        ])),
        Turn::Events(script_text("готово")),
    ]));
    let handle = session(&dir, provider.clone(), Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "поправь и перечитай").await;
    let seen = until(&mut rx, is_turn_over).await;

    let line = timeline(&seen, &["w1", "r2", "r3"]);
    assert_eq!(
        &line[..2],
        &["начал w1", "кончил w1"],
        "правка успевает целиком до чтений: {line:?}"
    );
    // А два чтения после неё — снова разом.
    assert_eq!(&line[2..4], &["начал r2", "начал r3"], "{line:?}");
}

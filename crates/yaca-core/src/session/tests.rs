//! Золотые тесты цикла.
//!
//! Проверяют ровно одно и главное: какая последовательность `AgentEvent`
//! выходит наружу. Интерфейс — чистая функция от этого потока, поэтому если
//! поток сохраняет форму, лента не сломается; если форма поехала — тест
//! обязан упасть раньше, чем это увидит человек.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::broadcast::Receiver;

use super::*;
use crate::error::ProviderError;
use crate::event::StopReason;
use crate::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use crate::testutil::TempDir;

/// Имя события — то, по чему интерфейс различает ветки.
fn names(events: &[AgentEvent]) -> Vec<&'static str> {
    events.iter().map(AgentEvent::kind).collect()
}

async fn collect(rx: &mut Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    let deadline = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let done = matches!(
                        ev,
                        AgentEvent::TurnFinished { .. } | AgentEvent::TurnCancelled { .. }
                    );
                    out.push(ev);
                    if done {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    })
    .await;
    assert!(
        deadline.is_ok(),
        "турн не завершился за 5 с: {:?}",
        names(&out)
    );
    out
}

fn session(dir: &TempDir, provider: MockProvider, mode: Mode) -> SessionHandle {
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = mode;
    cfg.journal = false;
    cfg.model = "claude-opus-5".into();
    spawn(cfg, Arc::new(provider)).unwrap()
}

async fn ask(handle: &SessionHandle, text: &str) {
    handle
        .send(Command::SendMessage {
            text: text.into(),
            attachments: vec![],
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn текстовый_ответ_даёт_ровно_ожидаемый_поток() {
    let dir = TempDir::new();
    let handle = session(&dir, MockProvider::text("Это на Rust."), Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "на чём написано ядро?").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged", // кольцо закрутилось
            // §5.8: точка снимается до того, как турн что-либо тронет, —
            // отсюда возвращает `Esc Esc` (§13).
            "checkpointCreated",
            "checkpointsUpdated",
            // §5.1: карточку сообщения человека рисует поток, а не интерфейс.
            "userMessage",
            "turnStarted",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );

    match &events[5] {
        AgentEvent::TextDelta { text, .. } => assert_eq!(text, "Это на Rust."),
        other => panic!("не дельта: {other:?}"),
    }
    match &events[7] {
        AgentEvent::TurnFinished { stop_reason, .. } => {
            assert_eq!(*stop_reason, StopReason::EndTurn)
        }
        other => panic!("не завершение: {other:?}"),
    }
}

#[tokio::test]
async fn вызов_тула_проходит_весь_путь_и_подпись_не_уезжает_модели() {
    let dir = TempDir::new();
    dir.write("src/main.rs", "fn main() {}\n");

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "src/main.rs", "description": "Прочитал точку входа." }),
        )),
        Turn::Events(script_text("Точка входа пустая.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "что в main?").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "checkpointCreated",
            "checkpointsUpdated",
            // §5.1: карточку сообщения человека рисует поток, а не интерфейс.
            "userMessage",
            "turnStarted",
            "usageUpdated",
            "toolCallStarted",
            "toolCallProgress",
            "toolCallFinished",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );

    match &events[6] {
        AgentEvent::ToolCallStarted {
            tool,
            description,
            target,
            input,
            ..
        } => {
            assert_eq!(tool, "Read");
            // Точка без точки на конце — форма §5.4.1.
            assert_eq!(description.as_deref(), Some("Прочитал точку входа"));
            assert_eq!(target.as_deref(), Some("src/main.rs"));
            assert!(
                input.get("description").is_none(),
                "подпись — забота ленты, тулу она не аргумент"
            );
        }
        other => panic!("не старт тула: {other:?}"),
    }

    match &events[7] {
        AgentEvent::ToolCallProgress { chunk, .. } => assert!(chunk.contains("fn main")),
        other => panic!("не вывод: {other:?}"),
    }
}

/// Учёт приходит после каждого ответа модели, а не раз в турн: кольцо
/// состояния должно заполняться и посреди хода с тулами.
#[tokio::test]
async fn расход_накапливается_на_каждом_ответе_модели() {
    let dir = TempDir::new();
    dir.write("src/main.rs", "fn main() {}\n");

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "src/main.rs", "description": "Прочитал точку входа" }),
        )),
        Turn::Events(script_text("Пусто.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "что в main?").await;
    let events = collect(&mut rx).await;

    let расходы: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::UsageUpdated { usage } => Some(usage.total.output),
            _ => None,
        })
        .collect();
    assert_eq!(
        расходы,
        vec![30, 50],
        "счётчик за сессию должен накапливаться"
    );
}

#[tokio::test]
async fn выход_за_границы_проекта_это_отклонение_а_не_сбой() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call(
            "toolu_1",
            "Read",
            json!({ "path": "../../secrets.env", "description": "Заглянул наружу" }),
        )),
        Turn::Events(script_text("Не вышло.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "прочитай соседний проект").await;
    let events = collect(&mut rx).await;

    // §5.9: сработавшая защита красится `--vermilion-ink`, не `--rust`,
    // и это отдельное событие, а не упавший тул.
    assert!(names(&events).contains(&"toolCallRejected"));
    assert!(!names(&events).contains(&"toolCallFailed"));
}

#[tokio::test]
async fn неизвестный_тул_не_роняет_турн() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Events(script_tool_call("t1", "Write", json!({ "path": "a" }))),
        Turn::Events(script_text("Понял, такого нет.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "перепиши файл").await;
    let events = collect(&mut rx).await;

    assert!(names(&events).contains(&"toolCallFailed"));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::EndTurn,
            ..
        })
    ));
}

#[tokio::test]
async fn ошибка_провайдера_гасит_кольцо_а_не_вешает_его() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![Turn::Fail(ProviderError::Http {
        status: 401,
        message: "invalid x-api-key".into(),
        retry_after_s: None,
    })]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "привет").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "checkpointCreated",
            "checkpointsUpdated",
            // §5.1: карточку сообщения человека рисует поток, а не интерфейс.
            "userMessage",
            "turnStarted",
            "error",
            "turnFinished",
        ]
    );
    match &events[5] {
        AgentEvent::Error {
            code, retryable, ..
        } => {
            assert_eq!(code, "http_401");
            assert!(!retryable, "401 повторять бессмысленно");
        }
        other => panic!("не ошибка: {other:?}"),
    }
    // Без `TurnFinished` кольцо крутилось бы вечно — интерфейс врал бы
    // про состояние, чего §0 не допускает.
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnFinished {
            stop_reason: StopReason::Failed,
            ..
        })
    ));
}

#[tokio::test]
async fn лимит_запросов_повторяется_и_показывает_счёт_попыток() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![
        Turn::Fail(ProviderError::Http {
            status: 429,
            message: "rate limited".into(),
            // Провайдер сказал, когда возвращаться, — слушаемся его,
            // а тест на этом не спит.
            retry_after_s: Some(0),
        }),
        Turn::Events(script_text("Дошло со второй попытки.")),
    ]);
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "привет").await;
    let events = collect(&mut rx).await;

    assert_eq!(
        names(&events),
        vec![
            "stateChanged",
            "checkpointCreated",
            "checkpointsUpdated",
            // §5.1: карточку сообщения человека рисует поток, а не интерфейс.
            "userMessage",
            "turnStarted",
            "retryScheduled",
            "textDelta",
            "usageUpdated",
            "turnFinished",
        ]
    );
    match &events[5] {
        AgentEvent::RetryScheduled { attempt, of, .. } => {
            // §5.10: «Повтор через 4 с · попытка 2 из 5».
            assert_eq!((*attempt, *of), (2, 5));
        }
        other => panic!("не повтор: {other:?}"),
    }
}

#[tokio::test]
async fn отмена_обрывает_турн_и_говорит_об_этом() {
    let dir = TempDir::new();

    let provider = MockProvider::new(vec![Turn::Events(script_text("длинный ответ"))])
        .with_gap(Duration::from_millis(80));
    let handle = session(&dir, provider, Mode::Plan);
    let mut rx = handle.subscribe();

    ask(&handle, "расскажи всё").await;
    tokio::time::sleep(Duration::from_millis(120)).await;
    handle.send(Command::Cancel).await.unwrap();

    let events = collect(&mut rx).await;
    assert!(
        matches!(events.last(), Some(AgentEvent::TurnCancelled { .. })),
        "получили {:?}",
        names(&events)
    );
    assert!(!names(&events).contains(&"turnFinished"));
}

#[tokio::test]
async fn режим_plan_не_отдаёт_модели_пишущих_тулов() {
    let dir = TempDir::new();
    let provider = Arc::new(MockProvider::text("ок"));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Plan;
    cfg.journal = false;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "привет").await;
    collect(&mut rx).await;

    let seen = provider.seen.lock().unwrap();
    let tools: Vec<_> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        tools,
        vec![
            "ExitPlanMode",
            "Glob",
            "Grep",
            "Read",
            "TodoWrite",
            "WebFetch"
        ]
    );
    assert!(seen[0].system.contains("Mode: plan"));
}

#[tokio::test]
async fn смена_режима_видна_и_доезжает_до_следующего_запроса() {
    let dir = TempDir::new();
    let provider = Arc::new(MockProvider::new(vec![Turn::Events(script_text("ок"))]));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.mode = Mode::Manual;
    cfg.journal = false;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    handle
        .send(Command::SetMode { mode: Mode::Plan })
        .await
        .unwrap();

    match tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
    {
        AgentEvent::StateChanged { state } => assert_eq!(state.mode, Mode::Plan),
        other => panic!("не смена состояния: {other:?}"),
    }
    assert_eq!(handle.state().mode, Mode::Plan);

    ask(&handle, "составь план").await;
    collect(&mut rx).await;

    let seen = provider.seen.lock().unwrap();
    let tools: Vec<_> = seen[0].tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tools.contains(&"TodoWrite"));
    assert!(tools.contains(&"ExitPlanMode"));
    assert!(!tools.contains(&"Edit"));
}

#[tokio::test]
async fn вложенный_файл_считается_прочитанным() {
    let dir = TempDir::new();
    dir.write("notes.md", "заметки");

    let provider = Arc::new(MockProvider::text("прочитал"));
    let mut cfg = SessionConfig::new(dir.path());
    cfg.journal = false;
    let handle = spawn(cfg, provider.clone()).unwrap();
    let mut rx = handle.subscribe();

    handle
        .send(Command::SendMessage {
            text: "что тут?".into(),
            attachments: vec!["notes.md".into()],
        })
        .await
        .unwrap();
    collect(&mut rx).await;

    // §7.2: человек приложил файл сам, заставлять агента перечитывать — глупость.
    let seen = provider.seen.lock().unwrap();
    let first = format!("{:?}", seen[0].messages[0]);
    assert!(first.contains("notes.md"), "вложение не попало в сообщение");
}

#[tokio::test]
async fn журнал_переигрывается_в_тот_же_поток() {
    let dir = TempDir::new();
    let mut cfg = SessionConfig::new(dir.path());
    cfg.journal = true;
    let handle = spawn(cfg, Arc::new(MockProvider::text("привет"))).unwrap();
    let mut rx = handle.subscribe();

    ask(&handle, "здравствуй").await;
    let live = collect(&mut rx).await;

    let path = dir
        .path()
        .join(".yaca/sessions")
        .join(format!("{}.jsonl", handle.id()));
    let replayed = log::replay(&path).unwrap();

    // Журнал пишет ровно то, что видел подписчик: сессия восстанавливается
    // проигрыванием файла, и любое расхождение здесь — расхождение ленты.
    // Хвостовой `stateChanged` приходит уже после `turnFinished`, на котором
    // остановился сбор живого потока.
    assert_eq!(
        names(&replayed),
        [names(&live), vec!["stateChanged"]].concat()
    );
}

// --- правки в рабочем дереве (§6) -------------------------------------------

fn правит(path: &str, old: &str, new: &str) -> Vec<crate::provider::ProviderEvent> {
    script_tool_call(
        "toolu_edit",
        "Edit",
        json!({ "path": path, "old_string": old, "new_string": new }),
    )
}

fn читает(id: &str, path: &str) -> Vec<crate::provider::ProviderEvent> {
    script_tool_call(
        id,
        "Read",
        json!({ "path": path, "description": "Посмотрел файл" }),
    )
}

#[tokio::test]
async fn правка_уезжает_в_дерево_и_даёт_ленте_дифф() {
    let dir = TempDir::new();
    let path = dir.write("src/a.rs", "let x = 1;\n");

    let provider = MockProvider::new(vec![
        // Прочитать перед правкой обязан и мок: инвариант не отключается.
        Turn::Events(читает("toolu_read", "src/a.rs")),
        Turn::Events(правит("src/a.rs", "let x = 1;", "let x = 42;")),
        Turn::Events(script_text("Готово.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "поменяй единицу на сорок два").await;
    let events = collect(&mut rx).await;

    let имена = names(&events);
    assert!(имена.contains(&"fileChanged"), "получили {имена:?}");

    let (fact, diff) = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::FileChanged { fact, diff, .. } => Some((fact, diff)),
            _ => None,
        })
        .expect("события о правке не было");
    assert_eq!(fact.path, "src/a.rs");
    assert_eq!(fact.kind, crate::change::ChangeKind::Modified);
    assert_eq!((fact.added, fact.removed), (1, 1));
    assert!(!diff.hunks.is_empty(), "раскрытая строка без диффа пуста");

    // §16.37: подтверждение одно и уже дано — правка на диске.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "let x = 42;\n");
}

#[tokio::test]
async fn агент_перечитывает_собственную_правку() {
    let dir = TempDir::new();
    dir.write("a.rs", "старое\n");

    let provider = MockProvider::new(vec![
        Turn::Events(читает("t1", "a.rs")),
        Turn::Events(правит("a.rs", "старое", "новое")),
        Turn::Events(читает("t3", "a.rs")),
        Turn::Events(script_text("Всё на месте.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "замени слово").await;
    let events = collect(&mut rx).await;

    // Последнее чтение обязано вернуть правку: она уже на диске, но отпечаток
    // после записи должен быть свежим, иначе чтение упрётся в инвариант.
    let последний = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCallProgress { chunk, .. } => Some(chunk.clone()),
            _ => None,
        })
        .next_back()
        .expect("вывода тулов не было");
    assert!(последний.contains("новое"), "прочитано: {последний}");
}

#[tokio::test]
async fn правка_непрочитанного_файла_отклоняется_а_не_падает() {
    let dir = TempDir::new();
    let path = dir.write("a.rs", "чужой труд\n");

    let provider = MockProvider::new(vec![
        Turn::Events(правит("a.rs", "чужой труд", "мой труд")),
        Turn::Events(script_text("Понял, сначала читаю.")),
    ]);
    let handle = session(&dir, provider, Mode::Auto);
    let mut rx = handle.subscribe();

    ask(&handle, "перепиши файл").await;
    let events = collect(&mut rx).await;

    // §5.9: сработавшая защита — не сбой, и красится она иначе.
    let имена = names(&events);
    assert!(имена.contains(&"toolCallRejected"), "получили {имена:?}");
    assert!(!имена.contains(&"fileChanged"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "чужой труд\n");
}

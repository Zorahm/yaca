//! Tests of the registry: which tools a mode offers, and who signs a row.

use super::*;

#[test]
fn подпись_обязательна_только_у_тулов_модели() {
    let reg = ToolRegistry::standard();
    let schemas = reg.schemas(Intent::Agent);

    let read = schemas.iter().find(|s| s.name == "Read").unwrap();
    assert!(read.input_schema["properties"]["description"].is_object());
    assert!(read.input_schema["required"]
        .as_array()
        .unwrap()
        .contains(&json!("description")));

    // Список задач рисует интерфейс, подпись ему не нужна.
    // Файловые действия и список задач подписывает интерфейс: он знает
    // факт и не может ошибиться, а лишнее поле модель тратила бы зря.
    // Команду подписывает модель: из строки запуска интерфейс не поймёт,
    // зачем её звали, — смысл действия знает только она (§5.4.1).
    let bash = schemas.iter().find(|s| s.name == "Bash").unwrap();
    assert!(bash.input_schema["properties"]["description"].is_object());

    for имя in [
        "TodoWrite",
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
    ] {
        let t = schemas.iter().find(|s| s.name == имя).unwrap();
        assert!(
            t.input_schema["properties"]["description"].is_null(),
            "{имя} не должен просить подпись у модели"
        );
    }
}

#[test]
fn режим_plan_читает_и_планирует_но_не_пишет() {
    let reg = ToolRegistry::standard();
    let names: Vec<_> = reg
        .schemas(Intent::Plan)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(names.contains(&"Read".to_string()));
    assert!(names.contains(&"WebFetch".to_string()));
    assert!(names.contains(&"TodoWrite".to_string()));
    assert!(names.contains(&"ExitPlanMode".to_string()));
    // A subagent only reads, so sending one changes nothing either.
    assert!(names.contains(&"Task".to_string()));
    // §7.1: пишущие тулы не отдаются модели вообще, а не запрещаются
    // текстом промпта, — слабая модель запрет проигнорирует. Команда —
    // там же: план обещает ничего не менять, а команда может всё.
    for пишущий in [
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
        "Bash",
        "BashOutput",
    ] {
        assert!(
            !names.contains(&пишущий.to_string()),
            "{пишущий} утёк в plan"
        );
    }
}

#[test]
fn пишущие_тулы_есть_только_в_режиме_agent() {
    let reg = ToolRegistry::standard();
    let names: Vec<_> = reg
        .schemas(Intent::Agent)
        .into_iter()
        .map(|s| s.name)
        .collect();
    for пишущий in [
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
        "Bash",
        "BashOutput",
    ] {
        assert!(names.contains(&пишущий.to_string()), "{пишущий} потерялся");
    }
}

#[test]
fn a_subagent_has_the_agents_tools_less_the_agents_own() {
    let names = |reg: &ToolRegistry, intent| -> Vec<String> {
        reg.schemas(intent).into_iter().map(|s| s.name).collect()
    };
    let (all, given) = (ToolRegistry::standard(), ToolRegistry::delegated());

    for intent in [Intent::Plan, Intent::Agent] {
        let mut expected = names(&all, intent);
        expected.retain(|n| !["Task", "TodoWrite", "ExitPlanMode"].contains(&n.as_str()));
        assert_eq!(names(&given, intent), expected, "{intent:?}");
    }
    // The mode binds a subagent as it binds the agent (§7.1).
    assert!(!names(&given, Intent::Plan).contains(&"Edit".to_string()));
    assert!(names(&given, Intent::Agent).contains(&"Edit".to_string()));
}

#[test]
fn порядок_схем_устойчив_иначе_кеш_промпта_обнуляется() {
    let reg = ToolRegistry::standard();
    let a: Vec<_> = reg
        .schemas(Intent::Agent)
        .into_iter()
        .map(|s| s.name)
        .collect();
    let b: Vec<_> = reg
        .schemas(Intent::Agent)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(a, b);
    let mut sorted = a.clone();
    sorted.sort();
    assert_eq!(a, sorted);
}

#[test]
fn подпись_чистится_по_правилам_спеки() {
    let mut v = json!({ "description": "Сверил байты строки схемы." });
    assert_eq!(
        take_description(&mut v).as_deref(),
        Some("Сверил байты строки схемы")
    );
    assert!(
        v.get("description").is_none(),
        "поле должно уйти из аргументов"
    );

    let mut long = json!({ "description": "я".repeat(80) });
    let cut = take_description(&mut long).unwrap();
    assert_eq!(cut.chars().count(), DESCRIPTION_MAX_CHARS);

    let mut multiline = json!({ "description": "Первая строка\nвторая" });
    assert_eq!(
        take_description(&mut multiline).as_deref(),
        Some("Первая строка")
    );

    let mut empty = json!({ "description": "  " });
    assert_eq!(take_description(&mut empty), None);

    let mut absent = json!({ "path": "a.rs" });
    assert_eq!(take_description(&mut absent), None);
}

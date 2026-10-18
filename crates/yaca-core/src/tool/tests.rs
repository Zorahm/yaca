//! Tests of the registry: which tools a mode offers, and who signs a row.

use super::*;

#[test]
fn description_is_only_required_for_model_tools() {
    let reg = ToolRegistry::standard();
    let schemas = reg.schemas(Intent::Agent);

    let read = schemas.iter().find(|s| s.name == "Read").unwrap();
    assert!(read.input_schema["properties"]["description"].is_object());
    assert!(read.input_schema["required"]
        .as_array()
        .unwrap()
        .contains(&json!("description")));

    // The task list is drawn by the UI; it does not need a description.
    // File operations and task list are labeled by the UI: it knows
    // the fact and cannot make a mistake, while an extra field would waste the model's budget.
    // Commands are described by the model: the UI cannot infer why a command was run
    // from its command line — only the model knows the intent (§5.4.1).
    let bash = schemas.iter().find(|s| s.name == "Bash").unwrap();
    assert!(bash.input_schema["properties"]["description"].is_object());

    for name in [
        "TodoWrite",
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
    ] {
        let t = schemas.iter().find(|s| s.name == name).unwrap();
        assert!(
            t.input_schema["properties"]["description"].is_null(),
            "{name} should not request a description from the model"
        );
    }
}

#[test]
fn plan_mode_reads_and_plans_but_does_not_write() {
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
    // §7.1: writing tools are not given to the model at all, rather than
    // forbidden via prompt text — a weaker model would ignore the prohibition. Command
    // is treated the same: the plan promises not to change anything, while a command could do anything.
    for writing in [
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
        "BashOutput",
    ] {
        assert!(
            !names.contains(&writing.to_string()),
            "{writing} leaked into plan"
        );
    }
    // Bash is offered, and each call is judged when it arrives: a read-only
    // command runs and a write is refused (§7.1, tool::bash::plan_check).
    assert!(names.contains(&"Bash".to_string()));
}

#[test]
fn writing_tools_are_only_present_in_agent_mode() {
    let reg = ToolRegistry::standard();
    let names: Vec<_> = reg
        .schemas(Intent::Agent)
        .into_iter()
        .map(|s| s.name)
        .collect();
    for writing in [
        "Write",
        "Edit",
        "Delete",
        "DeleteDir",
        "Move",
        "Bash",
        "BashOutput",
    ] {
        assert!(names.contains(&writing.to_string()), "{writing} missing");
    }
}

#[test]
fn a_subagent_has_the_agents_tools_less_the_agents_own() {
    let names = |reg: &ToolRegistry, intent| -> Vec<String> {
        reg.schemas(intent).into_iter().map(|s| s.name).collect()
    };
    let (all, given) = (
        ToolRegistry::of_session(true),
        ToolRegistry::of_session(true).delegated(),
    );

    for intent in [Intent::Plan, Intent::Agent] {
        let mut expected = names(&all, intent);
        expected.retain(|n| {
            ![
                "Task",
                "TodoWrite",
                "ExitPlanMode",
                "AskUser",
                "SetTitle",
                "SendText",
                "SearchMessage",
                "ExtractMessage",
            ]
            .contains(&n.as_str())
        });
        assert_eq!(names(&given, intent), expected, "{intent:?}");
    }
    // The mode binds a subagent as it binds the agent (§7.1).
    assert!(!names(&given, Intent::Plan).contains(&"Edit".to_string()));
    assert!(names(&given, Intent::Agent).contains(&"Edit".to_string()));
}

#[test]
fn schema_order_is_stable_otherwise_prompt_cache_is_invalidated() {
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
fn description_is_sanitized_according_to_spec() {
    let mut v = json!({ "description": "Checked schema string bytes." });
    assert_eq!(
        take_description(&mut v).as_deref(),
        Some("Checked schema string bytes")
    );
    assert!(
        v.get("description").is_none(),
        "field should be removed from arguments"
    );

    let mut long = json!({ "description": "a".repeat(80) });
    let cut = take_description(&mut long).unwrap();
    assert_eq!(cut.chars().count(), DESCRIPTION_MAX_CHARS);

    let mut multiline = json!({ "description": "First line\nsecond" });
    assert_eq!(
        take_description(&mut multiline).as_deref(),
        Some("First line")
    );

    let mut empty = json!({ "description": "  " });
    assert_eq!(take_description(&mut empty), None);

    let mut absent = json!({ "path": "a.rs" });
    assert_eq!(take_description(&mut absent), None);
}

#[test]
fn the_tools_that_read_the_journal_come_with_one() {
    let names = |reg: &ToolRegistry, intent| -> Vec<String> {
        reg.schemas(intent).into_iter().map(|s| s.name).collect()
    };
    let (without, with) = (ToolRegistry::of_session(false), ToolRegistry::of_session(true));

    for tool in ["SearchMessage", "ExtractMessage"] {
        assert!(!names(&without, Intent::Agent).contains(&tool.to_string()), "{tool}");
        assert!(names(&with, Intent::Agent).contains(&tool.to_string()), "{tool}");
    }
    // Looking back changes nothing; a draft is a file, and plan writes none.
    assert!(names(&with, Intent::Plan).contains(&"SearchMessage".to_string()));
    assert!(!names(&with, Intent::Plan).contains(&"ExtractMessage".to_string()));
}

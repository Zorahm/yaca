use serde_json::{json, Value};
use std::sync::Arc;
use yaca_core::{tool::{lsp::LspQuery, rename::Rename, ToolRegistry}, Intent, Profile};

fn inspect(value: &Value, path: &str) {
    match value {
        Value::Object(map) => for (key, value) in map {
            if key == "description" {
                if let Some(text) = value.as_str() {
                    if text.contains("    ") { println!("WHITESPACE {path}.{key}"); }
                }
            }
            inspect(value, &format!("{path}.{key}"));
        },
        Value::Array(items) => for (i, item) in items.iter().enumerate() {
            inspect(item, &format!("{path}[{i}]"));
        },
        _ => {}
    }
}

fn main() {
    let expected: Value = serde_json::from_str(r#"{"description":"Hand a self-contained task to a subagent. It works in a fresh context of its own and returns only a report, so the steps it takes do not fill yours. Use it for work that takes many steps and comes down to a short result: finding where something is implemented or used, surveying an unfamiliar part of the project, a contained change you can describe completely, a question to research on the web. For something one or two calls would settle, make the calls yourself. The subagent has the tools you have, under the same mode and the same permissions: the human sees its calls and is asked about them as about yours. It sees none of this conversation and cannot ask a question, so the prompt has to carry everything it needs. Independent tasks requested together in one response run at the same time; give those different files to work on. You wait for the report unless you set `background`: then the call returns at once, you go on with your own work, and the report is handed to you when it is ready. What the subagent read or changed, you have not seen: read a file yourself before you edit it or rely on what it now says.","prompt":"The task, complete on its own: what to do or find out, what you already know that narrows it, where it must stop, and what the report should contain.","background":"Set to true when you have work of your own to do while the subagent does this, on files it will not touch. Leave it out when the report is the next thing you need: waiting for it in place costs nothing."}"#).unwrap();
    let task = yaca_core::tool::task::Task;
    use yaca_core::tool::Tool;
    assert_eq!(task.description(), expected["description"].as_str().unwrap());
    let properties = task.properties();
    for field in ["prompt", "background"] {
        assert_eq!(properties[field]["description"], expected[field]);
    }
    println!("VERIFIED Task text is unchanged after collapsing repeated spaces");
    for (label, registry) in [
        ("standard", ToolRegistry::standard()),
        ("journal+lsp", ToolRegistry::of_session(true).with(vec![Arc::new(LspQuery), Arc::new(Rename)])),
    ] {
        for intent in [Intent::Agent, Intent::Plan] {
            for profile in [Profile::Code, Profile::Chat] {
                let schemas = registry.schemas(intent, profile);
                let mut serialized = Vec::new();
                let mut captions = 0;
                for tool in schemas {
                    if tool.input_schema["properties"]["description"]["maxLength"] == 60 {
                        captions += 1;
                    }
                    let value = json!({"name": tool.name, "description": tool.description, "input_schema": tool.input_schema});
                    let chars = value.to_string().chars().count();
                    if label == "journal+lsp" && intent == Intent::Agent && profile == Profile::Code {
                        println!("TOOL {} {chars}", value["name"].as_str().unwrap());
                        inspect(&value, value["name"].as_str().unwrap());
                    }
                    serialized.push(value);
                }
                let json = Value::Array(serialized).to_string();
                println!("TOTAL {label} {intent:?} {profile:?} tools={} captions={captions} chars={} bytes={}",
                    Value::from(serde_json::from_str::<Vec<Value>>(&json).unwrap().len()), json.chars().count(), json.len());
            }
        }
    }
}
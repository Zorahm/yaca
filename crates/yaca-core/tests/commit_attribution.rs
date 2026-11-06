//! Commit attribution is opt-in and reaches both lead and delegated requests.

mod common;

use std::sync::Arc;

use serde_json::json;
use yaca_core::provider::mock::{script_text, script_tool_call, MockProvider, Turn};
use yaca_core::{Mode, SessionConfig};

use common::{ask, is_turn_over, until, TempProject};

const TRAILER: &str = "Co-Authored-By: yaca-agent <yaca@zorahm.com>";

#[tokio::test]
async fn commit_attribution_is_opt_in_for_lead_and_subagent_requests() {
    for preference in [None, Some(true), Some(false)] {
        let dir = TempProject::new();
        let provider = Arc::new(MockProvider::new(vec![
            Turn::Events(script_tool_call(
                "task",
                "Task",
                json!({ "prompt": "Inspect the project.", "description": "Inspect project" }),
            )),
            Turn::Events(script_text("Inspected.")),
            Turn::Events(script_text("Done.")),
            Turn::Events(script_text("Still done.")),
        ]));
        let mut config = SessionConfig::new(dir.path());
        config.mode = Mode::Manual;
        if let Some(enabled) = preference {
            config.co_author_commits = enabled;
        }
        let handle = yaca_core::spawn(config, provider.clone()).unwrap();
        let mut rx = handle.subscribe();
        ask(&handle, "Inspect this project using a subagent.").await;
        until(&mut rx, is_turn_over).await;
        ask(&handle, "Thanks.").await;
        until(&mut rx, is_turn_over).await;

        let requests = provider.seen.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_ne!(requests[0].system, requests[1].system);
        assert_eq!(requests[0].system, requests[2].system);
        assert_eq!(requests[0].system, requests[3].system);
        for request in requests.iter() {
            assert_eq!(
                request.system.matches(TRAILER).count(),
                usize::from(preference == Some(true)),
                "preference: {preference:?}"
            );
            assert_eq!(
                request.system.contains("<commit_attribution>"),
                preference == Some(true)
            );
        }
    }
}

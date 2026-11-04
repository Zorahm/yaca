//! Application lifetime LSP behavior through real sessions and server processes.
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{fixture, until_within, TempProject};
use yaca_core::event::AgentEvent;
use yaca_core::lsp::runtime::Registry;
use yaca_core::provider::mock::MockProvider;
use yaca_core::{Command, LspStatus, SessionConfig, SessionHandle};

fn setup(disabled: bool) -> (TempProject, TempProject, Arc<Registry>) {
    let root = TempProject::new();
    root.write("main.zz", "fine\n");
    let config = TempProject::new();
    config.write(
        "lsp.json",
        &serde_json::json!({"lspServers": {"fixture": {
            "command": fixture("lsp_fixture"), "extensions": ["zz"], "disabled": disabled
        }}})
        .to_string(),
    );
    let registry = Arc::new(Registry::new(config.path().to_path_buf()));
    (root, config, registry)
}

fn open(root: &TempProject, registry: &Arc<Registry>) -> SessionHandle {
    let mut config = SessionConfig::new(root.path());
    config.lsp_registry = Some(registry.clone());
    yaca_core::spawn(config, Arc::new(MockProvider::new(vec![]))).unwrap()
}

async fn status(handle: &SessionHandle, wanted: LspStatus) {
    let mut events = handle.subscribe();
    if handle
        .lsp()
        .iter()
        .any(|s| s.name == "fixture" && s.status == wanted)
    {
        return;
    }
    until_within(&mut events, Duration::from_secs(30), |event|
        matches!(event, AgentEvent::LspUpdated { servers } if servers.iter().any(|s| s.name == "fixture" && s.status == wanted))
    ).await;
}

#[tokio::test]
async fn startup_needs_no_chat_and_chats_share_the_project_server() {
    let (root, _config, registry) = setup(false);
    let lsp = registry.project(root.path()).unwrap();
    let (tx, mut events) = tokio::sync::broadcast::channel(32);
    let _listener = lsp.listen(Arc::new(yaca_core::event::EventSink::new(tx)));
    until_within(&mut events, Duration::from_secs(30), |event| {
        matches!(event, AgentEvent::LspUpdated { servers } if servers[0].status == LspStatus::Ready)
    }).await;
    let first = open(&root, &registry);
    let second = open(&root, &registry);
    status(&first, LspStatus::Ready).await;
    status(&second, LspStatus::Ready).await;
    first
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: false,
        })
        .await
        .unwrap();
    status(&second, LspStatus::Disabled).await;
    second
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: true,
        })
        .await
        .unwrap();
    status(&first, LspStatus::Ready).await;
    drop(first);
    drop(second);
    assert_eq!(lsp.snapshot()[0].status, LspStatus::Ready);
}

#[tokio::test]
async fn manual_disable_survives_a_new_application_registry() {
    let (root, config, registry) = setup(false);
    let handle = open(&root, &registry);
    status(&handle, LspStatus::Ready).await;
    handle
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: false,
        })
        .await
        .unwrap();
    status(&handle, LspStatus::Disabled).await;
    let restarted = Arc::new(Registry::new(config.path().to_path_buf()));
    let next = open(&root, &restarted);
    assert_eq!(next.lsp()[0].status, LspStatus::Disabled);
    next.send(Command::RestartLspServer {
        server: "fixture".into(),
    })
    .await
    .unwrap();
    next.send(Command::SetLspServerEnabled {
        server: "fixture".into(),
        enabled: true,
    })
    .await
    .unwrap();
    status(&next, LspStatus::Ready).await;
    let again = Registry::new(config.path().to_path_buf());
    assert_ne!(
        again.project(root.path()).unwrap().snapshot()[0].status,
        LspStatus::Disabled
    );
}

#[tokio::test]
async fn a_globally_disabled_server_does_not_start_until_enabled() {
    let (root, _config, registry) = setup(true);
    let handle = open(&root, &registry);
    assert_eq!(handle.lsp()[0].status, LspStatus::Disabled);
    handle
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: true,
        })
        .await
        .unwrap();
    status(&handle, LspStatus::Ready).await;
}

#[tokio::test]
async fn disabling_during_startup_cannot_publish_a_late_ready_state() {
    let (root, _config, registry) = setup(true);
    let handle = open(&root, &registry);
    let mut events = handle.subscribe();
    handle
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: true,
        })
        .await
        .unwrap();
    handle
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: false,
        })
        .await
        .unwrap();
    until_within(&mut events, Duration::from_secs(30), |event| {
        matches!(event, AgentEvent::LspUpdated { servers } if servers[0].status == LspStatus::Disabled)
    }).await;
    assert!(tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            if let AgentEvent::LspUpdated { servers } = events.recv().await.unwrap() {
                assert_eq!(servers[0].status, LspStatus::Disabled);
            }
        }
    })
    .await
    .is_err());
    assert_eq!(handle.lsp()[0].status, LspStatus::Disabled);
}

#[tokio::test]
async fn disabling_a_server_interrupts_an_in_flight_query() {
    use yaca_core::provider::mock::{script_text, Turn};
    let (root, config, _) = setup(false);
    config.write(
        "lsp.json",
        &serde_json::json!({"lspServers": {"fixture": {
            "command": fixture("lsp_fixture"), "args": ["--stall-on-hover"], "extensions": ["zz"]
        }}})
        .to_string(),
    );
    let registry = Arc::new(Registry::new(config.path().to_path_buf()));
    let provider = Arc::new(MockProvider::new(vec![
        Turn::Events(common::calls(&[(
            "t1",
            "Lsp",
            serde_json::json!({
                "action": "hover", "path": "main.zz", "line": 1, "description": "Inspect type"
            }),
        )])),
        Turn::Events(script_text("Done.")),
    ]));
    let mut settings = SessionConfig::new(root.path());
    settings.lsp_registry = Some(registry);
    settings.mode = yaca_core::Mode::Auto;
    let handle = yaca_core::spawn(settings, provider.clone()).unwrap();
    let mut events = handle.subscribe();
    common::ask(&handle, "inspect").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !root.path().join("hover-started").is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    handle
        .send(Command::SetLspServerEnabled {
            server: "fixture".into(),
            enabled: false,
        })
        .await
        .unwrap();
    until_within(&mut events, Duration::from_secs(3), common::is_turn_over).await;
    assert!(common::tool_result_of(&provider, "t1").1);
    assert_eq!(handle.lsp()[0].status, LspStatus::Disabled);
}

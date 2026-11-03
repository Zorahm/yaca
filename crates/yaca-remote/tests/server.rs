//! The server end to end, through its public API: real TLS, real sockets, real core
//! sessions. Each test starts its own server on a free loopback port.

mod common;

use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::Command;
use yaca_remote::protocol::{
    Attention, Call, CallResult, ClientMessage, DeviceId, ErrorCode, Refusal, Reply, ServerMessage,
    PROTOCOL_VERSION,
};

use common::{code_of, connect, hello_message, pair_message, start, Client, TempDir, TestHost};

/// Pairs a phone and disconnects it, returning the credentials it was given.
async fn pair(server: &yaca_remote::RemoteServer) -> (DeviceId, String) {
    let code = code_of(&server.offer_pairing().unwrap().uri);
    let mut phone = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    phone.send(&pair_message(&code)).await;
    match phone.read().await {
        Some(ServerMessage::Paired {
            device_id, token, ..
        }) => (device_id, token),
        other => panic!("expected Paired, got {other:?}"),
    }
}

/// A connection that has been welcomed back with the given credentials.
async fn hello(server: &yaca_remote::RemoteServer, device: &DeviceId, token: &str) -> Client {
    let mut phone = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    phone.send(&hello_message(device.clone(), token)).await;
    match phone.read().await {
        Some(ServerMessage::Welcome { .. }) => phone,
        other => panic!("expected Welcome, got {other:?}"),
    }
}

#[tokio::test]
async fn pairing_once_then_hello_then_refusals() {
    let dir = TempDir::new("handshake");
    let server = start(&dir, TestHost::new()).await;
    let offer = server.offer_pairing().unwrap();
    let code = code_of(&offer.uri);

    let mut phone = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    phone.send(&pair_message(&code)).await;
    let (device, token) = match phone.read().await {
        Some(ServerMessage::Paired {
            device_id,
            token,
            host,
        }) => {
            assert_eq!(host.name, "test-host");
            assert_eq!(host.protocol, PROTOCOL_VERSION);
            (device_id, token)
        }
        other => panic!("expected Paired, got {other:?}"),
    };
    drop(phone);

    // The code is spent by the pairing above.
    let mut again = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    again.send(&pair_message(&code)).await;
    assert_eq!(
        again.read().await,
        Some(ServerMessage::Refused {
            reason: Refusal::PairingFailed
        })
    );

    // The paired device gets in with its token.
    drop(hello(&server, &device, &token).await);

    let mut wrong_token = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    wrong_token
        .send(&hello_message(device.clone(), "not the token"))
        .await;
    assert_eq!(
        wrong_token.read().await,
        Some(ServerMessage::Refused {
            reason: Refusal::Unauthorized
        })
    );

    let mut old = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    old.send(&ClientMessage::Hello {
        device_id: device,
        token,
        protocol: PROTOCOL_VERSION + 1,
    })
    .await;
    assert_eq!(
        old.read().await,
        Some(ServerMessage::Refused {
            reason: Refusal::ProtocolMismatch {
                server: PROTOCOL_VERSION
            }
        })
    );
}

#[tokio::test]
async fn a_client_pinning_another_fingerprint_fails_the_tls_handshake() {
    let dir = TempDir::new("pin");
    let server = start(&dir, TestHost::new()).await;

    let wrong = "0".repeat(64);
    assert!(connect(server.local_addr(), &wrong).await.is_err());

    // The same server with the right pin is reachable, so the failure above was the pin.
    assert!(connect(server.local_addr(), server.fingerprint())
        .await
        .is_ok());
}

#[tokio::test]
async fn an_attached_chat_streams_its_turn_and_asks_for_attention() {
    let dir = TempDir::new("chat");
    let project = TempDir::new("project");
    let server = start(&dir, TestHost::new()).await;
    let (device, token) = pair(&server).await;
    let mut phone = hello(&server, &device, &token).await;

    phone
        .wait_for(|m| matches!(m, ServerMessage::ChatsChanged { chats } if chats.is_empty()))
        .await;
    let project_path = project.path().display().to_string();
    let opened = match phone
        .request(Call::OpenChat {
            project: project_path.clone(),
            resume: None,
            startup: None,
        })
        .await
    {
        CallResult::Ok {
            reply: Reply::Opened { chat },
        } => *chat,
        other => panic!("expected Opened, got {other:?}"),
    };
    let session = opened.id.clone();

    // The list that names the chat goes out after its attention watcher is in place,
    // so the turn below cannot start unwatched.
    phone
        .wait_for(|m| matches!(m, ServerMessage::ChatsChanged { chats } if chats.iter().any(|c| c.id == session)))
        .await;

    match phone
        .request(Call::Attach {
            session: session.clone(),
        })
        .await
    {
        CallResult::Ok {
            reply: Reply::Attached { epoch, .. },
        } => assert_eq!(epoch, 1),
        other => panic!("expected Attached, got {other:?}"),
    }

    let sent = phone
        .request(Call::Send {
            session: session.clone(),
            command: Command::SendMessage {
                text: "hi".into(),
                attachments: vec![],
            },
        })
        .await;
    assert_eq!(sent, CallResult::Ok { reply: Reply::Done });

    let mut text = String::new();
    let mut turn_finished_in_frames = false;
    let mut turn_finished_attention = false;
    while !(turn_finished_in_frames && turn_finished_attention) {
        match phone.next().await {
            ServerMessage::Frame {
                session: frame_session,
                epoch,
                events,
            } if frame_session == session => {
                assert_eq!(epoch, 1);
                for event in events {
                    match event {
                        AgentEvent::TextDelta { text: delta, .. } => text.push_str(&delta),
                        AgentEvent::TurnFinished { .. } => turn_finished_in_frames = true,
                        _ => {}
                    }
                }
            }
            ServerMessage::Attention {
                session: asked,
                project: asked_project,
                reason: Attention::TurnFinished { .. },
                ..
            } if asked == session => {
                assert_eq!(asked_project, project_path);
                turn_finished_attention = true;
            }
            _ => {}
        }
    }
    assert!(
        text.contains("Hello from the mock."),
        "streamed text was {text:?}"
    );
}

#[tokio::test]
async fn revoking_a_device_closes_its_connection_and_its_token_stops_working() {
    let dir = TempDir::new("revoke");
    let server = start(&dir, TestHost::new()).await;
    let code = code_of(&server.offer_pairing().unwrap().uri);

    let mut phone = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    phone.send(&pair_message(&code)).await;
    let device = match phone.read().await {
        Some(ServerMessage::Paired { device_id, .. }) => device_id,
        other => panic!("expected Paired, got {other:?}"),
    };

    assert!(server.revoke(&device).unwrap());
    phone.read_to_close().await;

    let token = "stolen";
    let mut later = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    later.send(&hello_message(device, token)).await;
    assert_eq!(
        later.read().await,
        Some(ServerMessage::Refused {
            reason: Refusal::Unauthorized
        })
    );
    assert!(server.devices().is_empty());
}

#[tokio::test]
async fn five_failed_handshakes_refuse_the_sixth_connection_before_it_speaks() {
    let dir = TempDir::new("limit");
    let server = start(&dir, TestHost::new()).await;

    for _ in 0..5 {
        let mut attempt = connect(server.local_addr(), server.fingerprint())
            .await
            .unwrap();
        attempt
            .send(&hello_message(DeviceId("nobody".into()), "wrong"))
            .await;
        assert_eq!(
            attempt.read().await,
            Some(ServerMessage::Refused {
                reason: Refusal::Unauthorized
            })
        );
    }

    // Nothing is sent: the refusal comes before any message is read.
    let mut sixth = connect(server.local_addr(), server.fingerprint())
        .await
        .unwrap();
    assert_eq!(
        sixth.read().await,
        Some(ServerMessage::Refused {
            reason: Refusal::TooManyAttempts
        })
    );
    assert_eq!(sixth.read().await, None);
}

#[tokio::test]
async fn a_send_to_a_chat_that_is_not_open_is_chat_not_open() {
    let dir = TempDir::new("not-open");
    let server = start(&dir, TestHost::new()).await;
    let (device, token) = pair(&server).await;
    let mut phone = hello(&server, &device, &token).await;

    let result = phone
        .request(Call::Send {
            session: SessionId::new(),
            command: Command::SendMessage {
                text: "hi".into(),
                attachments: vec![],
            },
        })
        .await;
    match result {
        CallResult::Error { error } => assert_eq!(error.code, ErrorCode::ChatNotOpen),
        other => panic!("expected ChatNotOpen, got {other:?}"),
    }
}

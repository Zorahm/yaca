//! The handshake: the first message of a connection, and nothing else is read until
//! it succeeds.
//!
//! Every failure is a refusal, sent with a reason and then a closed socket, and
//! every refusal counts against the address. The reasons are kept distinct where
//! the client can act on them (a wrong protocol, a spent code), and merged where
//! telling them apart would help a guesser (an unknown device and a wrong token).

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::connection::{Ws, HANDSHAKE_TIMEOUT};
use crate::protocol::{
    ClientMessage, DeviceId, HostInfo, Refusal, ServerMessage, PROTOCOL_VERSION,
};
use crate::shared::{LiveGuard, Shared};
use crate::writer::encode;

/// Runs the handshake. `Some` when the device is admitted; the guard keeps the
/// connection registered for revoke for as long as it is held.
pub(crate) async fn run(
    ws: &mut Ws,
    shared: &Arc<Shared>,
    peer: IpAddr,
    token: &CancellationToken,
) -> Option<LiveGuard> {
    // Checked before anything is read: an address that has failed enough gets one
    // refusal and nothing it sends is parsed.
    if shared.limiter.blocked(peer, Instant::now()) {
        refuse(ws, shared, peer, Refusal::TooManyAttempts).await;
        return None;
    }
    let Some(message) = first_message(ws).await else {
        refuse(ws, shared, peer, Refusal::HandshakeExpected).await;
        return None;
    };
    // A request is not a handshake, however well formed.
    let protocol = match &message {
        ClientMessage::Pair { protocol, .. } | ClientMessage::Hello { protocol, .. } => *protocol,
        ClientMessage::Request { .. } => {
            refuse(ws, shared, peer, Refusal::HandshakeExpected).await;
            return None;
        }
    };
    // Before the token or the code is looked at: an old client should be told to
    // update, not refused for a credential it could never have got right.
    if protocol != PROTOCOL_VERSION {
        refuse(
            ws,
            shared,
            peer,
            Refusal::ProtocolMismatch {
                server: PROTOCOL_VERSION,
            },
        )
        .await;
        return None;
    }
    match message {
        ClientMessage::Pair {
            code, device_name, ..
        } => pair(ws, shared, peer, token, &code, &device_name).await,
        ClientMessage::Hello {
            device_id,
            token: device_token,
            ..
        } => hello(ws, shared, peer, token, &device_id, &device_token).await,
        // Requests were turned away above.
        ClientMessage::Request { .. } => None,
    }
}

async fn pair(
    ws: &mut Ws,
    shared: &Arc<Shared>,
    peer: IpAddr,
    token: &CancellationToken,
    code: &str,
    device_name: &str,
) -> Option<LiveGuard> {
    if !shared.pairing.redeem(code) {
        refuse(ws, shared, peer, Refusal::PairingFailed).await;
        return None;
    }
    let (device, device_token) = match shared.devices.register(device_name) {
        Ok(registered) => registered,
        Err(e) => {
            // The host's fault, not the phone's, so it is not a refusal. The code is
            // spent and the phone is told nothing beyond the closed socket; the cause
            // is in the host's log.
            tracing::error!(error = %e, "could not record a paired device");
            let _ = ws.close(None).await;
            return None;
        }
    };
    let Some(live) = shared.register(&device, token.clone()) else {
        refuse(ws, shared, peer, Refusal::Unauthorized).await;
        return None;
    };
    tracing::info!(device = %device.0, "device paired");
    send(
        ws,
        &ServerMessage::Paired {
            device_id: device,
            token: device_token,
            host: host_info(shared),
        },
    )
    .await;
    Some(live)
}

async fn hello(
    ws: &mut Ws,
    shared: &Arc<Shared>,
    peer: IpAddr,
    token: &CancellationToken,
    device: &DeviceId,
    device_token: &str,
) -> Option<LiveGuard> {
    if !shared.devices.verify(device, device_token) {
        refuse(ws, shared, peer, Refusal::Unauthorized).await;
        return None;
    }
    // Verified just now, yet revoked before it could register: the same answer as a
    // wrong token, because to the phone that is what it is.
    let Some(live) = shared.register(device, token.clone()) else {
        refuse(ws, shared, peer, Refusal::Unauthorized).await;
        return None;
    };
    tracing::debug!(device = %device.0, "device connected");
    send(
        ws,
        &ServerMessage::Welcome {
            host: host_info(shared),
        },
    )
    .await;
    Some(live)
}

/// The first message, if it is a text frame that parses within the handshake time.
async fn first_message(ws: &mut Ws) -> Option<ClientMessage> {
    let frame = timeout(HANDSHAKE_TIMEOUT, ws.next()).await.ok()?;
    let Message::Text(text) = frame?.ok()? else {
        return None;
    };
    serde_json::from_str(text.as_str()).ok()
}

/// Sends the refusal and closes. Every refusal counts against the address.
async fn refuse(ws: &mut Ws, shared: &Shared, peer: IpAddr, reason: Refusal) {
    shared.limiter.record(peer, Instant::now());
    tracing::debug!(%peer, ?reason, "handshake refused");
    send(ws, &ServerMessage::Refused { reason }).await;
    let _ = ws.close(None).await;
}

async fn send(ws: &mut Ws, message: &ServerMessage) {
    if let Some(frame) = encode(message) {
        // A failed write means the phone is already gone; the caller closes anyway.
        let _ = ws.send(frame).await;
    }
}

fn host_info(shared: &Shared) -> HostInfo {
    HostInfo {
        name: shared.host.name(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol: PROTOCOL_VERSION,
    }
}

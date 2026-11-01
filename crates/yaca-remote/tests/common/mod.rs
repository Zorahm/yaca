//! Scaffolding for the server's integration tests: a throwaway directory, a host
//! that opens real core sessions, and a client that accepts exactly one certificate.
//!
//! Real sockets, real TLS and real core sessions throughout: a test of the server
//! that mocked any of them would not be testing what the phone meets.

// Each test binary links its own copy and uses a subset of this.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{client_async, WebSocketStream};
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::provider::mock::MockProvider;
use yaca_core::usage::ModelInfo;
use yaca_core::{Command, SessionConfig, SessionMeta};
use yaca_remote::host::{Host, LiveChat};
use yaca_remote::protocol::{
    Call, CallResult, ClientMessage, ServerMessage, Startup, PROTOCOL_VERSION,
};
use yaca_remote::tls::fingerprint;
use yaca_remote::{RemoteServer, ServerOptions};

/// Longest a test waits for one message before it fails instead of hanging.
const WAIT: Duration = Duration::from_secs(5);

/// A directory under the system temp dir, unique per call and removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "yaca-remote-it-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The host of these tests: every chat is a real core session driven by a mock
/// provider that answers with one line of text.
pub struct TestHost {
    chats: Mutex<Vec<LiveChat>>,
    changed: watch::Sender<u64>,
}

impl TestHost {
    pub fn new() -> Arc<Self> {
        let (changed, _) = watch::channel(0);
        Arc::new(Self {
            chats: Mutex::new(Vec::new()),
            changed,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Vec<LiveChat>> {
        self.chats.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[async_trait]
impl Host for TestHost {
    fn name(&self) -> String {
        "test-host".into()
    }

    fn projects(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    fn sessions(&self, _project: &str) -> Result<Vec<SessionMeta>, String> {
        Ok(Vec::new())
    }

    fn feed(&self, _project: &str, _session: &SessionId) -> Result<Vec<AgentEvent>, String> {
        Err("the test host keeps no journals".into())
    }

    fn models(&self) -> Result<Vec<ModelInfo>, String> {
        Ok(Vec::new())
    }

    async fn open_chat(
        &self,
        project: &str,
        resume: Option<SessionId>,
        _startup: Option<Startup>,
    ) -> Result<LiveChat, String> {
        if resume.is_some() {
            return Err("the test host does not resume chats".into());
        }
        let provider = Arc::new(MockProvider::text("Hello from the mock."));
        let handle = yaca_core::spawn(SessionConfig::new(project), provider)
            .map_err(|e| e.to_string())?;
        let live = LiveChat {
            handle: Arc::new(handle),
            project: project.to_string(),
        };
        self.lock().push(live.clone());
        self.changed.send_modify(|n| *n += 1);
        Ok(live)
    }

    fn live_chats(&self) -> Vec<LiveChat> {
        self.lock().clone()
    }

    fn chat(&self, session: &SessionId) -> Option<LiveChat> {
        self.lock()
            .iter()
            .find(|chat| chat.handle.id() == session)
            .cloned()
    }

    async fn close_chat(&self, session: &SessionId) -> Result<(), String> {
        let closed = {
            let mut chats = self.lock();
            let at = chats.iter().position(|chat| chat.handle.id() == session);
            at.map(|at| chats.remove(at))
        };
        if let Some(chat) = closed {
            let _ = chat.handle.send(Command::Cancel).await;
            self.changed.send_modify(|n| *n += 1);
        }
        Ok(())
    }

    fn chats_changed(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
}

/// Starts a server on a free port of the loopback address, with `host`.
pub async fn start(dir: &TempDir, host: Arc<TestHost>) -> RemoteServer {
    let options = ServerOptions {
        dir: dir.path().to_path_buf(),
        port: 0,
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
    };
    RemoteServer::start(options, host).await.unwrap()
}

/// The one-time code out of an offer's URI.
pub fn code_of(uri: &str) -> String {
    uri.split(['?', '&'])
        .find_map(|pair| pair.strip_prefix("code="))
        .expect("the offer carries a code")
        .to_string()
}

/// Accepts exactly the certificate whose fingerprint was pinned, and nothing else.
/// The signatures are still checked, so the key has to be the one the certificate
/// carries, not merely a certificate that happens to have the right digest.
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint(end_entity.as_ref()) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("certificate is not the pinned one".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// A phone's side of the connection: messages the server sent while the test was
/// waiting for something else are kept, in order, for later.
pub struct Client {
    ws: WebSocketStream<TlsStream<TcpStream>>,
    pending: VecDeque<ServerMessage>,
    next_id: u64,
}

/// Connects over TCP and TLS, pinning `fingerprint`, then upgrades to a WebSocket.
pub async fn connect(addr: SocketAddr, fingerprint: &str) -> Result<Client, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Arc::new(Pinned {
        fingerprint: fingerprint.to_string(),
        provider: Arc::clone(&provider),
    });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    let tcp = TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
    let name = ServerName::try_from("yaca.local").unwrap();
    let tls = TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
        .map_err(|e| e.to_string())?;
    let (ws, _) = client_async("wss://yaca.local/", tls)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Client {
        ws,
        pending: VecDeque::new(),
        next_id: 1,
    })
}

impl Client {
    pub async fn send(&mut self, message: &ClientMessage) {
        let text = serde_json::to_string(message).unwrap();
        self.ws.send(Message::text(text)).await.unwrap();
    }

    /// The next message off the socket, or `None` once the server closed it.
    /// Fails the test if nothing arrives within [`WAIT`].
    pub async fn read(&mut self) -> Option<ServerMessage> {
        loop {
            let next = timeout(WAIT, self.ws.next())
                .await
                .expect("no message from the server in time");
            match next? {
                Ok(Message::Text(text)) => {
                    return Some(
                        serde_json::from_str(text.as_str())
                            .expect("the server sent a message this client cannot read"),
                    );
                }
                Ok(Message::Close(_)) | Err(_) => return None,
                Ok(_) => continue,
            }
        }
    }

    /// The next message, whichever it is: the held ones first, then the socket.
    pub async fn next(&mut self) -> ServerMessage {
        if let Some(message) = self.pending.pop_front() {
            return message;
        }
        self.read().await.expect("the connection closed")
    }

    /// Waits for the first message that `wanted` accepts. Messages it refuses are
    /// held for [`Client::next`], so waiting for one thing never loses another.
    pub async fn wait_for(&mut self, wanted: impl Fn(&ServerMessage) -> bool) -> ServerMessage {
        if let Some(at) = self.pending.iter().position(&wanted) {
            return self.pending.remove(at).expect("the position was just found");
        }
        loop {
            let message = self.read().await.expect("the connection closed while waiting");
            if wanted(&message) {
                return message;
            }
            self.pending.push_back(message);
        }
    }

    /// Reads until the server closes the connection, and fails if it does not.
    pub async fn read_to_close(&mut self) {
        while self.read().await.is_some() {}
    }

    /// Sends a request and waits for its response.
    pub async fn request(&mut self, call: Call) -> CallResult {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&ClientMessage::Request { id, call }).await;
        match self
            .wait_for(|m| matches!(m, ServerMessage::Response { id: got, .. } if *got == id))
            .await
        {
            ServerMessage::Response { result, .. } => result,
            other => panic!("expected the response to request {id}, got {other:?}"),
        }
    }
}

/// The handshake a fresh phone sends, with this build's protocol number.
pub fn pair_message(code: &str) -> ClientMessage {
    ClientMessage::Pair {
        code: code.into(),
        device_name: "Test phone".into(),
        protocol: PROTOCOL_VERSION,
    }
}

pub fn hello_message(device_id: yaca_remote::protocol::DeviceId, token: &str) -> ClientMessage {
    ClientMessage::Hello {
        device_id,
        token: token.into(),
        protocol: PROTOCOL_VERSION,
    }
}

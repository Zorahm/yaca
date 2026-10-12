//! The language servers of one session and the state each is in.
//!
//! A server is started by the first call that needs it, not when the session
//! opens: most sessions never ask a language server anything, and
//! `rust-analyzer` on a large workspace is not a thing to start for nothing.
//! State leaves whole, as [`AgentEvent::LspUpdated`].
//!
//! A server that dies is not restarted by itself. It stays `Failed` with the
//! reason, and the next call that needs it tries once more — a crash that
//! repeats is then one failed call each, never a loop in the background.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::client::{Client, Notice};
use super::config::ServerSpec;
use super::{LspError, LspServerSummary, LspStatus};
use crate::event::{AgentEvent, EventSink};

/// How long a server may take to answer the handshake. Indexing goes on after
/// it and is not waited for.
const STARTUP: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Lsp(Arc<Inner>);

struct Inner {
    /// Where servers start and what they are told is the project.
    root: PathBuf,
    events: EventSink,
    servers: Mutex<BTreeMap<String, Server>>,
}

struct Server {
    spec: ServerSpec,
    state: State,
    /// Held while the server is being started, so two calls that need it at
    /// once start one process, not two.
    starting: Arc<tokio::sync::Mutex<()>>,
    /// Which start the state belongs to. A close notice from an earlier
    /// process finds a different number and leaves the state alone.
    attempt: u64,
}

enum State {
    Idle,
    Starting,
    Ready(Arc<Client>),
    Failed(String),
}

impl Lsp {
    /// A session with no language servers.
    pub fn none(events: EventSink) -> Lsp {
        Lsp::new(Vec::new(), Path::new("."), events)
    }

    /// Nothing is started: the specs are what may be, and each starts when a
    /// call needs it.
    pub fn new(specs: Vec<ServerSpec>, root: &Path, events: EventSink) -> Lsp {
        let servers = specs
            .into_iter()
            .map(|spec| {
                let state = match &spec.invalid {
                    Some(reason) => State::Failed(reason.clone()),
                    None => State::Idle,
                };
                let server = Server {
                    spec,
                    state,
                    starting: Arc::default(),
                    attempt: 0,
                };
                (server.spec.name.clone(), server)
            })
            .collect::<BTreeMap<_, _>>();
        let listed = !servers.is_empty();
        let inner = Arc::new(Inner {
            root: root.to_path_buf(),
            events,
            servers: Mutex::new(servers),
        });
        if listed {
            inner.announce();
        }
        Lsp(inner)
    }

    pub fn snapshot(&self) -> Vec<LspServerSummary> {
        self.0.snapshot()
    }

    /// The server that answers for `path`, by its extension.
    pub fn server_for(&self, path: &Path) -> Option<String> {
        let extension = path.extension()?.to_str()?;
        self.0
            .servers
            .lock()
            .unwrap()
            .values()
            .find(|server| server.spec.handles(extension))
            .map(|server| server.spec.name.clone())
    }

    /// The server a request that names no file goes to: the one already up
    /// when there is exactly one, else none.
    pub fn only_ready(&self) -> Option<String> {
        let servers = self.0.servers.lock().unwrap();
        let mut ready = servers
            .values()
            .filter(|s| matches!(s.state, State::Ready(_)))
            .map(|s| s.spec.name.clone());
        match (ready.next(), ready.next()) {
            (Some(one), None) => Some(one),
            _ => None,
        }
    }

    /// The names of the servers that could be asked, for a message that tells
    /// the model what to name.
    pub fn names(&self) -> Vec<String> {
        self.0.servers.lock().unwrap().keys().cloned().collect()
    }

    /// The server's client, starting the server when it is not up.
    pub(crate) async fn client(
        &self,
        name: &str,
        cancel: &CancellationToken,
    ) -> Result<Arc<Client>, LspError> {
        let lock = {
            let servers = self.0.servers.lock().unwrap();
            let server = servers
                .get(name)
                .ok_or_else(|| LspError::Transport(format!("нет сервера «{name}»")))?;
            if let State::Ready(client) = &server.state {
                return Ok(client.clone());
            }
            server.starting.clone()
        };

        let _starting = tokio::select! {
            guard = lock.lock() => guard,
            _ = cancel.cancelled() => return Err(LspError::Cancelled),
        };

        // Whoever held the lock before may have started it.
        let (spec, attempt) = {
            let mut servers = self.0.servers.lock().unwrap();
            let server = servers.get_mut(name).expect("checked above");
            match &server.state {
                State::Ready(client) => return Ok(client.clone()),
                State::Failed(reason) if server.spec.invalid.is_some() => {
                    return Err(LspError::Transport(reason.clone()));
                }
                _ => {}
            }
            if server.spec.settings.disabled {
                return Err(LspError::Transport(format!("сервер «{name}» отключён")));
            }
            server.attempt += 1;
            server.state = State::Starting;
            (server.spec.clone(), server.attempt)
        };
        self.0.announce();

        let (notices, mut closed) = mpsc::unbounded_channel();
        let connecting = Client::connect(&spec, &self.0.root, notices);
        let outcome = tokio::select! {
            connected = tokio::time::timeout(STARTUP, connecting) => match connected {
                Ok(result) => result,
                Err(_) => Err(LspError::Timeout(STARTUP.as_secs())),
            },
            _ = cancel.cancelled() => Err(LspError::Cancelled),
        };

        match outcome {
            Ok(client) => {
                let client = Arc::new(client);
                {
                    let mut servers = self.0.servers.lock().unwrap();
                    if let Some(server) = servers.get_mut(name) {
                        server.state = State::Ready(client.clone());
                    }
                }
                self.0.announce();

                let weak = Arc::downgrade(&self.0);
                let name = name.to_string();
                tokio::spawn(async move {
                    if let Some(Notice::Closed(reason)) = closed.recv().await {
                        if let Some(inner) = Weak::upgrade(&weak) {
                            inner.failed(&name, attempt, reason);
                        }
                    }
                });
                Ok(client)
            }
            Err(error) => {
                // A cancelled start is nobody's failure: the server may be
                // fine, and the next call should simply try again.
                let state = if matches!(error, LspError::Cancelled) {
                    State::Idle
                } else {
                    State::Failed(error.to_string())
                };
                {
                    let mut servers = self.0.servers.lock().unwrap();
                    if let Some(server) = servers.get_mut(name) {
                        server.state = state;
                    }
                }
                self.0.announce();
                Err(error)
            }
        }
    }

    /// The client of a server that is already up, without starting anything.
    pub(crate) fn ready(&self, name: &str) -> Option<Arc<Client>> {
        match &self.0.servers.lock().unwrap().get(name)?.state {
            State::Ready(client) => Some(client.clone()),
            _ => None,
        }
    }
}

impl Inner {
    fn failed(&self, name: &str, attempt: u64, reason: String) {
        {
            let mut servers = self.servers.lock().unwrap();
            match servers.get_mut(name) {
                Some(server) if server.attempt == attempt => {
                    server.state = State::Failed(reason);
                }
                _ => return,
            }
        }
        self.announce();
    }

    fn snapshot(&self) -> Vec<LspServerSummary> {
        self.servers
            .lock()
            .unwrap()
            .values()
            .map(|server| {
                let (status, reason, label) = match &server.state {
                    _ if server.spec.settings.disabled => (LspStatus::Disabled, None, None),
                    State::Idle => (LspStatus::Idle, None, None),
                    State::Starting => (LspStatus::Starting, None, None),
                    State::Ready(client) => (LspStatus::Ready, None, client.label.clone()),
                    State::Failed(reason) => (LspStatus::Failed, Some(reason.clone()), None),
                };
                let settings = &server.spec.settings;
                LspServerSummary {
                    name: server.spec.name.clone(),
                    command: std::iter::once(settings.command.clone())
                        .chain(settings.args.iter().cloned())
                        .collect::<Vec<_>>()
                        .join(" "),
                    extensions: settings.extensions.clone(),
                    status,
                    reason,
                    label,
                }
            })
            .collect()
    }

    fn announce(&self) {
        self.events.emit(AgentEvent::LspUpdated {
            servers: self.snapshot(),
        });
    }
}

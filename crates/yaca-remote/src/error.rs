//! What can go wrong in running the server itself, as opposed to answering a
//! call: a failed call travels to the client as a [`CallError`], and the
//! server goes on.
//!
//! [`CallError`]: crate::protocol::CallError

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tls: {0}")]
    Tls(#[from] rustls::Error),
    #[error("certificate: {0}")]
    Certificate(String),
    /// Boxed: the tungstenite error is large enough to make every `Result` in the
    /// crate pay for it (clippy's `result_large_err`). `From` keeps `?` working.
    #[error("websocket: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    #[error("qr code: {0}")]
    Qr(String),
    #[error("random source: {0}")]
    Random(String),
    #[error("clock: {0}")]
    Clock(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(e))
    }
}

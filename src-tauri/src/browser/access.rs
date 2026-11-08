//! Human decisions about agent requests, before Chromium sends them.

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::Url;
use tokio::sync::oneshot;

use super::Browser;

const DECISION_LIMIT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRequest {
    pub id: u32,
    pub origin: String,
    pub method: String,
    pub submission: bool,
}

#[derive(Default)]
pub(super) struct Access {
    inner: Mutex<Decisions>,
}

#[derive(Default)]
struct Decisions {
    next: u32,
    requests: Vec<(BrowserRequest, oneshot::Sender<bool>)>,
}

impl Access {
    pub fn pending(&self) -> Option<BrowserRequest> {
        self.inner.lock().unwrap().requests.first().map(|(request, _)| request.clone())
    }

    pub async fn ask(
        &self,
        origin: String,
        method: String,
        submission: bool,
        changed: impl Fn(),
    ) -> bool {
        let (tx, rx) = oneshot::channel();
        let id = {
            let mut inner = self.inner.lock().unwrap();
            if inner.requests.len() >= 8 {
                return false;
            }
            inner.next = inner.next.wrapping_add(1);
            let id = inner.next;
            inner.requests.push((BrowserRequest { id, origin, method, submission }, tx));
            id
        };
        changed();
        let allowed = matches!(tokio::time::timeout(DECISION_LIMIT, rx).await, Ok(Ok(true)));
        self.answer(id, false);
        changed();
        allowed
    }

    pub fn answer(&self, id: u32, allow: bool) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(index) = inner.requests.iter().position(|(request, _)| request.id == id) {
            let (_, sender) = inner.requests.remove(index);
            let _ = sender.send(allow);
        }
    }

    pub fn deny_all(&self) {
        self.inner.lock().unwrap().requests.clear();
    }
}

impl Browser {
    pub(super) async fn authorize(
        &self,
        url: &Url,
        from: Option<&str>,
        method: &str,
        submission: bool,
    ) -> bool {
        if self.paused.load(Ordering::SeqCst) || !super::allowed(url) {
            return false;
        }
        let Some(origin) = origin(url) else {
            return !submission;
        };
        if !submission && (from == Some(origin.as_str())
            || origins().is_ok_and(|origins| origins.contains(&origin)))
        {
            return true;
        }
        self.access.ask(origin, method.to_string(), submission, || self.emit()).await
            && !self.paused.load(Ordering::SeqCst)
    }
}

pub(super) fn origin(url: &Url) -> Option<String> {
    matches!(url.scheme(), "http" | "https").then(|| url.origin().ascii_serialization())
}

fn path() -> Result<std::path::PathBuf, String> {
    Ok(crate::config_dir()?.join("browser-origins.json"))
}

pub(super) fn origins() -> Result<Vec<String>, String> {
    match std::fs::read(path()?) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn save_origins(text: &str) -> Result<Vec<String>, String> {
    let mut list = Vec::new();
    for entry in text.split_whitespace() {
        let address = if entry.contains("://") { entry.to_string() } else { super::normalize(entry)? };
        let url = Url::parse(&address).map_err(|e| e.to_string())?;
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some()
            || !url.username().is_empty() || url.password().is_some() || !super::allowed(&url)
        {
            return Err("bad_origin".into());
        }
        list.push(origin(&url).ok_or("bad_origin")?);
    }
    list.sort_unstable();
    list.dedup();
    let path = path()?;
    let parent = path.parent().ok_or("browser settings directory is missing")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(list)
}

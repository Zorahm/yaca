//! The window as a [`Host`] of the remote server: the projects, recorded chats and
//! live chats the window shows, so a turn started at the desk can be answered from
//! the phone and the other way round.
//!
//! Every method calls the code the window's commands call, so the phone and the
//! window never list different things. The one thing the window does not need is
//! the guard against opening a live chat twice: the window keeps its own list of
//! chats and checks it, the phone has no such list, so [`DesktopHost::open_chat`]
//! checks the table instead.

use async_trait::async_trait;
use tauri::{AppHandle, Manager};
use tokio::sync::{watch, Mutex};
use yaca_core::event::AgentEvent;
use yaca_core::id::SessionId;
use yaca_core::usage::ModelInfo;
use yaca_core::SessionMeta;
use yaca_remote::host::{Host, LiveChat};
use yaca_remote::protocol::Startup as RemoteStartup;

use crate::browser::Browsers;
use crate::startup::Startup;
use crate::state::AppState;
use crate::{chats, store};

pub struct DesktopHost {
    app: AppHandle,
    /// Held across a resume check and the open that follows it. Two phones asking
    /// for the same recorded chat at once must end with one live chat: two
    /// sessions on one journal would write into it twice.
    opening: Mutex<()>,
}

impl DesktopHost {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            opening: Mutex::new(()),
        }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }
}

#[async_trait]
impl Host for DesktopHost {
    fn name(&self) -> String {
        yaca_remote::host::machine_name()
    }

    fn projects(&self) -> Result<Vec<String>, String> {
        crate::projects()
    }

    fn sessions(&self, project: &str) -> Result<Vec<SessionMeta>, String> {
        Ok(store(project)?.list())
    }

    fn feed(&self, project: &str, session: &SessionId) -> Result<Vec<AgentEvent>, String> {
        Ok(store(project)?.feed(session))
    }

    fn models(&self) -> Result<Vec<ModelInfo>, String> {
        crate::providers::models()
    }

    async fn open_chat(
        &self,
        project: &str,
        resume: Option<SessionId>,
        startup: Option<RemoteStartup>,
    ) -> Result<LiveChat, String> {
        let _opening = self.opening.lock().await;
        if let Some(id) = &resume {
            // Already running: hand back the live one. Its project is the one it was
            // opened in, which is the name the phone's list already uses for it.
            if let Some(live) = self.chat(id) {
                return Ok(live);
            }
        }
        let startup = startup.map(|s| Startup {
            mode: s.mode,
            profile: s.profile,
            provider: s.provider,
            model: s.model,
            effort: s.effort,
        });
        let handle = chats::open_chat(
            &self.app,
            project.to_owned(),
            resume.map(|id| id.as_str().to_owned()),
            startup,
        )
        .await?;
        Ok(LiveChat {
            handle,
            project: project.to_owned(),
        })
    }

    fn live_chats(&self) -> Vec<LiveChat> {
        self.state()
            .live()
            .into_iter()
            .map(|(handle, project)| LiveChat { handle, project })
            .collect()
    }

    fn chat(&self, session: &SessionId) -> Option<LiveChat> {
        self.live_chats()
            .into_iter()
            .find(|chat| chat.handle.id() == session)
    }

    async fn close_chat(&self, session: &SessionId) -> Result<(), String> {
        chats::close(&self.state(), &self.app.state::<Browsers>(), session).await;
        Ok(())
    }

    fn chats_changed(&self) -> watch::Receiver<u64> {
        self.state().chats_changed()
    }
}

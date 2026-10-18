//! Shell. Deliberately thin: all the work is in `yaca-core`, here there is only the bridge
//! between the event stream and the webview.
//!
//! The only substantive thing here is merging deltas per frame (§12).
//! It lives in the bridge, not in the core: it is the transport's concern, and the CLI must
//! receive the raw stream.

mod bridge;
mod browser;
mod mcp;
mod notify;
mod prefs;
mod profile;
mod project;
mod providers;
mod routing;
mod settings;
mod state;
mod update;

use std::path::PathBuf;
use std::sync::Arc;

use yaca_core::event::{AgentEvent, SessionState};
use yaca_core::id::{SessionId, TurnId};
use yaca_core::provider::registry::{self, Providers};
use yaca_core::router::ROUTER_ID;
use yaca_core::slash::{self, SlashCommand};
use yaca_core::store::{SearchHit, SessionMeta, SessionStore};
use yaca_core::{Command, Effort, Mode, SessionConfig, SessionHandle};
use tauri::{Manager, State};

use settings::AppSettings;
use state::AppState;

/// Channel that event batches travel through. One name for everything: the interface is
/// a pure function of a single stream, and splitting channels by event type
/// would spread that function across several subscribers.
pub const EVENT_CHANNEL: &str = "yaca://events";

/// Directory of global settings.
///
/// The core computes it, not Tauri: `yaca-cli` reads the same `providers.json`,
/// and two different notions of where it lies would mean that a gateway
/// configured in the window does not answer from the CLI.
pub(crate) fn config_dir() -> Result<PathBuf, String> {
    registry::config_dir().map_err(|e| e.to_string())
}

/// The only spelling of a project path.
///
/// Всё, что приходит снаружи — из диалога папок, из списка настроек, из окна,
/// — приводится к тому же виду, в какой `Workspace::open` приводит корень
/// сессии. Иначе `D:\Kod` и `\\?\D:\Kod` были бы двумя разными проектами: у
/// одного чаты, у другого журналы.
fn project_path(path: &str) -> Result<PathBuf, String> {
    let canon = std::fs::canonicalize(path).map_err(|e| format!("не открыть {path}: {e}"))?;
    if !canon.is_dir() {
        return Err(format!("{} — не каталог", canon.display()));
    }
    Ok(canon)
}

fn listed(settings: &AppSettings) -> Vec<String> {
    settings
        .projects()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// Проекты, которые знает окно (§9.1), свежий первым.
///
/// Список переживает запуск, открытые чаты — нет: окно поднимается с пустым
/// сайдбаром разговоров, а не с попыткой воскресить вчерашние.
#[tauri::command]
fn projects() -> Result<Vec<String>, String> {
    Ok(listed(&AppSettings::load(&config_dir()?)))
}

/// Добавляет проект в список, не начиная чата (§9.1).
///
/// Домашний экран показывается до первого сообщения, а `spawn` пишет журнал
/// сессии сразу — иначе список §9 копил бы пустышку за каждый проект, в
/// котором ничего не спросили.
#[tauri::command]
fn add_project(path: String) -> Result<Vec<String>, String> {
    let path = project_path(&path)?;
    let dir = config_dir()?;
    let mut settings = AppSettings::load(&dir);
    settings.remember_project(path);
    settings.save(&dir).map_err(|e| e.to_string())?;
    Ok(listed(&settings))
}

/// Убирает проект из списка. Папку не трогает: забыть проект — действие окна,
/// а не правка диска, и журналы §9 остаются в папке чатов приложения.
#[tauri::command]
fn forget_project(path: String) -> Result<Vec<String>, String> {
    // Без `project_path`: забыть надо и тот проект, каталога которого больше
    // нет, — иначе исчезнувшая папка осталась бы в списке навсегда.
    let dir = config_dir()?;
    let mut settings = AppSettings::load(&dir);
    settings.forget_project(&PathBuf::from(path));
    settings.save(&dir).map_err(|e| e.to_string())?;
    Ok(listed(&settings))
}

/// С чем сессия начинается, когда её ещё нет.
///
/// `setMode` и соседи — команды сессии, а композер домашнего экрана (§9.1)
/// выбирает всё это до неё. Отдельный тип, а не три аргумента: они всегда
/// едут вместе и всегда означают одно — «человек выбрал до первого слова».
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Startup {
    pub mode: Mode,
    pub provider: String,
    pub model: String,
    pub effort: Effort,
}

/// Открытый чат: его идентификатор и состояние на момент открытия.
///
/// Идентификатор возвращается сразу, потому что дальше окно им и адресуется:
/// чатов много, и команда без имени сессии не знает, кому она.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OpenedChat {
    pub id: SessionId,
    pub state: SessionState,
    /// The chat's MCP servers, for the same reason as the state: they start
    /// connecting when the session does, before the window listens.
    pub mcp: Vec<yaca_core::McpServerSummary>,
    /// The chat's language servers, listed when the session opens for the
    /// same reason.
    pub lsp: Vec<yaca_core::LspServerSummary>,
}

/// A chat that kept running while the window was gone, as the reloaded window
/// picks it up again.
///
/// `events` is what the chat's feed is the fold of: the journal as written, then
/// the state that is never written down (mode and model, servers, background
/// commands, the queue, the checkpoint list). Folded, it is the feed the window
/// had before the reload, with a turn that is still running still running and a
/// question that is still open still open.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AttachedChat {
    pub id: SessionId,
    /// The project the window opened the chat in.
    pub project: String,
    /// The bridge this snapshot belongs to; frames of any other are not folded.
    pub epoch: u32,
    pub events: Vec<AgentEvent>,
}

/// С чем сессию начал бы сам проект (§10): основа для нижнего ряда §7.1 на
/// домашнем экране.
///
/// Без этого композер показывал бы встроенные умолчания и молча затирал ими
/// `.yaca/settings.json` — §10 требует обратного порядка: настройки проекта
/// основа, выбранное в окне поверх них.
///
/// The pair is settled against the provider list here, not only at the first
/// message: a project that names nothing starts on the first provider that
/// answered, and the composer should show that model rather than an empty
/// chip. None answered — the error says so, and the home screen does not send.
#[tauri::command]
fn project_startup(path: String) -> Result<Startup, String> {
    let config = SessionConfig::open(&path).map_err(|e| e.to_string())?;
    // The router is not in the provider list, it is made of it: a project that
    // names it is settled against `routing.json`, or the home screen would
    // show an error for a choice that is perfectly valid.
    if config.provider == ROUTER_ID {
        return Ok(Startup {
            mode: config.mode,
            provider: ROUTER_ID.to_string(),
            model: routing::require()?.id,
            effort: config.effort,
        });
    }
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;
    let (spec, model) = providers
        .resolve(&config.provider, &config.model)
        .map_err(|e| e.to_string())?;
    Ok(Startup {
        mode: config.mode,
        provider: spec.id.clone(),
        model,
        effort: config.effort,
    })
}

/// Открывает чат в проекте: новый или записанный (§9).
///
/// Чат становится ещё одним живым, а не заменяет предыдущий: разговоров в
/// окне столько, сколько их открыли, и ход, начатый в одном, идёт дальше,
/// пока смотрят в другой.
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    resume: Option<String>,
    startup: Option<Startup>,
) -> Result<OpenedChat, String> {
    let root = project_path(&path)?;
    let providers = Providers::load(&config_dir()?).map_err(|e| e.to_string())?;

    // Настройки проекта — основа, выбранное в окне поверх них (§10).
    let mut config = SessionConfig::open(root.as_path()).map_err(|e| e.to_string())?;
    // А это — уровень приложения, и в файле проекта его нет: ядру он приезжает
    // полем конфига, как режим и усилие.
    config.respect_robots = state.prefs().respect_robots;
    config.shell = state.prefs().shell;
    // The human's own MCP servers and their approvals of a project's: both
    // live in the configuration directory, like the providers.
    let config_home = config_dir()?;
    config.mcp_servers =
        yaca_core::mcp::config::load_global(&config_home).map_err(|e| e.to_string())?;
    // The human's own language servers, from the same directory.
    config.lsp_servers =
        yaca_core::lsp::config::load_global(&config_home).map_err(|e| e.to_string())?;
    config.lsp_autodetect = true;
    config.user_skills = yaca_core::skills::user_dirs(&config_home);
    config.user_agents = yaca_core::agents::user_dirs(&config_home);
    config.plugins = yaca_core::plugin::PluginSources::standard(&config_home).load().enabled();
    config.mcp_approvals = Some(config_home);
    config.journal = Some(yaca_core::store::chats_dir().map_err(|e| e.to_string())?);
    if let Some(id) = resume {
        config.resume_session(SessionId::from(id));
    }

    if let Some(startup) = startup {
        config.mode = startup.mode;
        config.choose_model(startup.provider, startup.model);
        config.effort = startup.effort;
    }

    // The small model the `Decision` tool asks, if there is one.
    config.decision = yaca_core::router::attach::decision_setup(&config_dir()?, &providers);

    // The chat's browser, offered to its session as an MCP server of the
    // window's own. The page itself is made on first use.
    let browser = app.state::<browser::Browsers>().prepare(&app);
    if let Some((_, server)) = &browser {
        config.app_servers.push(server.clone());
    }

    let provider = if config.provider == ROUTER_ID {
        // The same code the CLI runs: the session gets the router and a model
        // list that prices each request by the model that answered.
        yaca_core::router::attach::attach(&config_dir()?, &providers, &mut config)
            .map_err(|e| e.to_string())?
    } else {
        let (spec, model) = providers
            .resolve(&config.provider, &config.model)
            .map_err(|e| e.to_string())?;
        let spec = spec.clone();

        // No key check here: a missing key is not a reason to refuse the chat.
        // The request goes without it, and a provider that wants one answers with
        // an error naming the key (§10).

        config.provider = spec.id.clone();
        config.model = model;
        // Окно контекста и цены §7.1 считаются по моделям провайдера, а не по
        // встроенному каталогу: у чужого провайдера они свои.
        config.models = spec.models.clone();

        yaca_core::provider::build(spec)
    };
    let handle = Arc::new(yaca_core::spawn(config, provider).map_err(|e| e.to_string())?);

    // Проект, в котором открыли чат, поднимается наверх списка: сайдбар
    // читается сверху вниз, и сегодняшняя работа — сверху.
    let dir = config_dir()?;
    let mut settings = AppSettings::load(&dir);
    settings.remember_project(root);
    settings.save(&dir).map_err(|e| e.to_string())?;

    let id = handle.id().clone();
    if let Some((browser, _)) = browser {
        app.state::<browser::Browsers>().bind(id.clone(), browser);
    }
    state.open(handle.clone(), path, &app);
    // Read after the bridge is listening, so a server that becomes ready in
    // between is in one of the two: this list or the next frame.
    Ok(OpenedChat {
        id,
        state: handle.state(),
        mcp: handle.mcp(),
        lsp: handle.lsp(),
    })
}

/// Закрывает чат: ход останавливается, сессия отпускается, фоновые команды
/// умирают вместе с ней (§6.5). Журнал §9 остаётся — закрыть не значит
/// удалить, и закрытый чат открывается заново со своей историей.
#[tauri::command]
async fn close_chat(
    state: State<'_, AppState>,
    browsers: State<'_, browser::Browsers>,
    session: String,
) -> Result<(), String> {
    let id = SessionId::from(session);
    browsers.forget(&id);
    let Some(handle) = state.close(&id) else {
        return Ok(());
    };
    // Отмена до того, как ручку отпустят: иначе начатый ход доработал бы до
    // конца и только потом заметил, что его никто не ждёт.
    let _ = handle.send(Command::Cancel).await;
    Ok(())
}

/// Hands the chats that are still running back to a window that lost them.
///
/// The webview reloads — Ctrl+R, a hot reload on every edit of the frontend —
/// and the sessions do not: the turn goes on, the commands go on, a question
/// waits for its answer. Stopping them because nobody could see them was the
/// wrong trade: it threw away work to avoid showing it. The window calls this
/// on start, before it opens anything, and gets each chat as a snapshot plus a
/// bridge that carries everything after it (`AppState::rebind_all`).
#[tauri::command]
async fn attach_chats(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<AttachedChat>, String> {
    let mut attached = Vec::new();
    for chat in state.rebind_all(&app) {
        // What the window was watching went with it. Left set, the core would go
        // on sending the output of a row nobody has open.
        let _ = chat.handle.send(Command::WatchProcess { id: None }).await;

        // The journal is read from disk, and a long chat is a long file.
        let handle = chat.handle.clone();
        let replay = chat.replay;
        let events = tokio::task::spawn_blocking(move || handle.replay(&replay))
            .await
            .map_err(|e| e.to_string())?;

        attached.push(AttachedChat {
            id: chat.handle.id().clone(),
            project: chat.project,
            epoch: chat.epoch,
            events: bridge::coalesce(events),
        });
    }
    Ok(attached)
}

/// Хранилище сессий проекта (§9). Работает с проектом, а не с открытым чатом:
/// журналы лежат и у проекта, в котором сейчас никто не разговаривает.
fn store(project: &str) -> Result<SessionStore, String> {
    let chats = yaca_core::store::chats_dir().map_err(|e| e.to_string())?;
    Ok(SessionStore::open(&chats, &project_path(project)?))
}

/// Чат, которому адресована команда.
fn chat(state: &State<'_, AppState>, session: &str) -> Result<Arc<SessionHandle>, String> {
    state
        .chat(&SessionId::from(session.to_owned()))
        .ok_or_else(|| format!("чат {session} не открыт"))
}

#[tauri::command]
fn list_sessions(project: String) -> Result<Vec<SessionMeta>, String> {
    Ok(store(&project)?.list())
}

#[tauri::command]
fn search_sessions(
    project: String,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    Ok(store(&project)?.search(&query, limit.unwrap_or(50)))
}

#[tauri::command]
fn rename_session(
    state: State<'_, AppState>,
    project: String,
    id: String,
    title: String,
) -> Result<(), String> {
    let id = SessionId::from(id);
    // An open chat owns its index and writes it whole when a turn ends: a
    // name put into the file behind its back lasts until then.
    match state.chat(&id) {
        Some(chat) => chat.rename(&title),
        None => store(&project)?.rename(&id, &title),
    }
    .map_err(|e| e.to_string())
}

/// An open chat is closed first: a deleted session that kept running would go
/// on editing files with nobody watching it.
#[tauri::command]
async fn delete_session(
    state: State<'_, AppState>,
    project: String,
    id: String,
) -> Result<(), String> {
    let id = SessionId::from(id);
    if let Some(handle) = state.close(&id) {
        let _ = handle.send(Command::Cancel).await;
    }
    store(&project)?.delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn duplicate_session(
    project: String,
    id: String,
    through: Option<String>,
) -> Result<SessionMeta, String> {
    store(&project)?
        .duplicate(&SessionId::from(id), through.map(TurnId::from).as_ref())
        .map_err(|e| e.to_string())
}

/// Экспорт в Markdown (§9). Путь выбирает окно — диалог сохранения
/// принадлежит ему, — а собирает и пишет ядро: разговор, прошедший через
/// вебвью ради того, чтобы вернуться в файл, это лишняя его копия в памяти
/// страницы.
#[tauri::command]
fn export_session(project: String, id: String, path: String) -> Result<(), String> {
    let text = store(&project)?
        .export_markdown(&SessionId::from(id))
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("не записать {path}: {e}"))
}

/// Лента записанной сессии (§9) — журнал, каким его может проиграть заново
/// открытое окно. Что из него выпадает и почему — `yaca_core::store::replay`.
#[tauri::command]
fn session_feed(project: String, id: String) -> Result<Vec<AgentEvent>, String> {
    Ok(store(&project)?.feed(&SessionId::from(id)))
}

/// Слэш-команды проекта (§7.3) — то, чем меню композера дополняет ввод.
///
/// Читается по проекту, а не по чату: файлы лежат в `.yaca/commands`, и чатов
/// в одном проекте открыто может быть несколько. Разворачивает их ядро в
/// момент отправки, а не окно: что уехало модели, решает ядро.
#[tauri::command]
fn project_commands(project: String) -> Result<Vec<SlashCommand>, String> {
    let mut commands = slash::load(&project_path(&project)?);
    // After the project's: the core looks a typed name up in the same order.
    let plugins = yaca_core::plugin::PluginSources::standard(&config_dir()?).load().enabled();
    let taken: std::collections::HashSet<String> = commands.iter().map(|c| c.name.clone()).collect();
    commands.extend(
        yaca_core::plugin::commands(&plugins)
            .into_iter()
            .filter(|c| !taken.contains(&c.name)),
    );
    Ok(commands)
}

/// The skills a chat in this project sees, with what is wrong with those that
/// did not load: for the slash menu and the settings window. Scanned on every
/// call, like the commands, so a skill written a moment ago is there.
#[tauri::command]
fn project_skills(project: String) -> Result<yaca_core::skills::SkillReport, String> {
    let config = config_dir()?;
    let plugins = yaca_core::plugin::PluginSources::standard(&config).load().enabled();
    Ok(yaca_core::skills::SkillRoots::new(&project_path(&project)?, yaca_core::skills::user_dirs(&config))
        .with_plugins(&plugins)
        .scan()
        .report())
}

/// Every plugin found — Yaca's own and Claude Code's — with its switch and
/// what did not load. Read on every call, like the skills.
#[tauri::command]
fn list_plugins() -> Result<yaca_core::plugin::PluginReport, String> {
    Ok(yaca_core::plugin::PluginSources::standard(&config_dir()?).load().report())
}

/// Turns a plugin on or off for the chats opened after this; an open chat
/// keeps the plugins it started with, as it keeps its MCP servers.
#[tauri::command]
fn set_plugin_enabled(id: String, enabled: bool) -> Result<yaca_core::plugin::PluginReport, String> {
    let config = config_dir()?;
    yaca_core::plugin::set_enabled(&config, &id, enabled).map_err(|e| e.to_string())?;
    Ok(yaca_core::plugin::PluginSources::standard(&config).load().report())
}

/// Runs an installer operation off the event loop: it is git and the disk,
/// and a clone takes as long as the network does.
async fn installing<T: Send + 'static>(
    work: impl FnOnce(&yaca_core::plugin::install::Installer) -> Result<T, yaca_core::error::CoreError> + Send + 'static,
) -> Result<T, String> {
    let config = config_dir()?;
    tauri::async_runtime::spawn_blocking(move || {
        work(&yaca_core::plugin::install::Installer::standard(&config)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_marketplaces() -> Result<Vec<yaca_core::plugin::install::MarketplaceSummary>, String> {
    installing(|i| Ok(i.marketplaces())).await
}

#[tauri::command]
async fn add_marketplace(source: String) -> Result<yaca_core::plugin::install::MarketplaceSummary, String> {
    installing(move |i| i.add_marketplace(&source)).await
}

#[tauri::command]
async fn remove_marketplace(name: String) -> Result<(), String> {
    installing(move |i| i.remove_marketplace(&name)).await
}

#[tauri::command]
async fn refresh_marketplace(name: String) -> Result<yaca_core::plugin::install::MarketplaceSummary, String> {
    installing(move |i| i.refresh_marketplace(&name)).await
}

/// What a marketplace offers; fetches it the first time it is looked into.
#[tauri::command]
async fn marketplace_catalog(name: String) -> Result<Vec<yaca_core::plugin::install::CatalogEntry>, String> {
    installing(move |i| i.catalog(&name)).await
}

/// `plugin@marketplace` from the catalog. Installed plugins are on, and reach
/// the chats opened after this.
#[tauri::command]
async fn install_plugin(id: String) -> Result<yaca_core::plugin::PluginSummary, String> {
    installing(move |i| i.install(&id)).await
}

#[tauri::command]
async fn update_plugin(id: String) -> Result<yaca_core::plugin::install::UpdateOutcome, String> {
    installing(move |i| i.update(&id)).await
}

#[tauri::command]
async fn uninstall_plugin(id: String) -> Result<(), String> {
    installing(move |i| i.uninstall(&id)).await
}

/// The shells there are to choose from on this machine (§10).
///
/// Empty where there is no choice — everywhere but Windows — and the settings
/// window then draws no row for it.
#[tauri::command]
fn installed_shells() -> Vec<yaca_core::ShellChoice> {
    yaca_core::process::shell::installed()
}

/// What an attachment is, and its thumbnail if it is an image (§7.2).
///
/// The page gets pixels, not access: it cannot read the disk, and opening
/// the disk to it for thumbnails would open it to everything else on the
/// page. A path from the feed is written from the project root, so the root
/// comes along.
#[tauri::command]
async fn attachment_look(
    project: String,
    path: String,
) -> Result<yaca_core::picture::AttachmentLook, String> {
    let root = project_path(&project)?;
    tauri::async_runtime::spawn_blocking(move || yaca_core::picture::look(&root.join(path)))
        .await
        .map_err(|e| e.to_string())
}

/// The image whole, for the viewer: the preview is too small to look into.
#[tauri::command]
async fn image_full(project: String, path: String) -> Result<String, String> {
    let root = project_path(&project)?;
    tauri::async_runtime::spawn_blocking(move || {
        yaca_core::picture::original(&root.join(path)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Keeps an image pasted into the composer and returns its path from the
/// project root — the form every other attachment has (§7.2).
#[tauri::command]
async fn stash_image(project: String, data: String) -> Result<String, String> {
    let root = project_path(&project)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = yaca_core::picture::stash_encoded(&root, &data).map_err(|e| e.to_string())?;
        let shown = path.strip_prefix(&root).unwrap_or(&path);
        Ok(shown.to_string_lossy().replace('\\', "/"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn send_command(
    state: State<'_, AppState>,
    session: String,
    command: Command,
) -> Result<(), String> {
    chat(&state, &session)?
        .send(command)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn session_state(state: State<'_, AppState>, session: String) -> Option<SessionState> {
    chat(&state, &session).ok().map(|chat| chat.state())
}

/// Строка чата в списке §9 — заголовок для шапки (§3) и стоимость.
#[tauri::command]
fn session_meta(state: State<'_, AppState>, session: String) -> Option<SessionMeta> {
    chat(&state, &session).ok().and_then(|chat| chat.meta())
}

/// Имя для приветствия §9.1, взятое у операционной системы.
///
/// Учётных записей у Yaca нет, а спрашивать имя ради одной строки — плохая
/// плата за неё. Не нашлось — приветствие обходится без обращения.
#[tauri::command]
fn user_name() -> Option<String> {
    let var = if cfg!(windows) { "USERNAME" } else { "USER" };
    std::env::var(var)
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// The window's commands answer the window's own webview and nothing else.
///
/// The browser column's pages live in the same window, and Tauri lets the
/// application's commands answer any page it counts as local: a page of the
/// browser that got there must not be able to read chats or start one. The
/// capabilities file limits the plugins' commands the same way.
fn only_the_window(
    handler: impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    move |invoke| {
        if invoke.message.webview_ref().label() != "main" {
            invoke.resolver.reject("commands are for the Yaca window only");
            return true;
        }
        handler(invoke)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    // Before the builder: the webview opens its profile as the window is made.
    profile::adopt(&context.config().identifier);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .manage(browser::Browsers::default())
        // Preferences before the first window draws anything: the shell holds
        // them (§10) and the toast watcher asks about them on every event.
        .setup(|app| {
            if let Ok(dir) = config_dir() {
                app.state::<AppState>()
                    .set_prefs(AppSettings::load(&dir).prefs().clone());
            }
            update::watch(app.handle().clone());
            Ok(())
        })
        .invoke_handler(only_the_window(tauri::generate_handler![
            projects,
            add_project,
            forget_project,
            project_startup,
            open_project,
            close_chat,
            attach_chats,
            providers::providers,
            providers::save_provider,
            providers::delete_provider,
            user_name,
            send_command,
            session_state,
            session_meta,
            list_sessions,
            search_sessions,
            rename_session,
            delete_session,
            duplicate_session,
            export_session,
            session_feed,
            project_commands,
            project_skills,
            list_plugins,
            set_plugin_enabled,
            list_marketplaces,
            add_marketplace,
            remove_marketplace,
            refresh_marketplace,
            marketplace_catalog,
            install_plugin,
            update_plugin,
            uninstall_plugin,
            attachment_look,
            installed_shells,
            image_full,
            stash_image,
            providers::discover_models,
            providers::set_api_key,
            providers::forget_api_key,
            providers::api_key_source,
            providers::search_key_source,
            providers::models,
            providers::model_residency,
            routing::routing_status,
            routing::routing_text,
            routing::save_routing,
            routing::remove_routing,
            prefs::preferences,
            prefs::set_preferences,
            update::check_for_updates,
            update::update_status,
            project::project_settings,
            project::save_project_settings,
            project::project_instructions,
            project::shadow_excluded_dirs,
            mcp::global_mcp_servers,
            mcp::save_global_mcp_servers,
            mcp::project_mcp_file,
            mcp::session_mcp,
            browser::browser_supported,
            browser::browser_states,
            browser::browser_place,
            browser::browser_open,
            browser::browser_go,
            browser::browser_external,
            browser::browser_devtools,
            browser::browser_reset_viewport,
        ]))
        .run(context)
        .expect("не удалось запустить приложение");
}

//! Оболочка. Тонкая намеренно: вся работа в `yaca-core`, здесь только мост
//! между потоком событий и вебвью.
//!
//! Единственная содержательная вещь тут — склейка дельт по кадру (§12).
//! Она живёт в мосту, а не в ядре: это забота транспорта, и CLI должен
//! получать сырой поток.

mod bridge;
mod notify;
mod prefs;
mod profile;
mod project;
mod providers;
mod settings;
mod state;
mod window;

use std::path::PathBuf;
use std::sync::Arc;

use yaca_core::event::{AgentEvent, SessionState};
use yaca_core::id::{SessionId, TurnId};
use yaca_core::provider::registry::{self, Providers};
use yaca_core::slash::{self, SlashCommand};
use yaca_core::store::{SearchHit, SessionMeta, SessionStore};
use yaca_core::{Command, Effort, Mode, SessionConfig, SessionHandle};
use tauri::{Manager, State};

use settings::AppSettings;
use state::AppState;

/// Канал, в который уезжают пачки событий. Одно имя на всё: интерфейс —
/// чистая функция от одного потока, и разводить каналы по типам событий
/// значило бы размазать эту функцию по нескольким подписчикам.
pub const EVENT_CHANNEL: &str = "yaca://events";

/// Каталог глобальных настроек.
///
/// Считает его ядро, а не Tauri: `yaca-cli` читает тот же `providers.json`,
/// и два разных представления о том, где он лежит, означали бы, что шлюз,
/// настроенный в окне, из CLI не отвечает.
pub(crate) fn config_dir() -> Result<PathBuf, String> {
    registry::config_dir().map_err(|e| e.to_string())
}

/// Единственное написание пути проекта.
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
/// а не правка диска, и журналы §9 остаются лежать в `.yaca`.
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
    if let Some(id) = resume {
        config.resume_session(SessionId::from(id));
    }

    if let Some(startup) = startup {
        config.mode = startup.mode;
        config.choose_model(startup.provider, startup.model);
        config.effort = startup.effort;
    }

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

    let provider = yaca_core::provider::build(spec);
    let handle = Arc::new(yaca_core::spawn(config, provider).map_err(|e| e.to_string())?);

    // Проект, в котором открыли чат, поднимается наверх списка: сайдбар
    // читается сверху вниз, и сегодняшняя работа — сверху.
    let dir = config_dir()?;
    let mut settings = AppSettings::load(&dir);
    settings.remember_project(root);
    settings.save(&dir).map_err(|e| e.to_string())?;

    let opened = OpenedChat {
        id: handle.id().clone(),
        state: handle.state(),
    };
    state.open(handle, &app);
    Ok(opened)
}

/// Закрывает чат: ход останавливается, сессия отпускается, фоновые команды
/// умирают вместе с ней (§6.5). Журнал §9 остаётся — закрыть не значит
/// удалить, и закрытый чат открывается заново со своей историей.
#[tauri::command]
async fn close_chat(state: State<'_, AppState>, session: String) -> Result<(), String> {
    let Some(handle) = state.close(&SessionId::from(session)) else {
        return Ok(());
    };
    // Отмена до того, как ручку отпустят: иначе начатый ход доработал бы до
    // конца и только потом заметил, что его никто не ждёт.
    let _ = handle.send(Command::Cancel).await;
    Ok(())
}

/// Закрывает все чаты, о которых окно больше не помнит.
///
/// Вебвью перезагружается — в разработке на каждой правке фронтенда, — и
/// сессии от прошлой жизни окна остаются в оболочке живыми: ленты у них уже
/// нет, событий никто не ждёт, а агент продолжает работать и править файлы.
/// Окно зовёт это на старте, до того как откроет первый чат: невидимый агент
/// хуже прерванного.
#[tauri::command]
async fn close_all_chats(state: State<'_, AppState>) -> Result<(), String> {
    for handle in state.close_all() {
        let _ = handle.send(Command::Cancel).await;
    }
    Ok(())
}

/// Хранилище сессий проекта (§9). Работает с проектом, а не с открытым чатом:
/// журналы лежат и у проекта, в котором сейчас никто не разговаривает.
fn store(project: &str) -> Result<SessionStore, String> {
    Ok(SessionStore::open(project_path(project)?))
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
fn rename_session(project: String, id: String, title: String) -> Result<(), String> {
    store(&project)?
        .rename(&SessionId::from(id), &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_session(project: String, id: String) -> Result<(), String> {
    store(&project)?
        .delete(&SessionId::from(id))
        .map_err(|e| e.to_string())
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
    Ok(slash::load(&project_path(&project)?))
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    // Before the builder: the webview opens its profile as the window is made.
    profile::adopt(&context.config().identifier);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        // Preferences before the first window draws anything: the shell holds
        // them (§10) and the toast watcher asks about them on every event.
        .setup(|app| {
            if let Ok(dir) = config_dir() {
                app.state::<AppState>()
                    .set_prefs(AppSettings::load(&dir).prefs().clone());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            projects,
            add_project,
            forget_project,
            project_startup,
            open_project,
            close_chat,
            close_all_chats,
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
            attachment_look,
            image_full,
            stash_image,
            providers::discover_models,
            providers::set_api_key,
            providers::forget_api_key,
            providers::api_key_source,
            providers::search_key_source,
            providers::models,
            providers::model_residency,
            prefs::preferences,
            prefs::set_preferences,
            project::project_settings,
            project::save_project_settings,
            project::project_instructions,
            project::shadow_excluded_dirs,
            window::open_settings,
        ])
        .run(context)
        .expect("не удалось запустить приложение");
}

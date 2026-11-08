//! Commands called by the trusted browser panel.

use std::io::Write;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, State, Url};

use super::engine::{Area, Step, LOAD_LIMIT};
use super::layout::Rect;
use super::{access, allowed, layout, normalize, supported, tools, BrowserState, Browsers};

// --- the column's commands -------------------------------------------------

#[tauri::command]
pub fn browser_supported() -> bool {
    supported()
}

/// Every chat's browser, for a window that was reloaded.
#[tauri::command]
pub fn browser_states(browsers: State<'_, Browsers>) -> Vec<BrowserState> {
    browsers.all().iter().filter_map(|b| b.state()).collect()
}

/// The column shows `session`'s page at `rect`; every other page goes out of
/// sight. Both `None`: the column is closed.
#[tauri::command]
pub async fn browser_place(
    browsers: State<'_, Browsers>,
    session: Option<String>,
    rect: Option<Rect>,
) -> Result<(), String> {
    let shown = session.zip(rect);
    let _placement = browsers.placement.lock().await;
    for browser in browsers.all() {
        let mine = match (&shown, browser.session.lock().unwrap().as_ref()) {
            (Some((session, rect)), Some(id)) if id.to_string() == *session => Some(*rect),
            _ => None,
        };
        browser.show(mine).await;
    }
    Ok(())
}

/// The address field: opens the page, making it if this is the first.
///
#[tauri::command]
pub async fn browser_open(
    browsers: State<'_, Browsers>,
    session: String,
    url: String,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    let url = address(&url).map_err(|_| "bad_address")?;
    let engine = browser.engine().await?;
    engine.navigate(&url, LOAD_LIMIT).await.map(drop)
}

/// Search is an address-bar feature. The agent's navigate tool still takes URLs.
fn address(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("the address is empty".into());
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or_default();
    let explicit = input.contains("://")
        || ["about:", "data:", "file:", "javascript:"]
            .iter()
            .any(|prefix| input.to_ascii_lowercase().starts_with(prefix));
    if explicit
        || (!authority.chars().any(char::is_whitespace)
            && (authority.contains('.') || authority.starts_with('[')
                || authority == "localhost" || authority.contains(':')))
    {
        return normalize(input);
    }
    let mut search = Url::parse("https://www.google.com/search").map_err(|e| e.to_string())?;
    search.query_pairs_mut().append_pair("q", input);
    Ok(search.into())
}

#[tauri::command]
pub async fn browser_go(
    browsers: State<'_, Browsers>,
    session: String,
    step: String,
) -> Result<(), String> {
    let step = match step.as_str() {
        "back" => Step::Back,
        "forward" => Step::Forward,
        "reload" => Step::Reload,
        "stop" => Step::Stop,
        other => return Err(format!("unknown step {other}")),
    };
    let browser = browsers.get(&session)?;
    let _turn = if step == Step::Stop {
        browser.access.deny_all();
        browser.emit();
        None
    } else {
        Some(browser.calls.try_lock().map_err(|_| "browser_busy")?)
    };
    let engine = tools::open_page(&browser)?;
    engine.step(step, LOAD_LIMIT).await.map(drop)
}

/// Opens the page in the system's own browser.
#[tauri::command]
pub fn browser_external(
    app: AppHandle,
    browsers: State<'_, Browsers>,
    session: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let engine = tools::open_page(&*browsers.get(&session)?)?;
    let url = engine.location().url;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("only a web page opens in another browser".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_devtools(browsers: State<'_, Browsers>, session: String) -> Result<(), String> {
    tools::open_page(&*browsers.get(&session)?)?.open_devtools();
    Ok(())
}

#[tauri::command]
pub async fn browser_viewport(
    browsers: State<'_, Browsers>,
    session: String,
    preset: String,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    browser.force(layout::forced(Some(&preset), None, None)?).await
}

#[tauri::command]
pub async fn browser_tab(
    browsers: State<'_, Browsers>,
    session: String,
    action: String,
    id: Option<u32>,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    match action.as_str() {
        "new" => browser.new_tab()?,
        "select" | "close" => {
            let closed = {
                let mut tabs = browser.tabs.lock().unwrap();
                let index = tabs.pages.iter().position(|tab| Some(tab.id) == id).ok_or("unknown tab")?;
                if action == "select" {
                    tabs.active = index;
                    None
                } else {
                    let closed = tabs.pages.remove(index);
                    if tabs.pages.is_empty() {
                        tabs.add();
                    } else if index < tabs.active || tabs.active == tabs.pages.len() {
                        tabs.active -= 1;
                    }
                    closed.live
                }
            };
            if let Some(engine) = closed {
                engine.close();
            }
        }
        _ => return Err("unknown tab action".into()),
    }
    browser.apply().await;
    Ok(())
}

#[tauri::command]
pub fn browser_control(
    browsers: State<'_, Browsers>,
    session: String,
    mode: String,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let (paused, taken) = match mode.as_str() {
        "pause" => (true, false),
        "take" => (true, true),
        "resume" => (false, false),
        _ => return Err("unknown browser control mode".into()),
    };
    browser.paused.store(paused, Ordering::SeqCst);
    browser.taken.store(taken, Ordering::SeqCst);
    if paused {
        browser.access.deny_all();
    }
    if taken && !browser.busy.load(Ordering::SeqCst) {
        browser.focus_page();
    }
    browser.emit();
    Ok(())
}

#[tauri::command]
pub fn browser_decide(
    browsers: State<'_, Browsers>,
    session: String,
    id: u32,
    allow: bool,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    browser.access.answer(id, allow);
    browser.emit();
    Ok(())
}

#[tauri::command]
pub fn browser_origins(text: Option<String>) -> Result<Vec<String>, String> {
    match text {
        Some(text) => access::save_origins(&text),
        None => access::origins(),
    }
}

#[tauri::command]
pub fn browser_project_docs(project: String) -> Result<Vec<String>, String> {
    let root = crate::project_path(&project)?;
    Ok(["README.md", "docs/README.md", "docs/index.md", "docs/DESIGN.md", "docs/PRODUCT.md"]
        .into_iter()
        .filter(|path| root.join(path).is_file())
        .map(str::to_string)
        .collect())
}

#[tauri::command]
pub fn browser_zoom(
    browsers: State<'_, Browsers>,
    session: String,
    percent: u32,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    tools::open_page(&browser)?.zoom(percent.clamp(25, 300))
}

#[tauri::command]
pub async fn browser_screenshot(
    browsers: State<'_, Browsers>,
    session: String,
    destination: String,
) -> Result<(), String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    let shot = tools::open_page(&browser)?.screenshot(Area::Viewport).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::Path::new(&destination);
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or("missing destination folder")?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&shot.png).map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn browser_chat_context(
    browsers: State<'_, Browsers>,
    session: String,
) -> Result<String, String> {
    let browser = browsers.get(&session)?;
    let _turn = browser.calls.try_lock().map_err(|_| "browser_busy")?;
    let engine = tools::open_page(&browser)?;
    let value = engine.helper("chatContext()").await?;
    let mut url = Url::parse(value["url"].as_str().unwrap_or_default()).map_err(|e| e.to_string())?;
    if !allowed(&url) || !matches!(url.scheme(), "http" | "https" | "file") {
        return Err("there is no shareable page".into());
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    let title = value["title"].as_str().unwrap_or_default();
    let text = value["text"].as_str().unwrap_or_default().trim();
    Ok(format!("{title}\n{url}\n\n{text}").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn the_address_bar_searches_but_preserves_hosts_and_blocks_scripts() {
        assert_eq!(address("rust async").unwrap(), "https://www.google.com/search?q=rust+async");
        assert_eq!(address("localhost:5173").unwrap(), "http://localhost:5173");
        assert_eq!(address("example.org/docs").unwrap(), "https://example.org/docs");
        assert!(address("javascript:alert(1)").is_err());
    }
}


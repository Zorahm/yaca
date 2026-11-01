//! The main window can leave the desktop without stopping the running chats.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent};

const TRAY_ID: &str = "yaca";
const OPEN: &str = "tray_open";
const QUIT: &str = "tray_quit";

pub struct TrayMenu {
    open: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    // The frontend supplies translated labels as soon as preferences load.
    // Native actions stay available even if the webview fails to initialize.
    let open = MenuItem::with_id(app, OPEN, "Open Yaca", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "Close", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app.default_window_icon().ok_or_else(|| {
        tauri::Error::Io(std::io::Error::other("the application icon is missing"))
    })?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon.clone())
        .tooltip("Yaca")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            OPEN => crate::notify::raise(app),
            // AppHandle::exit runs Tauri's exit hooks, including window-state
            // persistence. Closing the window here would only hide it again.
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                crate::notify::raise(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(TrayMenu { open, quit });
    Ok(())
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != "main" || window.app_handle().tray_by_id(TRAY_ID).is_none() {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(error) = window.hide() {
            eprintln!("could not hide the main window: {error}");
        }
    }
}

#[tauri::command]
pub fn set_tray_labels(
    menu: tauri::State<'_, TrayMenu>,
    open: String,
    quit: String,
) -> Result<(), String> {
    menu.open
        .set_text(open)
        .map_err(|error| error.to_string())?;
    menu.quit.set_text(quit).map_err(|error| error.to_string())
}

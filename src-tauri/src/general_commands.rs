//! Settings › General: Windows integration (tray, launch at startup, quick-add
//! hotkey) plus preview refresh and cache-size housekeeping.

use magpie_core::Error;
use magpie_core::data::{self, AppBehaviour};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::commands::AppState;

type CmdResult<T> = Result<T, Error>;

const TRAY_ID: &str = "main";
/// Launched by Windows at sign-in: start hidden in the tray.
pub const MINIMISED_ARG: &str = "--minimised";

/// Ctrl+Alt+B ("bookmark"). Ctrl+Shift+S would take over "Save As" in other apps.
pub fn quick_add_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyB)
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn quick_add(app: &AppHandle) {
    show_main(app);
    let _ = app.emit("quick-add", ());
}

/// Global hotkey handler (registered while tray mode is on).
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, state: ShortcutState) {
    if state == ShortcutState::Pressed && *shortcut == quick_add_shortcut() {
        quick_add(app);
    }
}

/// Create the tray icon (hidden unless tray mode is on).
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Magpie", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "add", "Add bookmark…\tCtrl+Alt+B", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Magpie", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &add, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Magpie")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "add" => quick_add(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    tray.set_visible(false)?;
    Ok(())
}

/// Show or hide the tray icon and (un)register the hotkey to match settings.
pub fn apply_tray(app: &AppHandle, on: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(on);
    }
    let gs = app.global_shortcut();
    let sc = quick_add_shortcut();
    if on && !gs.is_registered(sc) {
        let _ = gs.register(sc);
    } else if !on && gs.is_registered(sc) {
        let _ = gs.unregister(sc);
    }
}

pub fn behaviour(app: &AppHandle) -> AppBehaviour {
    data::get_behaviour(&app.state::<AppState>().db()).unwrap_or_default()
}

/// Startup housekeeping: queue stale previews and trim the cache.
pub fn startup(app: &AppHandle) {
    let b = behaviour(app);
    apply_tray(app, b.minimise_to_tray);
    let state = app.state::<AppState>();
    let _ = data::queue_stale_previews(&state.db(), b.refresh_days);
    trim_cache(app);
    let minimised = std::env::args().any(|a| a == MINIMISED_ARG);
    if !(minimised && b.minimise_to_tray) {
        show_main(app);
    }
}

/// Keep the preview cache under its limit (after fetches and at startup).
pub fn trim_cache(app: &AppHandle) {
    let state = app.state::<AppState>();
    let limit = behaviour(app).cache_limit_mb as u64 * 1024 * 1024;
    let _ = data::enforce_cache_limit(&state.db(), &state.previews, limit);
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub behaviour: AppBehaviour,
    pub autostart: bool,
}

#[tauri::command]
pub async fn get_general_settings(app: AppHandle) -> CmdResult<GeneralSettings> {
    Ok(GeneralSettings { behaviour: behaviour(&app), autostart: app.autolaunch().is_enabled().unwrap_or(false) })
}

#[tauri::command]
pub async fn save_app_behaviour(app: AppHandle, state: State<'_, AppState>, behaviour: AppBehaviour) -> CmdResult<AppBehaviour> {
    let saved = data::save_behaviour(&state.db(), &behaviour)?;
    apply_tray(&app, saved.minimise_to_tray);
    trim_cache(&app);
    Ok(saved)
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<bool> {
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    r.map_err(|_| Error::Validation("Windows didn't allow changing the startup setting.".into()))?;
    Ok(al.is_enabled().unwrap_or(enabled))
}

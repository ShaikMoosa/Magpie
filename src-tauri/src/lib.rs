mod commands;

use std::sync::Mutex;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = magpie_core::db::open(&dir.join("magpie.db"))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            app.manage(AppState { db: Mutex::new(conn) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_bookmarks,
            commands::get_counts,
            commands::check_url,
            commands::create_bookmark,
            commands::update_bookmark,
            commands::set_favourite,
            commands::move_bookmarks,
            commands::trash_bookmarks,
            commands::restore_bookmarks,
            commands::purge_bookmarks,
            commands::empty_trash,
            commands::open_bookmark,
            commands::list_categories,
            commands::create_category,
            commands::update_category,
            commands::delete_category,
            commands::reorder_categories,
            commands::detect_browsers,
            commands::preview_import,
            commands::start_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod ai_commands;
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
            let db_path = dir.join("magpie.db");
            let conn = magpie_core::db::open(&db_path).map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let previews = dir.join("previews");
            std::fs::create_dir_all(&previews)?;
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
                previews,
                preview_worker: std::sync::atomic::AtomicBool::new(false),
                ai_worker: std::sync::atomic::AtomicBool::new(false),
            });
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
            commands::previews_dir,
            commands::fetch_metadata,
            commands::refresh_metadata,
            commands::get_facets,
            commands::rescan_bookmarks,
            commands::mark_safe,
            commands::list_overrides,
            commands::remove_override,
            ai_commands::get_ai_settings,
            ai_commands::save_ai_config,
            ai_commands::set_ai_key,
            ai_commands::remove_ai_key,
            ai_commands::list_ai_models,
            ai_commands::suggest_category,
            ai_commands::list_ai_suggestions,
            ai_commands::accept_ai_suggestions,
            ai_commands::reject_ai_suggestions,
            ai_commands::categorize_bookmarks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

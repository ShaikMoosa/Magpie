mod ai_commands;
mod commands;
mod data_commands;
mod general_commands;
mod private_browser;
mod security_commands;

use std::sync::Mutex;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch focuses the running window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| general_commands::show_main(app)))
        .plugin(tauri_plugin_autostart::Builder::new().args([general_commands::MINIMISED_ARG]).build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| general_commands::on_shortcut(app, shortcut, event.state()))
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("magpie.db");
            let conn = magpie_core::db::open(&db_path).map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let previews = dir.join("previews");
            std::fs::create_dir_all(&previews)?;
            let backups = dir.join("backups");
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
                previews,
                backups,
                preview_worker: std::sync::atomic::AtomicBool::new(false),
                ai_worker: std::sync::atomic::AtomicBool::new(false),
                scan_worker: std::sync::atomic::AtomicBool::new(false),
            });
            general_commands::create_tray(app.handle())?;
            general_commands::startup(app.handle());
            data_commands::start_auto_backup(app.handle());
            security_commands::scan_stale_at_startup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_bookmarks,
            commands::count_bookmarks,
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
            commands::open_private,
            commands::add_tags,
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
            data_commands::get_storage_info,
            data_commands::reveal_data_folder,
            data_commands::export_library,
            data_commands::get_backup_config,
            data_commands::save_backup_config,
            data_commands::list_backups,
            data_commands::create_backup,
            data_commands::inspect_backup,
            data_commands::restore_backup,
            data_commands::clear_preview_cache,
            data_commands::reset_app_data,
            general_commands::get_general_settings,
            general_commands::save_app_behaviour,
            general_commands::set_autostart,
            security_commands::get_security_settings,
            security_commands::save_security_config,
            security_commands::set_scan_key,
            security_commands::remove_scan_key,
            security_commands::scan_bookmarks,
        ])
        .on_window_event(|window, event| {
            // With tray mode on, closing the window keeps Magpie running in the tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && general_commands::behaviour(window.app_handle()).minimise_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

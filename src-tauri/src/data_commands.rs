//! Settings › Data & Storage: export, backups, restore, cache and reset.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use magpie_core::Error;
use magpie_core::data::{self, BackupConfig, BackupInfo, BackupKind, BackupSummary, ExportFormat, StorageInfo};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::AppState;

type CmdResult<T> = Result<T, Error>;

#[tauri::command]
pub async fn get_storage_info(state: State<'_, AppState>) -> CmdResult<StorageInfo> {
    data::storage_info(&state.db(), &state.db_path, &state.previews, &state.backups)
}

/// Show the database file in Explorer.
#[tauri::command]
pub async fn reveal_data_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    app.opener()
        .reveal_item_in_dir(&state.db_path)
        .map_err(|_| Error::Validation("Couldn't open the folder.".into()))
}

/// Write the library to `path` (chosen in the save dialog). Returns how many
/// bookmarks were exported.
#[tauri::command]
pub async fn export_library(state: State<'_, AppState>, format: ExportFormat, path: PathBuf, ids: Option<Vec<String>>) -> CmdResult<usize> {
    data::export(&state.db(), format, &path, ids.as_deref())
}

#[tauri::command]
pub async fn get_backup_config(state: State<'_, AppState>) -> CmdResult<BackupConfig> {
    data::get_backup_config(&state.db())
}

#[tauri::command]
pub async fn save_backup_config(state: State<'_, AppState>, config: BackupConfig) -> CmdResult<BackupConfig> {
    let db = state.db();
    let saved = data::save_backup_config(&db, &config)?;
    data::prune(&state.backups, saved.keep)?;
    Ok(saved)
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> CmdResult<Vec<BackupInfo>> {
    data::list_backups(&state.backups)
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> CmdResult<BackupInfo> {
    let db = state.db();
    let keep = data::get_backup_config(&db)?.keep;
    data::create_backup(&db, &state.backups, BackupKind::Manual, keep)
}

/// What a backup file contains, for the restore confirmation.
#[tauri::command]
pub async fn inspect_backup(state: State<'_, AppState>, name: Option<String>, path: Option<PathBuf>) -> CmdResult<BackupSummary> {
    data::inspect_backup(&source(&state, name, path)?)
}

/// Replace the library with a backup from the list (`name`) or a file the
/// user picked (`path`). Takes a "before restore" snapshot first.
#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, name: Option<String>, path: Option<PathBuf>) -> CmdResult<BackupSummary> {
    if state.preview_worker.load(Ordering::SeqCst) || state.ai_worker.load(Ordering::SeqCst) || state.scan_worker.load(Ordering::SeqCst) {
        return Err(Error::Validation("Wait for previews, AI organising or security scans to finish, then try again.".into()));
    }
    let src = source(&state, name, path)?;
    // Copy a picked file first so restoring never reads from a file that
    // could change underneath us (or be the live database itself).
    let staged = state.backups.join("restore-staging.tmp");
    std::fs::create_dir_all(&state.backups)?;
    std::fs::copy(&src, &staged)?;
    let mut db = state.db();
    let keep = data::get_backup_config(&db)?.keep;
    let result = data::restore(&mut db, &staged, &state.backups, keep);
    let _ = std::fs::remove_file(&staged);
    result
}

fn source(state: &AppState, name: Option<String>, path: Option<PathBuf>) -> CmdResult<PathBuf> {
    match (name, path) {
        (Some(n), _) => data::backup_path(&state.backups, &n),
        (None, Some(p)) => {
            if p.canonicalize().ok() == state.db_path.canonicalize().ok() {
                return Err(Error::Validation("That's the library Magpie is using now.".into()));
            }
            Ok(p)
        }
        (None, None) => Err(Error::Validation("Choose a backup to restore.".into())),
    }
}

/// Delete cached preview images; they're fetched again when needed.
#[tauri::command]
pub async fn clear_preview_cache(state: State<'_, AppState>) -> CmdResult<u64> {
    if state.preview_worker.load(Ordering::SeqCst) {
        return Err(Error::Validation("Previews are being fetched. Try again in a moment.".into()));
    }
    data::clear_previews(&state.db(), &state.previews)
}

/// Delete all bookmarks, categories and settings and remove stored API keys.
/// Backups are kept so this can be undone from Settings.
#[tauri::command]
pub async fn reset_app_data(state: State<'_, AppState>) -> CmdResult<()> {
    if state.preview_worker.load(Ordering::SeqCst) || state.ai_worker.load(Ordering::SeqCst) || state.scan_worker.load(Ordering::SeqCst) {
        return Err(Error::Validation("Wait for previews, AI organising or security scans to finish, then try again.".into()));
    }
    {
        let db = state.db();
        let keep = data::get_backup_config(&db)?.keep;
        data::create_backup(&db, &state.backups, BackupKind::BeforeRestore, keep)?;
        data::reset(&db)?;
        data::clear_previews(&db, &state.previews)?;
    }
    crate::ai_commands::remove_all_keys();
    crate::security_commands::remove_all_keys();
    Ok(())
}

/// Daily automatic backup, run once in the background at startup.
pub fn start_auto_backup(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (db_path, dir) = (state.db_path.clone(), state.backups.clone());
    std::thread::spawn(move || {
        if let Ok(conn) = magpie_core::db::open(&db_path) {
            let _ = data::auto_backup_if_due(&conn, &dir);
        }
    });
}

/// Snapshot before a large change such as an import (skipped when empty).
pub fn safety_backup(state: &AppState, kind: BackupKind) -> CmdResult<()> {
    let db = state.db();
    let has_data: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM bookmarks)", [], |r| r.get(0))?;
    if has_data {
        let keep = data::get_backup_config(&db)?.keep;
        data::create_backup(&db, &state.backups, kind, keep)?;
    }
    Ok(())
}

//! Tauri IPC commands. Thin wrappers over `magpie_core`; all validation
//! happens in Rust and the frontend never supplies security status.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use magpie_core::models::{
    Bookmark, BookmarkQuery, Category, Counts, DeleteCategory, NewBookmark, NewCategory, SecurityStatus,
    UpdateBookmark, UpdateCategory, UrlCheck,
};
use magpie_core::import::{BrowserProfile, ImportOptions, ImportPreview, ImportSource, ImportSummary};
use magpie_core::metadata::{Fetcher, PageMeta};
use magpie_core::{Error, bookmarks, categories, import};
use magpie_core::rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub db_path: PathBuf,
    /// Cached preview images and favicons.
    pub previews: PathBuf,
    /// Local database snapshots.
    pub backups: PathBuf,
    pub preview_worker: AtomicBool,
    pub ai_worker: AtomicBool,
    pub scan_worker: AtomicBool,
}

impl AppState {
    pub(crate) fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

type CmdResult<T> = Result<T, Error>;

// ---------- Bookmarks ----------

#[tauri::command]
pub async fn count_bookmarks(state: State<'_, AppState>, query: BookmarkQuery) -> CmdResult<i64> {
    bookmarks::count(&state.db(), &query)
}

#[tauri::command]
pub async fn list_bookmarks(state: State<'_, AppState>, query: BookmarkQuery) -> CmdResult<Vec<Bookmark>> {
    bookmarks::list(&state.db(), &query)
}

#[tauri::command]
pub async fn get_counts(state: State<'_, AppState>) -> CmdResult<Counts> {
    bookmarks::counts(&state.db())
}

#[tauri::command]
pub async fn check_url(state: State<'_, AppState>, url: String, exclude_id: Option<String>) -> CmdResult<UrlCheck> {
    bookmarks::check_url(&state.db(), &url, exclude_id.as_deref())
}

#[tauri::command]
pub async fn create_bookmark(app: AppHandle, state: State<'_, AppState>, input: NewBookmark) -> CmdResult<Bookmark> {
    let b = bookmarks::create(&state.db(), input)?;
    crate::security_commands::scan_saved(&app, Some(vec![b.id.clone()]));
    Ok(b)
}

#[tauri::command]
pub async fn update_bookmark(state: State<'_, AppState>, input: UpdateBookmark) -> CmdResult<Bookmark> {
    bookmarks::update(&state.db(), input)
}

#[tauri::command]
pub async fn set_favourite(state: State<'_, AppState>, ids: Vec<String>, value: bool) -> CmdResult<usize> {
    bookmarks::set_favourite(&state.db(), &ids, value)
}

#[tauri::command]
pub async fn move_bookmarks(
    state: State<'_, AppState>,
    ids: Vec<String>,
    category_id: Option<String>,
) -> CmdResult<usize> {
    bookmarks::move_to_category(&state.db(), &ids, category_id)
}

#[tauri::command]
pub async fn trash_bookmarks(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    bookmarks::trash(&state.db(), &ids)
}

#[tauri::command]
pub async fn restore_bookmarks(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    bookmarks::restore(&state.db(), &ids)
}

#[tauri::command]
pub async fn purge_bookmarks(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    bookmarks::purge(&state.db(), &ids)
}

#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> CmdResult<usize> {
    bookmarks::empty_trash(&state.db())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOutcome {
    /// False when the bookmark needs confirmation (suspicious) or is blocked (dangerous).
    pub opened: bool,
    pub bookmark: Bookmark,
}

/// Open a bookmark in the default browser. Suspicious and dangerous URLs
/// are not opened unless `confirmed` is true.
#[tauri::command]
pub async fn open_bookmark(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    confirmed: bool,
) -> CmdResult<OpenOutcome> {
    let bookmark = {
        let db = state.db();
        let current = bookmarks::get(&db, &id)?;
        let cfg = magpie_core::scanners::get_config(&db)?;
        let risky = match current.security_status {
            // "Block known malicious sites" off: still warn, never open silently.
            SecurityStatus::Dangerous => true,
            SecurityStatus::Suspicious => cfg.warn_suspicious,
            _ => false,
        };
        if risky && !confirmed {
            return Ok(OpenOutcome { opened: false, bookmark: current });
        }
        bookmarks::openable_url(&current)?;
        bookmarks::record_open(&db, &id)?
    };
    app.opener()
        .open_url(bookmarks::openable_url(&bookmark)?, None::<&str>)
        .map_err(|_| Error::Validation("Couldn't open your default browser.".into()))?;
    Ok(OpenOutcome { opened: true, bookmark })
}

/// Open in the default browser's private window, with the same security
/// gating as a normal open.
#[tauri::command]
pub async fn open_private(state: State<'_, AppState>, id: String, confirmed: bool) -> CmdResult<OpenOutcome> {
    let bookmark = {
        let db = state.db();
        let current = bookmarks::get(&db, &id)?;
        let cfg = magpie_core::scanners::get_config(&db)?;
        let risky = match current.security_status {
            SecurityStatus::Dangerous => true,
            SecurityStatus::Suspicious => cfg.warn_suspicious,
            _ => false,
        };
        if risky && !confirmed {
            return Ok(OpenOutcome { opened: false, bookmark: current });
        }
        current
    };
    crate::private_browser::open(&bookmarks::openable_url(&bookmark)?)?;
    let bookmark = bookmarks::record_open(&state.db(), &id)?;
    Ok(OpenOutcome { opened: true, bookmark })
}

#[tauri::command]
pub async fn add_tags(state: State<'_, AppState>, ids: Vec<String>, tags: Vec<String>) -> CmdResult<usize> {
    bookmarks::add_tags(&state.db(), &ids, &tags)
}

// ---------- Categories ----------

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> CmdResult<Vec<Category>> {
    categories::list(&state.db())
}

#[tauri::command]
pub async fn create_category(state: State<'_, AppState>, input: NewCategory) -> CmdResult<Category> {
    categories::create(&state.db(), input)
}

#[tauri::command]
pub async fn update_category(state: State<'_, AppState>, input: UpdateCategory) -> CmdResult<Category> {
    categories::update(&state.db(), input)
}

#[tauri::command]
pub async fn delete_category(state: State<'_, AppState>, input: DeleteCategory) -> CmdResult<()> {
    categories::delete(&state.db(), input)
}

#[tauri::command]
pub async fn reorder_categories(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    categories::reorder(&state.db(), &ids)
}

// ---------- Import ----------

fn local_app_data(app: &AppHandle) -> CmdResult<std::path::PathBuf> {
    app.path()
        .local_data_dir()
        .map_err(|_| Error::Validation("Couldn't find the Windows AppData folder.".into()))
}

/// Chrome and Edge profiles that have bookmarks.
#[tauri::command]
pub async fn detect_browsers(app: AppHandle) -> CmdResult<Vec<BrowserProfile>> {
    Ok(import::detect_profiles(&local_app_data(&app)?))
}

/// Read a source and report what an import would do. Nothing is written.
#[tauri::command]
pub async fn preview_import(app: AppHandle, state: State<'_, AppState>, source: ImportSource) -> CmdResult<ImportPreview> {
    let (label, _, items) = import::read_source(&source, &local_app_data(&app)?)?;
    import::preview(&state.db(), label, &items)
}

/// Import the selected folders. Browser files are only read.
#[tauri::command]
pub async fn start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    source: ImportSource,
    options: ImportOptions,
) -> CmdResult<ImportSummary> {
    let (label, browser, items) = import::read_source(&source, &local_app_data(&app)?)?;
    crate::data_commands::safety_backup(&state, magpie_core::data::BackupKind::BeforeImport)?;
    let summary = import::import(&state.db(), &label, &browser, &items, &options)?;
    crate::security_commands::scan_saved(&app, None);
    Ok(summary)
}

// ---------- Link previews ----------

/// Folder holding cached preview images (served to the UI via the asset protocol).
#[tauri::command]
pub async fn previews_dir(state: State<'_, AppState>) -> CmdResult<String> {
    Ok(state.previews.to_string_lossy().to_string())
}

/// Fetch a page's preview for the Add Bookmark dialog. Nothing is saved
/// except cached images.
#[tauri::command]
pub async fn fetch_metadata(state: State<'_, AppState>, url: String) -> CmdResult<PageMeta> {
    let dir = state.previews.clone();
    tauri::async_runtime::spawn_blocking(move || Fetcher::new(dir).fetch(&url))
        .await
        .map_err(|_| Error::Network("The preview request stopped unexpectedly.".into()))?
}

/// Queue bookmarks for a fresh preview (all pending ones when `ids` is None)
/// and make sure the background worker is running.
#[tauri::command]
pub async fn refresh_metadata(app: AppHandle, state: State<'_, AppState>, ids: Option<Vec<String>>) -> CmdResult<()> {
    if let Some(ids) = ids {
        bookmarks::reset_metadata(&state.db(), &ids)?;
    }
    start_preview_worker(&app, &state);
    Ok(())
}

const PREVIEW_BATCH: u32 = 8;

fn start_preview_worker(app: &AppHandle, state: &AppState) {
    if state.preview_worker.swap(true, Ordering::SeqCst) {
        return; // already running; it keeps going until nothing is pending
    }
    let app = app.clone();
    let db_path = state.db_path.clone();
    let dir = state.previews.clone();
    std::thread::spawn(move || {
        loop {
            let _ = run_preview_batches(&app, &db_path, &dir);
            let state = app.state::<AppState>();
            state.preview_worker.store(false, Ordering::SeqCst);
            // Re-check for work queued while we were finishing.
            let more = bookmarks::pending_metadata(&state.db(), 1).map(|v| !v.is_empty()).unwrap_or(false);
            if !more || state.preview_worker.swap(true, Ordering::SeqCst) {
                break;
            }
        }
        crate::general_commands::trim_cache(&app);
    });
}

fn run_preview_batches(app: &AppHandle, db_path: &std::path::Path, dir: &std::path::Path) -> Result<(), Error> {
    // Own connection so the UI's connection is never blocked by network I/O.
    let conn = magpie_core::db::open(db_path)?;
    let fetcher = Fetcher::new(dir);
    loop {
        let batch = bookmarks::pending_metadata(&conn, PREVIEW_BATCH)?;
        if batch.is_empty() {
            return Ok(());
        }
        let results: Vec<(String, Result<PageMeta, Error>)> = std::thread::scope(|s| {
            let handles: Vec<_> = batch.iter().map(|(id, url)| (id.clone(), s.spawn(|| fetcher.fetch(url)))).collect();
            handles
                .into_iter()
                .map(|(id, h)| (id, h.join().unwrap_or_else(|_| Err(Error::Network("Preview fetch failed.".into())))))
                .collect()
        });
        for (id, result) in results {
            match result {
                Ok(meta) => bookmarks::apply_metadata(&conn, &id, &meta)?,
                Err(_) => bookmarks::mark_metadata_failed(&conn, &id)?,
            }
        }
        let _ = app.emit("previews-updated", batch.len());
    }
}

// ---------- Filters & security ----------

#[tauri::command]
pub async fn get_facets(state: State<'_, AppState>) -> CmdResult<magpie_core::models::Facets> {
    bookmarks::facets(&state.db())
}

/// Re-run local URL checks (all bookmarks when `ids` is None).
#[tauri::command]
pub async fn rescan_bookmarks(state: State<'_, AppState>, ids: Option<Vec<String>>) -> CmdResult<magpie_core::models::RescanSummary> {
    bookmarks::rescan(&state.db(), ids.as_deref())
}

/// Report a false positive: trust the bookmarks' domains.
#[tauri::command]
pub async fn mark_safe(state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<usize> {
    bookmarks::mark_safe(&state.db(), &ids)
}

#[tauri::command]
pub async fn list_overrides(state: State<'_, AppState>) -> CmdResult<Vec<magpie_core::models::SecurityOverride>> {
    bookmarks::list_overrides(&state.db())
}

#[tauri::command]
pub async fn remove_override(state: State<'_, AppState>, domain: String) -> CmdResult<()> {
    bookmarks::remove_override(&state.db(), &domain)
}

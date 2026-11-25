//! Tauri IPC commands. Thin wrappers over `magpie_core`; all validation
//! happens in Rust and the frontend never supplies security status.

use std::sync::{Mutex, MutexGuard};

use magpie_core::models::{
    Bookmark, BookmarkQuery, Category, Counts, DeleteCategory, NewBookmark, NewCategory, SecurityStatus,
    UpdateBookmark, UpdateCategory, UrlCheck,
};
use magpie_core::{Error, bookmarks, categories};
use magpie_core::rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub db: Mutex<Connection>,
}

impl AppState {
    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

type CmdResult<T> = Result<T, Error>;

// ---------- Bookmarks ----------

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
pub async fn create_bookmark(state: State<'_, AppState>, input: NewBookmark) -> CmdResult<Bookmark> {
    bookmarks::create(&state.db(), input)
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
        let risky = matches!(current.security_status, SecurityStatus::Suspicious | SecurityStatus::Dangerous);
        if risky && !confirmed {
            return Ok(OpenOutcome { opened: false, bookmark: current });
        }
        bookmarks::record_open(&db, &id)?
    };
    app.opener()
        .open_url(&bookmark.url, None::<&str>)
        .map_err(|_| Error::Validation("Couldn't open your default browser.".into()))?;
    Ok(OpenOutcome { opened: true, bookmark })
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

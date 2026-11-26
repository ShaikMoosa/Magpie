//! Data & Storage: export, local backups, restore, storage usage and reset.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use chrono::Timelike;
use serde_json::json;

use crate::error::{Error, Result};
use crate::models::{Bookmark, BookmarkQuery, Category};
use crate::{bookmarks, categories, db};

// ---------------------------------------------------------------- export

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Html,
    Csv,
}

/// Live (not trashed) bookmarks, oldest first so re-imports keep their order.
/// With `only`, just those bookmarks.
fn live_bookmarks(conn: &Connection, only: Option<&[String]>) -> Result<Vec<Bookmark>> {
    let mut items = bookmarks::list(conn, &BookmarkQuery::default())?;
    if let Some(ids) = only {
        items.retain(|b| ids.contains(&b.id));
    }
    items.reverse();
    Ok(items)
}

/// "Parent › Child" for every category.
fn category_paths(cats: &[Category]) -> HashMap<String, String> {
    let by_id: HashMap<&str, &Category> = cats.iter().map(|c| (c.id.as_str(), c)).collect();
    cats.iter()
        .map(|c| {
            let mut parts = vec![c.name.clone()];
            let mut parent = c.parent_id.as_deref();
            // Guard against cycles from a damaged database.
            for _ in 0..16 {
                let Some(p) = parent.and_then(|id| by_id.get(id)) else { break };
                parts.push(p.name.clone());
                parent = p.parent_id.as_deref();
            }
            parts.reverse();
            (c.id.clone(), parts.join(" › "))
        })
        .collect()
}

pub fn to_json(conn: &Connection, only: Option<&[String]>) -> Result<String> {
    let cats = categories::list(conn)?;
    let items = live_bookmarks(conn, only)?;
    let doc = json!({
        "format": "magpie",
        "version": 1,
        "exportedAt": db::now(),
        "categories": cats.iter().map(|c| json!({
            "id": c.id, "name": c.name, "description": c.description, "color": c.color,
            "icon": c.icon, "parentId": c.parent_id, "sortOrder": c.sort_order,
        })).collect::<Vec<_>>(),
        "bookmarks": items.iter().map(|b| json!({
            "url": b.url, "title": b.title, "description": b.description, "categoryId": b.category_id,
            "isFavourite": b.is_favourite, "tags": b.tags, "createdAt": b.created_at, "updatedAt": b.updated_at,
            "lastOpenedAt": b.last_opened_at, "openCount": b.open_count,
            "sourceBrowser": b.source_browser, "sourceFolder": b.source_folder,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&doc).map_err(|_| Error::Validation("Couldn't build the export.".into()))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unix(ts: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(ts).map(|d| d.timestamp()).unwrap_or(0)
}

/// Netscape bookmark file, readable by Chrome, Edge, Firefox and Magpie.
pub fn to_html(conn: &Connection, only: Option<&[String]>) -> Result<String> {
    let cats = categories::list(conn)?;
    let items = live_bookmarks(conn, only)?;
    let mut by_cat: HashMap<Option<&str>, Vec<&Bookmark>> = HashMap::new();
    for b in &items {
        by_cat.entry(b.category_id.as_deref()).or_default().push(b);
    }
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<!-- This is an automatically generated file. Exported from Magpie. -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    fn links(out: &mut String, list: Option<&Vec<&Bookmark>>, indent: usize) {
        for b in list.into_iter().flatten() {
            let pad = "    ".repeat(indent);
            let tags = if b.tags.is_empty() { String::new() } else { format!(" TAGS=\"{}\"", esc(&b.tags.join(","))) };
            out.push_str(&format!("{pad}<DT><A HREF=\"{}\" ADD_DATE=\"{}\"{tags}>{}</A>\n", esc(&b.url), unix(&b.created_at), esc(&b.title)));
            if let Some(d) = b.description.as_deref().filter(|d| !d.is_empty()) {
                out.push_str(&format!("{pad}<DD>{}\n", esc(d)));
            }
        }
    }
    fn folder(out: &mut String, cats: &[Category], by_cat: &HashMap<Option<&str>, Vec<&Bookmark>>, parent: Option<&str>, indent: usize) {
        if indent > 16 {
            return;
        }
        for c in cats.iter().filter(|c| c.parent_id.as_deref() == parent) {
            let pad = "    ".repeat(indent);
            out.push_str(&format!("{pad}<DT><H3 ADD_DATE=\"{}\">{}</H3>\n{pad}<DL><p>\n", unix(&c.created_at), esc(&c.name)));
            folder(out, cats, by_cat, Some(&c.id), indent + 1);
            links(out, by_cat.get(&Some(c.id.as_str())), indent + 1);
            out.push_str(&format!("{pad}</DL><p>\n"));
        }
    }
    folder(&mut out, &cats, &by_cat, None, 1);
    links(&mut out, by_cat.get(&None), 1);
    out.push_str("</DL><p>\n");
    Ok(out)
}

/// One CSV cell. Cells that a spreadsheet would run as a formula are prefixed
/// with an apostrophe so opening the file can't execute anything.
fn cell(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
}

pub fn to_csv(conn: &Connection, only: Option<&[String]>) -> Result<String> {
    let paths = category_paths(&categories::list(conn)?);
    let mut out = String::from("\u{feff}title,url,category,tags,favourite,description,added,last_opened,open_count,security\r\n");
    for b in live_bookmarks(conn, only)? {
        let row = [
            cell(&b.title),
            cell(&b.url),
            cell(b.category_id.as_ref().and_then(|id| paths.get(id)).map(String::as_str).unwrap_or("")),
            cell(&b.tags.join("; ")),
            if b.is_favourite { "yes".into() } else { "no".into() },
            cell(b.description.as_deref().unwrap_or("")),
            cell(&b.created_at),
            cell(b.last_opened_at.as_deref().unwrap_or("")),
            b.open_count.to_string(),
            cell(serde_json::to_value(b.security_status).ok().and_then(|v| v.as_str().map(str::to_string)).as_deref().unwrap_or("")),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    Ok(out)
}

/// Write to a temporary file next to `path`, then move it into place.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = path.with_extension("magpie-tmp");
    fs::write(&tmp, contents)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Where an export may be written: a file with the format's extension, not a
/// folder, and not inside Magpie's own data folder (so the database, backups
/// and cache can't be overwritten).
pub fn check_export_path(path: &Path, format: ExportFormat, protected: &Path) -> Result<()> {
    let ext = match format {
        ExportFormat::Json => "json",
        ExportFormat::Html => "html",
        ExportFormat::Csv => "csv",
    };
    let bad = |m: &str| Err(Error::Validation(m.into()));
    if !path.is_absolute() || path.is_dir() {
        return bad("Choose a file to export to.");
    }
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext) || (ext == "html" && e.eq_ignore_ascii_case("htm"))) {
        return bad("The file name must end in the export format's extension.");
    }
    let parent = path.parent().and_then(|p| p.canonicalize().ok()).ok_or_else(|| Error::Validation("That folder doesn't exist.".into()))?;
    if let Ok(protected) = protected.canonicalize() {
        if parent.starts_with(&protected) {
            return bad("Choose a folder outside Magpie's data folder.");
        }
    }
    Ok(())
}

/// Export live bookmarks (or just `only`). Returns how many were written.
pub fn export(conn: &Connection, format: ExportFormat, path: &Path, only: Option<&[String]>) -> Result<usize> {
    let count = live_bookmarks(conn, only)?.len();
    let body = match format {
        ExportFormat::Json => to_json(conn, only)?,
        ExportFormat::Html => to_html(conn, only)?,
        ExportFormat::Csv => to_csv(conn, only)?,
    };
    write_atomic(path, body.as_bytes())?;
    Ok(count)
}

// ---------------------------------------------------------------- backups

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupKind {
    Auto,
    Manual,
    BeforeImport,
    BeforeRestore,
}

impl BackupKind {
    fn tag(self) -> &'static str {
        match self {
            BackupKind::Auto => "auto",
            BackupKind::Manual => "manual",
            BackupKind::BeforeImport => "before-import",
            BackupKind::BeforeRestore => "before-restore",
        }
    }
    fn from_tag(s: &str) -> Option<Self> {
        [BackupKind::Auto, BackupKind::Manual, BackupKind::BeforeImport, BackupKind::BeforeRestore].into_iter().find(|k| k.tag() == s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupConfig {
    pub daily: bool,
    pub keep: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self { daily: true, keep: 7 }
    }
}

const BACKUP_CONFIG_KEY: &str = "backup_config";
/// Safety snapshots (before import/restore) kept per kind.
const KEEP_SAFETY: usize = 5;

pub fn get_backup_config(conn: &Connection) -> Result<BackupConfig> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM application_settings WHERE key = ?1", [BACKUP_CONFIG_KEY], |r| r.get(0)).optional()?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub fn save_backup_config(conn: &Connection, cfg: &BackupConfig) -> Result<BackupConfig> {
    let cfg = BackupConfig { daily: cfg.daily, keep: cfg.keep.clamp(1, 60) };
    let json = serde_json::to_string(&cfg).map_err(|_| Error::Validation("Couldn't save settings.".into()))?;
    conn.execute(
        "INSERT INTO application_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![BACKUP_CONFIG_KEY, json],
    )?;
    Ok(cfg)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// File name inside the backups folder (used as the id).
    pub name: String,
    pub kind: BackupKind,
    pub created_at: String,
    pub size: u64,
}

const PREFIX: &str = "magpie-";

fn parse_name(name: &str) -> Option<(chrono::NaiveDateTime, BackupKind)> {
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(".db")?;
    // magpie-20261126-090000-auto.db
    let (stamp, tag) = rest.split_at_checked(15)?;
    let at = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()?;
    Some((at, BackupKind::from_tag(tag.strip_prefix('-')?)?))
}

/// Backups in `dir`, newest first. Unrelated files are ignored.
pub fn list_backups(dir: &Path) -> Result<Vec<BackupInfo>> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return Ok(out) };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some((at, kind)) = parse_name(&name) else { continue };
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        let created_at = at.and_local_timezone(chrono::Local).single().map(|d| d.to_rfc3339()).unwrap_or_default();
        out.push(BackupInfo { name, kind, created_at, size });
    }
    out.sort_by(|a, b| b.name[PREFIX.len()..PREFIX.len() + 15].cmp(&a.name[PREFIX.len()..PREFIX.len() + 15]));
    Ok(out)
}

/// Resolve a backup name to a path inside `dir`, refusing anything else.
pub fn backup_path(dir: &Path, name: &str) -> Result<PathBuf> {
    if parse_name(name).is_none() || name.contains(['/', '\\']) {
        return Err(Error::NotFound("Backup"));
    }
    let p = dir.join(name);
    if !p.is_file() {
        return Err(Error::NotFound("Backup"));
    }
    Ok(p)
}

/// Snapshot the live database into `dir` (consistent even while in use).
pub fn create_backup(conn: &Connection, dir: &Path, kind: BackupKind, keep: u32) -> Result<BackupInfo> {
    fs::create_dir_all(dir)?;
    let mut now = chrono::Local::now().naive_local();
    // Names sort by time, so a new backup must be newer than every existing
    // one (several in the same second would otherwise reuse a freed name).
    if let Some(newest) = list_backups(dir)?.iter().filter_map(|b| parse_name(&b.name)).map(|(at, _)| at).max() {
        if newest >= now.with_nanosecond(0).unwrap_or(now) {
            now = newest + chrono::Duration::seconds(1);
        }
    }
    let mut path;
    loop {
        path = dir.join(format!("{PREFIX}{}-{}.db", now.format("%Y%m%d-%H%M%S"), kind.tag()));
        if !path.exists() {
            break;
        }
        now += chrono::Duration::seconds(1);
    }
    let target = path.to_string_lossy().into_owned();
    conn.execute("VACUUM INTO ?1", [&target])?;
    prune(dir, keep)?;
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    list_backups(dir)?.into_iter().find(|b| b.name == name).ok_or(Error::NotFound("Backup"))
}

/// Keep the newest `keep` automatic backups and a few of each safety kind.
/// Manual backups are never removed automatically.
pub fn prune(dir: &Path, keep: u32) -> Result<()> {
    let all = list_backups(dir)?;
    for (kind, limit) in [(BackupKind::Auto, keep as usize), (BackupKind::BeforeImport, KEEP_SAFETY), (BackupKind::BeforeRestore, KEEP_SAFETY)] {
        for old in all.iter().filter(|b| b.kind == kind).skip(limit.max(1)) {
            let _ = fs::remove_file(dir.join(&old.name));
        }
    }
    Ok(())
}

/// Make the daily automatic backup if it's on and the last one is a day old.
pub fn auto_backup_if_due(conn: &Connection, dir: &Path) -> Result<Option<BackupInfo>> {
    let cfg = get_backup_config(conn)?;
    if !cfg.daily {
        return Ok(None);
    }
    let has_data: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM bookmarks)", [], |r| r.get(0))?;
    if !has_data {
        return Ok(None);
    }
    let last = list_backups(dir)?.into_iter().find(|b| b.kind == BackupKind::Auto);
    let due = match last.and_then(|b| chrono::DateTime::parse_from_rfc3339(&b.created_at).ok()) {
        Some(at) => chrono::Local::now().signed_duration_since(at) >= chrono::Duration::hours(23),
        None => true,
    };
    if !due {
        return Ok(None);
    }
    create_backup(conn, dir, BackupKind::Auto, cfg.keep).map(Some)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub bookmarks: i64,
    pub categories: i64,
}

/// Check that `path` is a Magpie database this version can open, without
/// changing it. Returns what it contains.
pub fn inspect_backup(path: &Path) -> Result<BackupSummary> {
    let bad = || Error::Validation("That file isn't a Magpie backup.".into());
    let src = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(|_| bad())?;
    let ok: String = src.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|_| bad())?;
    if ok != "ok" {
        return Err(Error::Validation("That backup is damaged and can't be restored.".into()));
    }
    let version: i64 = src.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|_| bad())?;
    if version < 1 {
        return Err(bad());
    }
    if version as usize > db::MIGRATIONS.len() {
        return Err(Error::Validation("That backup was made by a newer version of Magpie. Update Magpie first.".into()));
    }
    let count = |sql: &str| src.query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(|_| bad());
    if count("SELECT COUNT(*) FROM bookmarks WHERE url NOT LIKE 'http://%' AND url NOT LIKE 'https://%'")? > 0 {
        return Err(Error::Validation("That backup contains links that aren't web addresses, so it won't be restored.".into()));
    }
    Ok(BackupSummary {
        bookmarks: count("SELECT COUNT(*) FROM bookmarks WHERE deleted_at IS NULL")?,
        categories: count("SELECT COUNT(*) FROM categories")?,
    })
}

/// Replace the live database with `src`. A "before restore" snapshot is taken
/// first so the restore itself can be undone.
pub fn restore(conn: &mut Connection, src: &Path, backups_dir: &Path, keep: u32) -> Result<BackupSummary> {
    let summary = inspect_backup(src)?;
    create_backup(conn, backups_dir, BackupKind::BeforeRestore, keep)?;
    conn.restore(rusqlite::MAIN_DB, src, None::<fn(rusqlite::backup::Progress)>)?;
    db::upgrade(conn)?;
    Ok(summary)
}

// ---------------------------------------------------------------- windows settings

/// Behaviour that the Rust side needs to know about (tray, preview refresh,
/// cache size). Stored in `application_settings`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppBehaviour {
    pub minimise_to_tray: bool,
    /// Re-fetch previews older than this many days (0 = never).
    pub refresh_days: u32,
    /// Maximum size of the preview cache in MB (0 = no limit).
    pub cache_limit_mb: u32,
}

impl Default for AppBehaviour {
    fn default() -> Self {
        Self { minimise_to_tray: false, refresh_days: 30, cache_limit_mb: 500 }
    }
}

const BEHAVIOUR_KEY: &str = "app_behaviour";

pub fn get_behaviour(conn: &Connection) -> Result<AppBehaviour> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM application_settings WHERE key = ?1", [BEHAVIOUR_KEY], |r| r.get(0)).optional()?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub fn save_behaviour(conn: &Connection, b: &AppBehaviour) -> Result<AppBehaviour> {
    let b = AppBehaviour { refresh_days: b.refresh_days.min(365), cache_limit_mb: b.cache_limit_mb.min(10_000), ..b.clone() };
    let json = serde_json::to_string(&b).map_err(|_| Error::Validation("Couldn't save settings.".into()))?;
    conn.execute(
        "INSERT INTO application_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![BEHAVIOUR_KEY, json],
    )?;
    Ok(b)
}

/// Queue previews older than `days` for a fresh fetch. Returns how many.
pub fn queue_stale_previews(conn: &Connection, days: u32) -> Result<usize> {
    if days == 0 {
        return Ok(0);
    }
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Ok(conn.execute(
        "UPDATE bookmarks SET metadata_status = NULL
         WHERE deleted_at IS NULL AND metadata_status IS NOT NULL AND metadata_fetched_at < ?1",
        [cutoff],
    )?)
}

/// Delete the least recently written preview files until the cache fits in
/// `limit_bytes`, and forget them on the bookmarks that used them (those
/// bookmarks show their placeholder until the preview is refreshed).
pub fn enforce_cache_limit(conn: &Connection, dir: &Path, limit_bytes: u64) -> Result<u64> {
    if limit_bytes == 0 {
        return Ok(0);
    }
    let mut files: Vec<(std::time::SystemTime, u64, String)> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok().filter(|m| m.is_file())?;
                    Some((m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.file_name().to_string_lossy().into_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= limit_bytes {
        return Ok(0);
    }
    files.sort();
    let mut freed = 0;
    for (_, len, name) in files {
        if total <= limit_bytes {
            break;
        }
        if fs::remove_file(dir.join(&name)).is_ok() {
            total -= len;
            freed += len;
            conn.execute("UPDATE bookmarks SET thumbnail_path = NULL WHERE thumbnail_path = ?1", [&name])?;
            conn.execute("UPDATE bookmarks SET favicon_path = NULL WHERE favicon_path = ?1", [&name])?;
        }
    }
    Ok(freed)
}

// ---------------------------------------------------------------- storage

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub db_path: String,
    pub data_dir: String,
    pub backups_dir: String,
    pub database_bytes: u64,
    pub previews_bytes: u64,
    pub backups_bytes: u64,
    pub bookmarks: i64,
    pub categories: i64,
    pub trash: i64,
}

fn file_size(p: &Path) -> u64 {
    fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

pub fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .map(|it| it.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

pub fn storage_info(conn: &Connection, db_path: &Path, previews: &Path, backups: &Path) -> Result<StorageInfo> {
    let with = |ext: &str| {
        let mut p = db_path.as_os_str().to_owned();
        p.push(ext);
        file_size(Path::new(&p))
    };
    let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
    Ok(StorageInfo {
        db_path: db_path.to_string_lossy().into_owned(),
        data_dir: db_path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        backups_dir: backups.to_string_lossy().into_owned(),
        database_bytes: file_size(db_path) + with("-wal"),
        previews_bytes: dir_size(previews),
        backups_bytes: dir_size(backups),
        bookmarks: count("SELECT COUNT(*) FROM bookmarks WHERE deleted_at IS NULL")?,
        categories: count("SELECT COUNT(*) FROM categories")?,
        trash: count("SELECT COUNT(*) FROM bookmarks WHERE deleted_at IS NOT NULL")?,
    })
}

/// Delete cached preview images and queue every bookmark for a fresh fetch.
pub fn clear_previews(conn: &Connection, previews: &Path) -> Result<u64> {
    let freed = dir_size(previews);
    if let Ok(entries) = fs::read_dir(previews) {
        for e in entries.flatten() {
            if e.metadata().map(|m| m.is_file()).unwrap_or(false) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    conn.execute("UPDATE bookmarks SET thumbnail_path = NULL, favicon_path = NULL, metadata_status = NULL", [])?;
    Ok(freed)
}

/// Remove every bookmark, category and setting. Backups are kept.
pub fn reset(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "DELETE FROM ai_suggestions;
         DELETE FROM security_overrides;
         DELETE FROM bookmark_tags;
         DELETE FROM tags;
         DELETE FROM security_scans;
         DELETE FROM bookmark_history;
         DELETE FROM bookmarks_fts;
         DELETE FROM bookmarks;
         DELETE FROM categories;
         DELETE FROM import_history;
         DELETE FROM application_settings;",
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{NewBookmark, NewCategory};

    fn seed(conn: &Connection) {
        let design = categories::create(conn, NewCategory { name: "Design".into(), description: None, color: "#7C3AED".into(), icon: None, parent_id: None }).unwrap();
        let kits = categories::create(conn, NewCategory { name: "UI Kits".into(), description: None, color: "#7C3AED".into(), icon: None, parent_id: Some(design.id.clone()) }).unwrap();
        bookmarks::create(conn, NewBookmark { url: "https://figma.com".into(), title: Some("Figma \"kits\" & <more>".into()), category_id: Some(kits.id), tags: vec!["ui".into()], ..Default::default() }).unwrap();
        bookmarks::create(conn, NewBookmark { url: "https://example.com".into(), title: Some("=HYPERLINK(\"x\")".into()), description: Some("line1\nline2".into()), ..Default::default() }).unwrap();
        let gone = bookmarks::create(conn, NewBookmark { url: "https://trash.example".into(), ..Default::default() }).unwrap();
        bookmarks::trash(conn, &[gone.id]).unwrap();
    }

    #[test]
    fn json_export_has_live_bookmarks_and_categories() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let v: serde_json::Value = serde_json::from_str(&to_json(&conn, None).unwrap()).unwrap();
        assert_eq!(v["format"], "magpie");
        assert_eq!(v["bookmarks"].as_array().unwrap().len(), 2);
        assert_eq!(v["categories"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn html_export_round_trips_through_the_importer() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let html = to_html(&conn, None).unwrap();
        assert!(html.contains("&quot;kits&quot; &amp; &lt;more&gt;"));
        let parsed = crate::import::parse_netscape_html(&html);
        assert_eq!(parsed.len(), 2);
        let figma = parsed.iter().find(|b| b.url.contains("figma")).unwrap();
        assert_eq!(figma.folder, vec!["Design".to_string(), "UI Kits".to_string()]);
        assert_eq!(figma.title, "Figma \"kits\" & <more>");
    }

    #[test]
    fn csv_quotes_and_neutralises_formulas() {
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let csv = to_csv(&conn, None).unwrap();
        assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""));
        assert!(csv.contains("\"line1\nline2\""));
        assert!(csv.contains("Design › UI Kits"));
        assert_eq!(csv.lines().filter(|l| l.contains("https://")).count(), 2);
    }

    #[test]
    fn export_only_selected() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let first = bookmarks::list(&conn, &BookmarkQuery::default()).unwrap().remove(0);
        let path = dir.path().join("one.csv");
        assert_eq!(export(&conn, ExportFormat::Csv, &path, Some(std::slice::from_ref(&first.id))).unwrap(), 1);
        assert_eq!(export(&conn, ExportFormat::Json, &dir.path().join("all.json"), None).unwrap(), 2);
        assert_eq!(fs::read_to_string(&path).unwrap().matches("https://").count(), 1);
    }

    #[test]
    fn export_paths_are_checked() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("appdata");
        fs::create_dir_all(&data).unwrap();
        let ok = dir.path().join("out.CSV");
        assert!(check_export_path(&ok, ExportFormat::Csv, &data).is_ok());
        assert!(check_export_path(&dir.path().join("x.htm"), ExportFormat::Html, &data).is_ok());
        assert!(check_export_path(&dir.path().join("x.exe"), ExportFormat::Csv, &data).is_err());
        assert!(check_export_path(&data.join("magpie.json"), ExportFormat::Json, &data).is_err(), "can't overwrite app data");
        assert!(check_export_path(dir.path(), ExportFormat::Json, &data).is_err());
        assert!(check_export_path(Path::new("relative.json"), ExportFormat::Json, &data).is_err());
        assert!(check_export_path(&dir.path().join("missing").join("x.json"), ExportFormat::Json, &data).is_err());
    }

    #[test]
    fn backups_with_non_web_links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open(&dir.path().join("magpie.db")).unwrap();
        seed(&conn);
        conn.execute("UPDATE bookmarks SET url = 'file:///C:/Windows/System32/calc.exe' WHERE deleted_at IS NULL AND url LIKE '%example%'", []).unwrap();
        let b = create_backup(&conn, &dir.path().join("b"), BackupKind::Manual, 7).unwrap();
        let err = inspect_backup(&dir.path().join("b").join(&b.name)).unwrap_err().to_string();
        assert!(err.contains("aren't web addresses"), "{err}");
    }

    #[test]
    fn cell_escaping() {
        assert_eq!(cell("plain"), "plain");
        assert_eq!(cell("a,b"), "\"a,b\"");
        assert_eq!(cell("+1"), "'+1");
        assert_eq!(cell("@x"), "'@x");
    }

    #[test]
    fn backup_restore_and_prune() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        let mut conn = db::open(&dir.path().join("magpie.db")).unwrap();
        seed(&conn);
        let snap = create_backup(&conn, &backups, BackupKind::Manual, 7).unwrap();
        assert_eq!(snap.kind, BackupKind::Manual);
        assert_eq!(inspect_backup(&backups.join(&snap.name)).unwrap().bookmarks, 2);

        reset(&conn).unwrap();
        assert_eq!(bookmarks::counts(&conn).unwrap().all, 0);
        let summary = restore(&mut conn, &backups.join(&snap.name), &backups, 7).unwrap();
        assert_eq!(summary.bookmarks, 2);
        assert_eq!(bookmarks::counts(&conn).unwrap().all, 2);
        // Search still works after a restore.
        let q = BookmarkQuery { search: Some("figma".into()), ..Default::default() };
        assert_eq!(bookmarks::list(&conn, &q).unwrap().len(), 1);
        assert!(list_backups(&backups).unwrap().iter().any(|b| b.kind == BackupKind::BeforeRestore));

        for _ in 0..4 {
            create_backup(&conn, &backups, BackupKind::Auto, 2).unwrap();
        }
        let all = list_backups(&backups).unwrap();
        assert_eq!(all.iter().filter(|b| b.kind == BackupKind::Auto).count(), 2);
        assert_eq!(all.iter().filter(|b| b.kind == BackupKind::Manual).count(), 1);
    }

    #[test]
    fn rejects_non_backups_and_path_tricks() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.db");
        fs::write(&junk, b"not sqlite").unwrap();
        assert!(inspect_backup(&junk).is_err());
        assert!(backup_path(dir.path(), "..\\magpie.db").is_err());
        assert!(backup_path(dir.path(), "magpie-20261126-090000-auto.db").is_err());
    }

    #[test]
    fn auto_backup_runs_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open(&dir.path().join("magpie.db")).unwrap();
        let backups = dir.path().join("backups");
        assert!(auto_backup_if_due(&conn, &backups).unwrap().is_none(), "empty library: nothing to back up");
        seed(&conn);
        assert!(auto_backup_if_due(&conn, &backups).unwrap().is_some());
        assert!(auto_backup_if_due(&conn, &backups).unwrap().is_none());
        save_backup_config(&conn, &BackupConfig { daily: false, keep: 3 }).unwrap();
        assert_eq!(get_backup_config(&conn).unwrap().keep, 3);
    }

    #[test]
    fn behaviour_defaults_and_stale_previews() {
        let conn = db::open_in_memory().unwrap();
        let b = get_behaviour(&conn).unwrap();
        assert!(!b.minimise_to_tray && b.refresh_days == 30 && b.cache_limit_mb == 500);
        save_behaviour(&conn, &AppBehaviour { minimise_to_tray: true, refresh_days: 999, cache_limit_mb: 0 }).unwrap();
        assert_eq!(get_behaviour(&conn).unwrap().refresh_days, 365);

        seed(&conn);
        conn.execute("UPDATE bookmarks SET metadata_status = 'ok', metadata_fetched_at = '2020-01-01T00:00:00.000Z'", []).unwrap();
        let fresh = bookmarks::list(&conn, &BookmarkQuery::default()).unwrap()[0].id.clone();
        conn.execute("UPDATE bookmarks SET metadata_fetched_at = ?2 WHERE id = ?1", params![fresh, db::now()]).unwrap();
        assert_eq!(queue_stale_previews(&conn, 0).unwrap(), 0);
        assert_eq!(queue_stale_previews(&conn, 30).unwrap(), 1, "only the old live bookmark is queued");
    }

    #[test]
    fn cache_limit_removes_oldest_files() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        let id = bookmarks::list(&conn, &BookmarkQuery::default()).unwrap()[0].id.clone();
        fs::write(dir.path().join("old.png"), [0u8; 600]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(dir.path().join("new.png"), [0u8; 600]).unwrap();
        conn.execute("UPDATE bookmarks SET thumbnail_path = 'old.png', favicon_path = 'new.png' WHERE id = ?1", [&id]).unwrap();
        assert_eq!(enforce_cache_limit(&conn, dir.path(), 0).unwrap(), 0, "0 means no limit");
        assert_eq!(enforce_cache_limit(&conn, dir.path(), 1000).unwrap(), 600);
        assert!(!dir.path().join("old.png").exists() && dir.path().join("new.png").exists());
        let b = bookmarks::get(&conn, &id).unwrap();
        assert_eq!((b.thumbnail_path, b.favicon_path.as_deref()), (None, Some("new.png")));
    }

    #[test]
    fn clearing_previews_queues_refetch() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open_in_memory().unwrap();
        seed(&conn);
        fs::write(dir.path().join("a.png"), [0u8; 10]).unwrap();
        assert_eq!(clear_previews(&conn, dir.path()).unwrap(), 10);
        assert_eq!(dir_size(dir.path()), 0);
        assert_eq!(bookmarks::pending_metadata(&conn, 10).unwrap().len(), 2);
    }
}

//! Bookmark CRUD, search and counts.

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use crate::db::{new_id, now};
use crate::error::{Error, Result};
use crate::models::{
    Bookmark, BookmarkQuery, Counts, Facet, Facets, NewBookmark, RescanSummary, SecurityOverride, SecurityStatus, Sort,
    UpdateBookmark, UrlCheck, View,
};
use crate::{security, urls};

const REASON_SEP: char = '\u{1f}';
const TRUSTED_REASON: &str = "You marked this site as safe";
const RECENT_DAYS: i64 = 30;

const SELECT: &str = "SELECT b.id, b.url, b.title, b.description, b.domain, b.favicon_path, b.thumbnail_path,
    b.category_id, b.is_favourite, b.source_browser, b.source_folder, b.security_status, b.security_reasons,
    b.security_source, b.security_checked_at, b.created_at, b.updated_at, b.last_opened_at, b.open_count,
    b.deleted_at, b.metadata_status,
    (SELECT group_concat(t.name, char(31) ORDER BY t.name) FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
      WHERE bt.bookmark_id = b.id) AS tags
    FROM bookmarks b";

fn split(s: Option<String>) -> Vec<String> {
    s.map(|s| s.split(REASON_SEP).filter(|x| !x.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn from_row(r: &Row) -> rusqlite::Result<Bookmark> {
    let status: String = r.get(11)?;
    Ok(Bookmark {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        domain: r.get(4)?,
        favicon_path: r.get(5)?,
        thumbnail_path: r.get(6)?,
        category_id: r.get(7)?,
        is_favourite: r.get::<_, i64>(8)? != 0,
        source_browser: r.get(9)?,
        source_folder: r.get(10)?,
        security_status: SecurityStatus::parse(&status),
        security_reasons: split(r.get(12)?),
        security_source: r.get(13)?,
        security_checked_at: r.get(14)?,
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
        last_opened_at: r.get(17)?,
        open_count: r.get(18)?,
        deleted_at: r.get(19)?,
        metadata_status: r.get(20)?,
        tags: split(r.get(21)?),
    })
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The URL to hand to the browser. Stored URLs are re-validated because the
/// database can come from outside the app (a restored backup file), and
/// Windows would run `file:`, `ms-*:` and other handlers if asked to open them.
pub fn openable_url(b: &Bookmark) -> Result<String> {
    let refuse = || Error::Validation("This bookmark's address isn't a web link, so Magpie won't open it.".into());
    let lower = b.url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(refuse());
    }
    // Re-serialised by the URL parser, so what reaches Windows is a clean http(s) URL.
    Ok(urls::parse(&b.url).map_err(|_| refuse())?.url.to_string())
}

fn find_duplicate(conn: &Connection, normalized: &str, exclude: Option<&str>) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM bookmarks WHERE normalized_url = ?1 AND deleted_at IS NULL
             AND (?2 IS NULL OR id <> ?2) LIMIT 1",
            params![normalized, exclude],
            |r| r.get(0),
        )
        .optional()?)
}

fn ensure_category(conn: &Connection, id: &Option<String>) -> Result<()> {
    if let Some(id) = id {
        let exists: bool =
            conn.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?1)", [id], |r| r.get(0))?;
        if !exists {
            return Err(Error::NotFound("Category"));
        }
    }
    Ok(())
}

/// Validate a URL and report duplicates and local security findings
/// without saving anything.
pub fn check_url(conn: &Connection, input: &str, exclude: Option<&str>) -> Result<UrlCheck> {
    let parsed = urls::parse(input)?;
    let assessment = assess(conn, &parsed.url, &parsed.domain)?;
    Ok(UrlCheck {
        url: parsed.url.to_string(),
        duplicate_of: find_duplicate(conn, &parsed.normalized, exclude)?,
        normalized: parsed.normalized,
        domain: parsed.domain,
        security_status: assessment.status,
        security_reasons: assessment.reasons,
    })
}

pub fn create(conn: &Connection, input: NewBookmark) -> Result<Bookmark> {
    let tx = conn.unchecked_transaction()?;
    let id = insert(&tx, input, None)?;
    tx.commit()?;
    get(conn, &id)
}

/// Insert without opening a transaction (callers own it). `created_at`
/// overrides the timestamp, e.g. to keep a browser's original "date added".
pub(crate) fn insert(conn: &Connection, input: NewBookmark, created_at: Option<&str>) -> Result<String> {
    let parsed = urls::parse(&input.url)?;
    if !input.allow_duplicate
        && let Some(existing_id) = find_duplicate(conn, &parsed.normalized, None)?
    {
        return Err(Error::Duplicate { existing_id });
    }
    ensure_category(conn, &input.category_id)?;
    let assessment = assess(conn, &parsed.url, &parsed.domain)?;
    let id = new_id();
    let checked = now();
    let ts = created_at.map(str::to_string).unwrap_or_else(|| checked.clone());
    let title = clean(input.title).unwrap_or_else(|| parsed.domain.clone());
    let reasons: String = assessment.reasons.join(&REASON_SEP.to_string());

    let cached = |n: &Option<String>| n.clone().filter(|n| crate::metadata::is_cache_name(n));
    let (thumb, fav) = (cached(&input.thumbnail), cached(&input.favicon));
    let has_meta = thumb.is_some() || fav.is_some() || input.description.is_some();
    let tx = conn;
    tx.execute(
        "INSERT INTO bookmarks (id, url, normalized_url, title, description, domain, category_id,
            is_favourite, source_browser, source_folder, security_status, security_reasons, security_source,
            security_checked_at, created_at, updated_at, search_rowid)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15,
            (SELECT COALESCE(MAX(search_rowid), 0) + 1 FROM bookmarks))",
        params![
            id,
            parsed.url.as_str(),
            parsed.normalized,
            title,
            clean(input.description),
            parsed.domain,
            input.category_id,
            input.is_favourite as i64,
            input.source_browser,
            input.source_folder,
            assessment.status.as_str(),
            reasons,
            security::LOCAL_SOURCE,
            checked,
            ts,
        ],
    )?;
    if has_meta {
        tx.execute(
            "UPDATE bookmarks SET thumbnail_path = ?2, favicon_path = ?3, metadata_status = 'ok', metadata_fetched_at = ?4 WHERE id = ?1",
            params![id, thumb, fav, now()],
        )?;
    }
    set_tags(tx, &id, &input.tags)?;
    reindex(tx, &id)?;
    Ok(id)
}

pub fn update(conn: &Connection, input: UpdateBookmark) -> Result<Bookmark> {
    let current = get(conn, &input.id)?;
    let parsed = urls::parse(&input.url)?;
    ensure_category(conn, &input.category_id)?;
    let url_changed = parsed.url.as_str() != current.url;
    if url_changed && let Some(existing_id) = find_duplicate(conn, &parsed.normalized, Some(&input.id))? {
        return Err(Error::Duplicate { existing_id });
    }
    let title = clean(Some(input.title)).unwrap_or_else(|| parsed.domain.clone());
    let ts = now();

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE bookmarks SET url = ?2, normalized_url = ?3, domain = ?4, title = ?5, description = ?6,
            category_id = ?7, is_favourite = ?8, updated_at = ?9 WHERE id = ?1",
        params![
            input.id,
            parsed.url.as_str(),
            parsed.normalized,
            parsed.domain,
            title,
            clean(input.description),
            input.category_id,
            input.is_favourite as i64,
            ts
        ],
    )?;
    if url_changed {
        let a = assess(&tx, &parsed.url, &parsed.domain)?;
        tx.execute(
            "UPDATE bookmarks SET security_status = ?2, security_reasons = ?3, security_source = ?4,
                security_checked_at = ?5, thumbnail_path = NULL, favicon_path = NULL, metadata_status = NULL
                WHERE id = ?1",
            params![input.id, a.status.as_str(), a.reasons.join(&REASON_SEP.to_string()), security::LOCAL_SOURCE, ts],
        )?;
    }
    set_tags(&tx, &input.id, &input.tags)?;
    reindex(&tx, &input.id)?;
    tx.commit()?;
    get(conn, &input.id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Bookmark> {
    conn.query_row(&format!("{SELECT} WHERE b.id = ?1"), [id], from_row)
        .optional()?
        .ok_or(Error::NotFound("Bookmark"))
}

/// The WHERE clause (without the keyword) and its arguments for a query.
fn filters(q: &BookmarkQuery) -> Result<(String, Vec<Value>)> {
    let mut sql = String::new();
    let mut args: Vec<Value> = Vec::new();

    sql.push_str(if q.view == View::Trash { "b.deleted_at IS NOT NULL" } else { "b.deleted_at IS NULL" });

    match q.view {
        View::Favourites => sql.push_str(" AND b.is_favourite = 1"),
        View::Recent => {
            let since = (chrono::Utc::now() - chrono::Duration::days(RECENT_DAYS))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            sql.push_str(" AND b.created_at >= ?");
            args.push(since.into());
        }
        View::Uncategorized => sql.push_str(" AND b.category_id IS NULL"),
        View::Security => sql.push_str(" AND b.security_status IN ('dangerous', 'suspicious')"),
        View::Category => {
            let id = q.category_id.clone().ok_or(Error::Validation("Missing category.".into()))?;
            sql.push_str(
                " AND b.category_id IN (WITH RECURSIVE sub(id) AS (SELECT ?
                    UNION ALL SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id) SELECT id FROM sub)",
            );
            args.push(id.into());
        }
        View::All | View::Trash => {}
    }

    if q.favourites_only {
        sql.push_str(" AND b.is_favourite = 1");
    }
    if let Some(status) = q.security {
        sql.push_str(" AND b.security_status = ?");
        args.push(status.as_str().to_string().into());
    }
    for tag in q.tags.iter().filter(|t| !t.is_empty()) {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
               WHERE bt.bookmark_id = b.id AND t.name = ? COLLATE NOCASE)",
        );
        args.push(tag.trim_start_matches('#').to_string().into());
    }
    match q.source.as_deref() {
        Some("manual") => sql.push_str(" AND b.source_browser IS NULL"),
        Some(s) if !s.is_empty() => {
            sql.push_str(" AND b.source_browser = ?");
            args.push(s.to_string().into());
        }
        _ => {}
    }
    if let Some(days) = q.added_within_days {
        let since = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        sql.push_str(" AND b.created_at >= ?");
        args.push(since.into());
    }
    if let Some(domain) = q.domain.as_deref().filter(|d| !d.is_empty()) {
        sql.push_str(" AND b.domain = ?");
        args.push(domain.to_string().into());
    }
    if let Some(m) = q.search.as_deref().and_then(fts_query) {
        sql.push_str(" AND b.search_rowid IN (SELECT rowid FROM bookmarks_fts WHERE bookmarks_fts MATCH ?)");
        args.push(m.into());
    }
    Ok((sql, args))
}

/// How many bookmarks match a query (ignoring paging).
pub fn count(conn: &Connection, q: &BookmarkQuery) -> Result<i64> {
    let (filters, args) = filters(q)?;
    Ok(conn.query_row(&format!("SELECT COUNT(*) FROM bookmarks b WHERE {filters}"), params_from_iter(args), |r| r.get(0))?)
}

pub fn list(conn: &Connection, q: &BookmarkQuery) -> Result<Vec<Bookmark>> {
    let (filters, args) = filters(q)?;
    let mut sql = format!("{SELECT} WHERE {filters}");
    sql.push_str(match q.sort {
        Sort::RecentlyAdded if q.view == View::Trash => " ORDER BY b.deleted_at DESC",
        Sort::RecentlyAdded => " ORDER BY b.created_at DESC",
        Sort::RecentlyModified => " ORDER BY b.updated_at DESC",
        Sort::TitleAsc => " ORDER BY b.title COLLATE NOCASE ASC",
        Sort::TitleDesc => " ORDER BY b.title COLLATE NOCASE DESC",
        Sort::Domain => " ORDER BY b.domain ASC, b.title COLLATE NOCASE ASC",
        Sort::RecentlyOpened => " ORDER BY b.last_opened_at IS NULL, b.last_opened_at DESC",
        Sort::MostOpened => " ORDER BY b.open_count DESC, b.last_opened_at DESC",
    });
    // Stable order across pages.
    sql.push_str(", b.id");
    if let Some(limit) = q.limit {
        sql.push_str(&format!(" LIMIT {limit} OFFSET {}", q.offset.unwrap_or(0)));
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Turn free text into an FTS5 query: every word must match as a prefix.
fn fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.to_lowercase()))
        .collect();
    if terms.is_empty() { None } else { Some(terms.join(" AND ")) }
}

pub fn counts(conn: &Connection) -> Result<Counts> {
    let since = (chrono::Utc::now() - chrono::Duration::days(RECENT_DAYS))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Ok(conn.query_row(
        "SELECT
            COALESCE(SUM(deleted_at IS NULL), 0),
            COALESCE(SUM(deleted_at IS NULL AND is_favourite = 1), 0),
            COALESCE(SUM(deleted_at IS NULL AND created_at >= ?1), 0),
            COALESCE(SUM(deleted_at IS NULL AND category_id IS NULL), 0),
            COALESCE(SUM(deleted_at IS NULL AND security_status IN ('dangerous','suspicious')), 0),
            COALESCE(SUM(deleted_at IS NOT NULL), 0)
         FROM bookmarks",
        [since],
        |r| {
            Ok(Counts {
                all: r.get(0)?,
                favourites: r.get(1)?,
                recent: r.get(2)?,
                uncategorized: r.get(3)?,
                security_alerts: r.get(4)?,
                trash: r.get(5)?,
            })
        },
    )?)
}

fn for_each_id(conn: &Connection, ids: &[String], sql: &str, extra: Option<Value>) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    {
        let mut stmt = tx.prepare(sql)?;
        for id in ids {
            n += match &extra {
                Some(v) => stmt.execute(params![id, v, now()])?,
                None => stmt.execute(params![id, now()])?,
            };
        }
    }
    tx.commit()?;
    Ok(n)
}

pub fn set_favourite(conn: &Connection, ids: &[String], value: bool) -> Result<usize> {
    for_each_id(
        conn,
        ids,
        "UPDATE bookmarks SET is_favourite = ?2, updated_at = ?3 WHERE id = ?1",
        Some(Value::Integer(value as i64)),
    )
}

pub fn move_to_category(conn: &Connection, ids: &[String], category_id: Option<String>) -> Result<usize> {
    ensure_category(conn, &category_id)?;
    let n = for_each_id(
        conn,
        ids,
        "UPDATE bookmarks SET category_id = ?2, updated_at = ?3 WHERE id = ?1",
        Some(category_id.map(Value::Text).unwrap_or(Value::Null)),
    )?;
    for id in ids {
        reindex(conn, id)?;
    }
    Ok(n)
}

pub fn trash(conn: &Connection, ids: &[String]) -> Result<usize> {
    for_each_id(conn, ids, "UPDATE bookmarks SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL", None)
}

pub fn restore(conn: &Connection, ids: &[String]) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for id in ids {
        n += tx.execute(
            "UPDATE bookmarks SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
            params![id, now()],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

/// Permanently delete bookmarks that are already in Trash.
pub fn purge(conn: &Connection, ids: &[String]) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for id in ids {
        tx.execute(
            "DELETE FROM bookmarks_fts WHERE rowid = (SELECT search_rowid FROM bookmarks WHERE id = ?1 AND deleted_at IS NOT NULL)",
            [id],
        )?;
        let removed = tx.execute("DELETE FROM bookmarks WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?;
        n += removed;
    }
    tx.commit()?;
    Ok(n)
}

pub fn empty_trash(conn: &Connection) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM bookmarks_fts WHERE rowid IN (SELECT search_rowid FROM bookmarks WHERE deleted_at IS NOT NULL)",
        [],
    )?;
    let n = tx.execute("DELETE FROM bookmarks WHERE deleted_at IS NOT NULL", [])?;
    tx.commit()?;
    Ok(n)
}

/// Record that a bookmark was opened and return it (so the caller can
/// check its security status before launching the browser).
pub fn record_open(conn: &Connection, id: &str) -> Result<Bookmark> {
    let ts = now();
    let tx = conn.unchecked_transaction()?;
    let n = tx.execute(
        "UPDATE bookmarks SET last_opened_at = ?2, open_count = open_count + 1 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, ts],
    )?;
    if n == 0 {
        return Err(Error::NotFound("Bookmark"));
    }
    tx.execute(
        "INSERT INTO bookmark_history (id, bookmark_id, opened_at) VALUES (?1, ?2, ?3)",
        params![new_id(), id, ts],
    )?;
    tx.commit()?;
    get(conn, id)
}

/// Add tags to several bookmarks, keeping the tags they already have.
pub fn add_tags(conn: &Connection, ids: &[String], tags: &[String]) -> Result<usize> {
    if tags.iter().all(|t| t.trim().trim_start_matches('#').trim().is_empty()) {
        return Err(Error::Validation("Enter at least one tag.".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let ts = now();
    let mut n = 0;
    for id in ids {
        let b = get(&tx, id)?;
        let mut merged = b.tags.clone();
        merged.extend(tags.iter().cloned());
        set_tags(&tx, id, &merged)?;
        tx.execute("UPDATE bookmarks SET updated_at = ?2 WHERE id = ?1", params![id, ts])?;
        reindex(&tx, id)?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

fn set_tags(conn: &Connection, bookmark_id: &str, tags: &[String]) -> Result<()> {
    conn.execute("DELETE FROM bookmark_tags WHERE bookmark_id = ?1", [bookmark_id])?;
    for raw in tags {
        let name = raw.trim().trim_start_matches('#').trim().to_lowercase();
        if name.is_empty() || name.len() > 40 {
            continue;
        }
        let tag_id: String = match conn
            .query_row("SELECT id FROM tags WHERE name = ?1", [&name], |r| r.get(0))
            .optional()?
        {
            Some(id) => id,
            None => {
                let id = new_id();
                conn.execute("INSERT INTO tags (id, name) VALUES (?1, ?2)", params![id, name])?;
                id
            }
        };
        conn.execute(
            "INSERT OR IGNORE INTO bookmark_tags (bookmark_id, tag_id) VALUES (?1, ?2)",
            params![bookmark_id, tag_id],
        )?;
    }
    conn.execute("DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM bookmark_tags)", [])?;
    Ok(())
}

/// Refresh the full-text index row for one bookmark.
pub fn reindex(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM bookmarks_fts WHERE rowid = (SELECT search_rowid FROM bookmarks WHERE id = ?1)", [id])?;
    conn.execute(
        "INSERT INTO bookmarks_fts (rowid, bookmark_id, title, description, url, domain, category, tags)
         SELECT b.search_rowid, b.id, b.title, COALESCE(b.description, ''), b.url, b.domain,
            COALESCE((SELECT name FROM categories c WHERE c.id = b.category_id), ''),
            COALESCE((SELECT group_concat(t.name, ' ') FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                      WHERE bt.bookmark_id = b.id), '')
         FROM bookmarks b WHERE b.id = ?1",
        [id],
    )?;
    Ok(())
}

/// Local heuristics, unless the user marked this domain as safe.
fn assess(conn: &Connection, url: &url::Url, domain: &str) -> Result<security::Assessment> {
    let overridden: bool =
        conn.query_row("SELECT EXISTS(SELECT 1 FROM security_overrides WHERE domain = ?1)", [domain], |r| r.get(0))?;
    if overridden {
        return Ok(security::Assessment {
            status: SecurityStatus::NotChecked,
            reasons: vec![TRUSTED_REASON.into()],
        });
    }
    Ok(security::assess(url))
}

/// Re-run local checks for the given bookmarks (all live bookmarks when `None`).
pub fn rescan(conn: &Connection, ids: Option<&[String]>) -> Result<RescanSummary> {
    let wanted: Option<std::collections::HashSet<&str>> = ids.map(|ids| ids.iter().map(String::as_str).collect());
    let targets: Vec<(String, String, String, String, String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, url, domain, security_status, COALESCE(security_reasons, ''), security_source FROM bookmarks WHERE deleted_at IS NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|t| wanted.as_ref().is_none_or(|w| w.contains(t.0.as_str())))
            .collect()
    };
    let tx = conn.unchecked_transaction()?;
    // Skip per-bookmark history lookups when no provider has ever answered.
    let any_external: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM security_scans WHERE provider IN ('google','urlhaus','virustotal'))",
        [],
        |r| r.get(0),
    )?;
    let mut summary = RescanSummary::default();
    let ts = now();
    for (id, url, domain, old_status, old_reasons, old_source) in targets {
        let Ok(parsed) = url::Url::parse(&url) else { continue };
        let a = assess(&tx, &parsed, &domain)?;
        // Keep the latest external verdicts (unless the user trusts this site).
        let trusted = a.reasons.iter().any(|r| r == TRUSTED_REASON);
        let external = if trusted || !any_external { Vec::new() } else { crate::scanners::latest_results(&tx, &id)? };
        let combined = crate::scanners::combine(&a, &external);
        let reasons = combined.reasons.join(&REASON_SEP.to_string());
        let unchanged = old_status == combined.status.as_str() && old_reasons == reasons && old_source.as_deref() == Some(combined.source.as_str());
        if unchanged {
            // Same result: just record when it was checked, without growing the history.
            tx.execute("UPDATE bookmarks SET security_checked_at = ?2 WHERE id = ?1", params![id, ts])?;
        } else {
            tx.execute(
                "UPDATE bookmarks SET security_status = ?2, security_reasons = ?3, security_source = ?4, security_checked_at = ?5
                 WHERE id = ?1",
                params![id, combined.status.as_str(), reasons, combined.source, ts],
            )?;
            tx.execute(
                "INSERT INTO security_scans (id, bookmark_id, provider, status, details, scanned_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![new_id(), id, security::LOCAL_SOURCE, a.status.as_str(), a.reasons.join("; "), ts],
            )?;
        }
        summary.scanned += 1;
        if matches!(combined.status, SecurityStatus::Suspicious | SecurityStatus::Dangerous) {
            summary.suspicious += 1;
        }
    }
    tx.commit()?;
    Ok(summary)
}

/// "Report false positive": trust these bookmarks' domains and re-check them.
pub fn mark_safe(conn: &Connection, ids: &[String]) -> Result<usize> {
    let ts = now();
    for id in ids {
        let b = get(conn, id)?;
        conn.execute(
            "INSERT OR IGNORE INTO security_overrides (domain, created_at) VALUES (?1, ?2)",
            params![b.domain, ts],
        )?;
    }
    let domains: Vec<String> = ids.iter().filter_map(|id| get(conn, id).ok().map(|b| b.domain)).collect();
    let affected: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id, domain FROM bookmarks WHERE deleted_at IS NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().filter(|(_, d)| domains.contains(d)).map(|(id, _)| id).collect()
    };
    rescan(conn, Some(&affected))?;
    Ok(affected.len())
}

pub fn list_overrides(conn: &Connection) -> Result<Vec<SecurityOverride>> {
    let mut stmt = conn.prepare("SELECT domain, created_at FROM security_overrides ORDER BY domain")?;
    let rows = stmt.query_map([], |r| Ok(SecurityOverride { domain: r.get(0)?, created_at: r.get(1)? }))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Stop trusting a domain and re-check its bookmarks.
pub fn remove_override(conn: &Connection, domain: &str) -> Result<()> {
    conn.execute("DELETE FROM security_overrides WHERE domain = ?1", [domain])?;
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM bookmarks WHERE domain = ?1 AND deleted_at IS NULL")?;
        stmt.query_map([domain], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
    };
    rescan(conn, Some(&ids))?;
    Ok(())
}

/// Values for the filter menus (live bookmarks only).
pub fn facets(conn: &Connection) -> Result<Facets> {
    let q = |sql: &str| -> Result<Vec<Facet>> {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| Ok(Facet { value: r.get(0)?, count: r.get(1)? }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    };
    Ok(Facets {
        domains: q("SELECT domain, COUNT(*) c FROM bookmarks WHERE deleted_at IS NULL GROUP BY domain ORDER BY c DESC, domain LIMIT 200")?,
        tags: q("SELECT t.name, COUNT(*) c FROM tags t JOIN bookmark_tags bt ON bt.tag_id = t.id
                 JOIN bookmarks b ON b.id = bt.bookmark_id AND b.deleted_at IS NULL GROUP BY t.name ORDER BY c DESC, t.name")?,
        sources: q("SELECT COALESCE(source_browser, 'manual'), COUNT(*) c FROM bookmarks WHERE deleted_at IS NULL
                    GROUP BY 1 ORDER BY c DESC")?,
        security: q("SELECT security_status, COUNT(*) FROM bookmarks WHERE deleted_at IS NULL GROUP BY 1")?,
    })
}

/// Bookmarks whose preview hasn't been fetched yet: (id, url).
pub fn pending_metadata(conn: &Connection, limit: u32) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, url FROM bookmarks WHERE metadata_status IS NULL AND deleted_at IS NULL
         ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Queue bookmarks for a fresh preview fetch.
pub fn reset_metadata(conn: &Connection, ids: &[String]) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for id in ids {
        n += tx.execute("UPDATE bookmarks SET metadata_status = NULL WHERE id = ?1", [id])?;
    }
    tx.commit()?;
    Ok(n)
}

/// Store fetched preview data. Fills title/description only when they are
/// still the defaults (title = domain, no description) so user edits win.
pub fn apply_metadata(conn: &Connection, id: &str, meta: &crate::metadata::PageMeta) -> Result<()> {
    let current = get(conn, id)?;
    let title = match &meta.title {
        Some(t) if current.title == current.domain || current.title.trim().is_empty() => t.clone(),
        _ => current.title.clone(),
    };
    let description = current.description.clone().or_else(|| meta.description.clone());
    conn.execute(
        "UPDATE bookmarks SET title = ?2, description = ?3,
            thumbnail_path = COALESCE(?4, thumbnail_path), favicon_path = COALESCE(?5, favicon_path),
            metadata_status = 'ok', metadata_fetched_at = ?6 WHERE id = ?1",
        params![id, title, description, meta.image, meta.favicon, now()],
    )?;
    reindex(conn, id)
}

pub fn mark_metadata_failed(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE bookmarks SET metadata_status = 'failed', metadata_fetched_at = ?2 WHERE id = ?1",
        params![id, now()],
    )?;
    Ok(())
}

pub(crate) fn reindex_category(conn: &Connection, category_id: &str) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM bookmarks WHERE category_id = ?1")?;
        stmt.query_map([category_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
    };
    for id in ids {
        reindex(conn, &id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn only_web_links_are_opened() {
        let c = crate::db::open_in_memory().unwrap();
        let b = create(&c, NewBookmark { url: "https://example.com/a?b=1".into(), ..Default::default() }).unwrap();
        assert_eq!(openable_url(&b).unwrap(), "https://example.com/a?b=1");
        // A crafted database (e.g. a restored backup) can contain anything.
        for evil in ["file:///C:/Windows/System32/calc.exe", "ms-msdt:/id PCWDiagnostic", "javascript:alert(1)", "\\\\server\\share\\x.exe", "C:\\x.exe", "https ://x"] {
            c.execute("UPDATE bookmarks SET url = ?2 WHERE id = ?1", params![b.id, evil]).unwrap();
            assert!(openable_url(&get(&c, &b.id).unwrap()).is_err(), "{evil}");
        }
    }

    #[test]
    fn search_index_stays_in_step() {
        let c = crate::db::open_in_memory().unwrap();
        let search = |q: &str, view: View| list(&c, &BookmarkQuery { search: Some(q.into()), view, ..Default::default() }).unwrap().len();
        let fts_rows = || c.query_row("SELECT COUNT(*) FROM bookmarks_fts", [], |r| r.get::<_, i64>(0)).unwrap();
        let a = create(&c, NewBookmark { url: "https://a.example".into(), title: Some("Alpha guide".into()), ..Default::default() }).unwrap();
        let b = create(&c, NewBookmark { url: "https://b.example".into(), title: Some("Beta notes".into()), ..Default::default() }).unwrap();
        assert_eq!((search("alpha", View::All), fts_rows()), (1, 2));

        update(&c, UpdateBookmark { id: a.id.clone(), url: a.url.clone(), title: "Gamma guide".into(), description: None, category_id: None, is_favourite: false, tags: vec![] }).unwrap();
        assert_eq!((search("alpha", View::All), search("gamma", View::All), fts_rows()), (0, 1, 2), "update replaces, never duplicates");

        trash(&c, std::slice::from_ref(&a.id)).unwrap();
        assert_eq!((search("gamma", View::All), search("gamma", View::Trash)), (0, 1));
        restore(&c, std::slice::from_ref(&a.id)).unwrap();
        assert_eq!(search("gamma", View::All), 1);

        trash(&c, &[a.id.clone(), b.id.clone()]).unwrap();
        purge(&c, std::slice::from_ref(&a.id)).unwrap();
        assert_eq!(fts_rows(), 1);
        empty_trash(&c).unwrap();
        assert_eq!(fts_rows(), 0);

        // New bookmarks keep getting fresh index rows after deletions.
        create(&c, NewBookmark { url: "https://c.example".into(), title: Some("Delta".into()), ..Default::default() }).unwrap();
        assert_eq!((search("delta", View::All), fts_rows()), (1, 1));
    }

    #[test]
    fn paging_is_stable() {
        let c = crate::db::open_in_memory().unwrap();
        for i in 0..25 {
            insert(&c, NewBookmark { url: format!("https://p{i}.example"), ..Default::default() }, Some("2026-01-01T00:00:00.000Z")).unwrap();
        }
        let page = |o| list(&c, &BookmarkQuery { limit: Some(10), offset: Some(o), ..Default::default() }).unwrap();
        let mut ids: Vec<String> = [0, 10, 20].into_iter().flat_map(|o| page(o).into_iter().map(|b| b.id)).collect();
        assert_eq!(ids.len(), 25);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 25, "same timestamps never repeat or skip across pages");
        assert_eq!(count(&c, &BookmarkQuery { limit: Some(10), ..Default::default() }).unwrap(), 25);
        assert_eq!(count(&c, &BookmarkQuery { search: Some("p3".into()), ..Default::default() }).unwrap(), 1);
    }

    #[test]
    fn add_tags_merges() {
        let c = crate::db::open_in_memory().unwrap();
        let a = create(&c, NewBookmark { url: "https://a.example".into(), tags: vec!["keep".into()], ..Default::default() }).unwrap();
        let b = create(&c, NewBookmark { url: "https://b.example".into(), ..Default::default() }).unwrap();
        assert_eq!(add_tags(&c, &[a.id.clone(), b.id.clone()], &["#New".into(), "keep".into()]).unwrap(), 2);
        assert_eq!(get(&c, &a.id).unwrap().tags, vec!["keep", "new"]);
        assert_eq!(get(&c, &b.id).unwrap().tags, vec!["keep", "new"]);
        assert!(add_tags(&c, std::slice::from_ref(&a.id), &[" # ".into()]).is_err());
        let q = BookmarkQuery { search: Some("new".into()), ..Default::default() };
        assert_eq!(list(&c, &q).unwrap().len(), 2, "search index sees new tags");
    }
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::NewCategory;

    fn add(conn: &Connection, url: &str) -> Bookmark {
        create(conn, NewBookmark { url: url.into(), ..Default::default() }).unwrap()
    }

    #[test]
    fn create_defaults_title_and_checks_security() {
        let c = open_in_memory().unwrap();
        let b = add(&c, "figma.com/community");
        assert_eq!(b.title, "figma.com");
        assert_eq!(b.url, "https://figma.com/community");
        assert_eq!(b.security_status, SecurityStatus::NotChecked);
        let s = add(&c, "https://paypa1-secure-login.com/verify");
        assert_eq!(s.security_status, SecurityStatus::Suspicious);
        assert!(!s.security_reasons.is_empty());
    }

    #[test]
    fn duplicates_are_rejected_unless_allowed() {
        let c = open_in_memory().unwrap();
        let first = add(&c, "https://example.com/");
        let err = create(&c, NewBookmark { url: "http://www.example.com".into(), ..Default::default() }).unwrap_err();
        match err {
            Error::Duplicate { existing_id } => assert_eq!(existing_id, first.id),
            other => panic!("unexpected {other:?}"),
        }
        create(&c, NewBookmark { url: "example.com".into(), allow_duplicate: true, ..Default::default() }).unwrap();
        // Trashed bookmarks don't count as duplicates.
        trash(&c, &[first.id]).unwrap();
        assert!(check_url(&c, "https://example.com/x", None).unwrap().duplicate_of.is_none());
    }

    #[test]
    fn trash_restore_purge() {
        let c = open_in_memory().unwrap();
        let b = add(&c, "https://a.com");
        trash(&c, &[b.id.clone()]).unwrap();
        assert_eq!(counts(&c).unwrap().trash, 1);
        assert_eq!(list(&c, &BookmarkQuery::default()).unwrap().len(), 0);
        restore(&c, &[b.id.clone()]).unwrap();
        assert_eq!(counts(&c).unwrap().all, 1);
        // Purge only affects trashed items.
        assert_eq!(purge(&c, &[b.id.clone()]).unwrap(), 0);
        trash(&c, &[b.id.clone()]).unwrap();
        assert_eq!(purge(&c, &[b.id.clone()]).unwrap(), 1);
        assert!(get(&c, &b.id).is_err());
    }

    #[test]
    fn search_is_prefix_and_case_insensitive_across_fields() {
        let c = open_in_memory().unwrap();
        let cat = crate::categories::create(
            &c,
            NewCategory { name: "Design".into(), description: None, color: "#7C3AED".into(), icon: None, parent_id: None },
        )
        .unwrap();
        create(
            &c,
            NewBookmark {
                url: "https://lucide.dev".into(),
                title: Some("Lucide Icons".into()),
                category_id: Some(cat.id.clone()),
                tags: vec!["#Icons".into(), "svg".into()],
                ..Default::default()
            },
        )
        .unwrap();
        add(&c, "https://react.dev");
        let q = |s: &str| list(&c, &BookmarkQuery { search: Some(s.into()), ..Default::default() }).unwrap().len();
        assert_eq!(q("luc"), 1);
        assert_eq!(q("ICONS"), 1);
        assert_eq!(q("design"), 1);
        assert_eq!(q("svg"), 1);
        assert_eq!(q("react"), 1);
        assert_eq!(q("nothing-here"), 0);
        assert_eq!(q("  "), 2);
    }

    #[test]
    fn views_and_counts() {
        let c = open_in_memory().unwrap();
        let a = add(&c, "https://a.com");
        add(&c, "https://b.com");
        set_favourite(&c, &[a.id.clone()], true).unwrap();
        let counts = counts(&c).unwrap();
        assert_eq!((counts.all, counts.favourites, counts.recent, counts.uncategorized), (2, 1, 2, 2));
        let fav = list(&c, &BookmarkQuery { view: View::Favourites, ..Default::default() }).unwrap();
        assert_eq!(fav.len(), 1);
        let opened = record_open(&c, &a.id).unwrap();
        assert_eq!(opened.open_count, 1);
    }

    #[test]
    fn metadata_fills_defaults_but_keeps_user_text() {
        let c = open_in_memory().unwrap();
        let a = add(&c, "https://react.dev");
        let b = create(&c, NewBookmark { url: "https://lucide.dev".into(), title: Some("My icons".into()), ..Default::default() }).unwrap();
        assert_eq!(pending_metadata(&c, 10).unwrap().len(), 2);
        let meta = crate::metadata::PageMeta {
            title: Some("React".into()),
            description: Some("Docs".into()),
            image: Some(format!("{}.png", "a".repeat(64))),
            ..Default::default()
        };
        apply_metadata(&c, &a.id, &meta).unwrap();
        apply_metadata(&c, &b.id, &meta).unwrap();
        let a = get(&c, &a.id).unwrap();
        let b = get(&c, &b.id).unwrap();
        assert_eq!((a.title.as_str(), a.description.as_deref()), ("React", Some("Docs")));
        assert_eq!(b.title, "My icons", "user titles are kept");
        assert_eq!(a.metadata_status.as_deref(), Some("ok"));
        assert!(pending_metadata(&c, 10).unwrap().is_empty());
        reset_metadata(&c, &[a.id.clone()]).unwrap();
        assert_eq!(pending_metadata(&c, 10).unwrap().len(), 1);
        // Searching finds fetched text.
        assert_eq!(list(&c, &BookmarkQuery { search: Some("docs".into()), ..Default::default() }).unwrap().len(), 2, "both had no description, so both got it");
    }

    #[test]
    fn create_only_accepts_cache_file_names() {
        let c = open_in_memory().unwrap();
        let b = create(
            &c,
            NewBookmark { url: "https://a.com".into(), thumbnail: Some(r"..\..\secret.png".into()), favicon: Some(format!("{}.ico", "b".repeat(64))), ..Default::default() },
        )
        .unwrap();
        assert_eq!(b.thumbnail_path, None);
        assert!(b.favicon_path.is_some());
        assert_eq!(b.metadata_status.as_deref(), Some("ok"));
    }

    #[test]
    fn filters_by_tag_source_and_date() {
        let c = open_in_memory().unwrap();
        create(&c, NewBookmark { url: "https://a.com".into(), tags: vec!["design".into(), "ui".into()], ..Default::default() }).unwrap();
        create(&c, NewBookmark { url: "https://b.com".into(), tags: vec!["design".into()], source_browser: Some("Chrome".into()), ..Default::default() }).unwrap();
        let n = |q: BookmarkQuery| list(&c, &q).unwrap().len();
        assert_eq!(n(BookmarkQuery { tags: vec!["#Design".into()], ..Default::default() }), 2);
        assert_eq!(n(BookmarkQuery { tags: vec!["design".into(), "ui".into()], ..Default::default() }), 1);
        assert_eq!(n(BookmarkQuery { source: Some("Chrome".into()), ..Default::default() }), 1);
        assert_eq!(n(BookmarkQuery { source: Some("manual".into()), ..Default::default() }), 1);
        assert_eq!(n(BookmarkQuery { added_within_days: Some(7), ..Default::default() }), 2);
        let f = facets(&c).unwrap();
        assert_eq!(f.tags[0].value, "design");
        assert_eq!(f.tags[0].count, 2);
        assert_eq!(f.sources.len(), 2);
    }

    #[test]
    fn mark_safe_overrides_and_can_be_undone() {
        let c = open_in_memory().unwrap();
        let s = add(&c, "https://paypa1-secure-login.com/verify");
        let s2 = add(&c, "https://paypa1-secure-login.com/other");
        assert_eq!(s.security_status, SecurityStatus::Suspicious);
        assert_eq!(mark_safe(&c, &[s.id.clone()]).unwrap(), 2, "applies to every bookmark on the domain");
        let after = get(&c, &s2.id).unwrap();
        assert_eq!(after.security_status, SecurityStatus::NotChecked);
        assert_eq!(after.security_reasons, vec!["You marked this site as safe"]);
        // New bookmarks on the domain respect the override too.
        assert_eq!(check_url(&c, "https://paypa1-secure-login.com/x", None).unwrap().security_status, SecurityStatus::NotChecked);
        assert_eq!(list_overrides(&c).unwrap().len(), 1);
        remove_override(&c, "paypa1-secure-login.com").unwrap();
        assert_eq!(get(&c, &s.id).unwrap().security_status, SecurityStatus::Suspicious);
        let r = rescan(&c, None).unwrap();
        assert_eq!((r.scanned, r.suspicious), (2, 2));
    }

    #[test]
    fn update_rechecks_changed_url() {
        let c = open_in_memory().unwrap();
        let b = add(&c, "https://example.com");
        let u = update(
            &c,
            UpdateBookmark {
                id: b.id.clone(),
                url: "https://accounts-google.com/login".into(),
                title: "  ".into(),
                description: Some("  ".into()),
                category_id: None,
                is_favourite: true,
                tags: vec![],
            },
        )
        .unwrap();
        assert_eq!(u.security_status, SecurityStatus::Suspicious);
        assert_eq!(u.title, "accounts-google.com");
        assert!(u.description.is_none());
        assert!(u.is_favourite);
    }
}

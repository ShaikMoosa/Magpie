//! Bookmark CRUD, search and counts.

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter, types::Value};

use crate::db::{new_id, now};
use crate::error::{Error, Result};
use crate::models::{
    Bookmark, BookmarkQuery, Counts, NewBookmark, SecurityStatus, Sort, UpdateBookmark, UrlCheck, View,
};
use crate::{security, urls};

const REASON_SEP: char = '\u{1f}';
const RECENT_DAYS: i64 = 30;

const SELECT: &str = "SELECT b.id, b.url, b.title, b.description, b.domain, b.favicon_path, b.thumbnail_path,
    b.category_id, b.is_favourite, b.source_browser, b.source_folder, b.security_status, b.security_reasons,
    b.security_source, b.security_checked_at, b.created_at, b.updated_at, b.last_opened_at, b.open_count,
    b.deleted_at,
    (SELECT group_concat(t.name, char(31)) FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
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
        tags: split(r.get(20)?),
    })
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
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
    let assessment = security::assess(&parsed.url);
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
    let parsed = urls::parse(&input.url)?;
    if !input.allow_duplicate
        && let Some(existing_id) = find_duplicate(conn, &parsed.normalized, None)?
    {
        return Err(Error::Duplicate { existing_id });
    }
    ensure_category(conn, &input.category_id)?;
    let assessment = security::assess(&parsed.url);
    let id = new_id();
    let ts = now();
    let title = clean(input.title).unwrap_or_else(|| parsed.domain.clone());
    let reasons: String = assessment.reasons.join(&REASON_SEP.to_string());

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO bookmarks (id, url, normalized_url, title, description, domain, category_id,
            is_favourite, source_browser, source_folder, security_status, security_reasons, security_source,
            security_checked_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
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
            ts,
            ts,
        ],
    )?;
    set_tags(&tx, &id, &input.tags)?;
    reindex(&tx, &id)?;
    tx.commit()?;
    get(conn, &id)
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
        let a = security::assess(&parsed.url);
        tx.execute(
            "UPDATE bookmarks SET security_status = ?2, security_reasons = ?3, security_source = ?4,
                security_checked_at = ?5, thumbnail_path = NULL, favicon_path = NULL WHERE id = ?1",
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

pub fn list(conn: &Connection, q: &BookmarkQuery) -> Result<Vec<Bookmark>> {
    let mut sql = format!("{SELECT} WHERE ");
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
    if let Some(domain) = q.domain.as_deref().filter(|d| !d.is_empty()) {
        sql.push_str(" AND b.domain = ?");
        args.push(domain.to_string().into());
    }
    if let Some(m) = q.search.as_deref().and_then(fts_query) {
        sql.push_str(" AND b.id IN (SELECT bookmark_id FROM bookmarks_fts WHERE bookmarks_fts MATCH ?)");
        args.push(m.into());
    }

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
    if let Some(limit) = q.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
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
        let removed = tx.execute("DELETE FROM bookmarks WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?;
        if removed > 0 {
            tx.execute("DELETE FROM bookmarks_fts WHERE bookmark_id = ?1", [id])?;
        }
        n += removed;
    }
    tx.commit()?;
    Ok(n)
}

pub fn empty_trash(conn: &Connection) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM bookmarks_fts WHERE bookmark_id IN (SELECT id FROM bookmarks WHERE deleted_at IS NOT NULL)",
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
    conn.execute("DELETE FROM bookmarks_fts WHERE bookmark_id = ?1", [id])?;
    conn.execute(
        "INSERT INTO bookmarks_fts (bookmark_id, title, description, url, domain, category, tags)
         SELECT b.id, b.title, COALESCE(b.description, ''), b.url, b.domain,
            COALESCE((SELECT name FROM categories c WHERE c.id = b.category_id), ''),
            COALESCE((SELECT group_concat(t.name, ' ') FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                      WHERE bt.bookmark_id = b.id), '')
         FROM bookmarks b WHERE b.id = ?1",
        [id],
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

//! Browser bookmark import: Chrome/Edge profile files (JSON) and Netscape
//! bookmark HTML exports. Browser files are only ever read, never written.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::{new_id, now};
use crate::error::{Error, Result};
use crate::models::{NewBookmark, NewCategory};
use crate::{bookmarks, categories, urls};

/// Separator for folder keys sent to the frontend (folder names may contain `/`).
pub const KEY_SEP: char = '\u{1f}';
const MAX_HTML_BYTES: u64 = 50 * 1024 * 1024;
const PALETTE: &[&str] = &["#2563EB", "#7C3AED", "#DB2777", "#EA580C", "#16A34A", "#0D9488", "#D97706", "#DC2626", "#475569"];
/// Top-level containers that never become categories.
const ROOT_FOLDERS: &[&str] = &[
    "bookmarks bar", "bookmarks toolbar", "other bookmarks", "mobile bookmarks", "favorites bar",
    "favourites bar", "other favorites", "other favourites", "bookmarks menu",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Edge,
}

impl Browser {
    pub fn label(self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome",
            Browser::Edge => "Edge",
        }
    }
    fn user_data(self, local_app_data: &Path) -> PathBuf {
        match self {
            Browser::Chrome => local_app_data.join("Google").join("Chrome").join("User Data"),
            Browser::Edge => local_app_data.join("Microsoft").join("Edge").join("User Data"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub browser: Browser,
    /// Folder name, e.g. `Default` or `Profile 1`.
    pub dir: String,
    /// Display name from the browser, e.g. `Work`.
    pub name: String,
    pub bookmark_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportSource {
    Browser { browser: Browser, profile: String },
    Html { path: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawBookmark {
    pub url: String,
    pub title: String,
    pub folder: Vec<String>,
    /// RFC 3339 timestamp from the browser, if known.
    pub added: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFolder {
    pub key: String,
    pub name: String,
    pub depth: usize,
    /// Bookmarks directly inside this folder.
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub source_label: String,
    pub total: usize,
    pub folders: usize,
    pub valid: usize,
    pub invalid: usize,
    pub duplicates_in_file: usize,
    pub already_in_library: usize,
    pub new_count: usize,
    pub folder_tree: Vec<ImportFolder>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    /// Folder keys to import. Items are included when their own folder is selected.
    pub selected_folders: Vec<String>,
    pub folders_as_categories: bool,
    pub skip_duplicates: bool,
    pub destination_category_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub total: usize,
    pub imported: usize,
    pub duplicates_skipped: usize,
    pub invalid_skipped: usize,
    pub categories_created: usize,
}

// ---------------------------------------------------------------- profiles

fn valid_profile_dir(dir: &str) -> bool {
    !dir.is_empty() && !dir.contains(['/', '\\', ':']) && dir != "." && dir != ".."
}

/// Path to a profile's `Bookmarks` file. `profile` must be a plain folder name.
pub fn profile_bookmarks_path(local_app_data: &Path, browser: Browser, profile: &str) -> Result<PathBuf> {
    if !valid_profile_dir(profile) {
        return Err(Error::Validation("Unknown browser profile.".into()));
    }
    Ok(browser.user_data(local_app_data).join(profile).join("Bookmarks"))
}

/// Find Chrome and Edge profiles that have a bookmarks file.
pub fn detect_profiles(local_app_data: &Path) -> Vec<BrowserProfile> {
    let mut out = Vec::new();
    for browser in [Browser::Chrome, Browser::Edge] {
        let root = browser.user_data(local_app_data);
        let names = profile_names(&root);
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        let mut found: Vec<BrowserProfile> = entries
            .flatten()
            .filter_map(|e| {
                let dir = e.file_name().to_string_lossy().to_string();
                let file = e.path().join("Bookmarks");
                if !file.is_file() {
                    return None;
                }
                let count = std::fs::read_to_string(&file)
                    .ok()
                    .and_then(|t| parse_chromium_json(&t).ok())
                    .map(|v| v.len())
                    .unwrap_or(0);
                Some(BrowserProfile {
                    browser,
                    name: names.get(&dir).cloned().unwrap_or_else(|| dir.clone()),
                    dir,
                    bookmark_count: count,
                })
            })
            .collect();
        found.sort_by(|a, b| (a.dir != "Default", &a.dir).cmp(&(b.dir != "Default", &b.dir)));
        out.extend(found);
    }
    out
}

/// Display names from `Local State` → `profile.info_cache`.
fn profile_names(user_data: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(user_data.join("Local State")) else { return HashMap::new() };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return HashMap::new() };
    json.pointer("/profile/info_cache")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(dir, info)| Some((dir.clone(), info.get("name")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------- parsing

/// Chrome/Edge `Bookmarks` JSON.
pub fn parse_chromium_json(text: &str) -> Result<Vec<RawBookmark>> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|_| Error::Validation("The browser's bookmarks file couldn't be read.".into()))?;
    let roots = json
        .get("roots")
        .and_then(|r| r.as_object())
        .ok_or_else(|| Error::Validation("The browser's bookmarks file couldn't be read.".into()))?;
    let mut out = Vec::new();
    for key in ["bookmark_bar", "other", "synced"] {
        if let Some(node) = roots.get(key) {
            walk_chromium(node, &mut Vec::new(), &mut out);
        }
    }
    Ok(out)
}

fn walk_chromium(node: &serde_json::Value, path: &mut Vec<String>, out: &mut Vec<RawBookmark>) {
    let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    match node.get("type").and_then(|v| v.as_str()) {
        Some("url") => out.push(RawBookmark {
            url: node.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            title: name,
            folder: path.clone(),
            added: node.get("date_added").and_then(|v| v.as_str()).and_then(chromium_time),
        }),
        Some("folder") => {
            path.push(if name.is_empty() { "Untitled folder".into() } else { name });
            for child in node.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
                walk_chromium(child, path, out);
            }
            path.pop();
        }
        _ => {}
    }
}

/// Chromium timestamps are microseconds since 1601-01-01 UTC.
fn chromium_time(s: &str) -> Option<String> {
    let micros: i64 = s.parse().ok()?;
    if micros <= 0 {
        return None;
    }
    let unix = micros / 1_000_000 - 11_644_473_600;
    unix_time(unix)
}

fn unix_time(secs: i64) -> Option<String> {
    if secs <= 0 {
        return None;
    }
    chrono::DateTime::from_timestamp(secs, 0).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

/// Netscape bookmark HTML, as exported by every major browser.
pub fn parse_netscape_html(text: &str) -> Vec<RawBookmark> {
    let mut out = Vec::new();
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut pending: Option<String> = None;
    let lower = text.to_ascii_lowercase();
    let mut i = 0;
    while let Some(off) = lower[i..].find('<') {
        let start = i + off;
        let Some(end_off) = lower[start..].find('>') else { break };
        let end = start + end_off;
        let tag = &lower[start + 1..end];
        let raw_tag = &text[start + 1..end];
        i = end + 1;
        let name = tag.trim_start_matches('/').split(|c: char| c.is_whitespace()).next().unwrap_or("");
        let closing = tag.starts_with('/');
        match (name, closing) {
            ("h3", false) => {
                let close = lower[i..].find("</h3").map(|o| i + o).unwrap_or(i);
                pending = Some(clean_text(&text[i..close]));
                i = close;
            }
            ("dl", false) => stack.push(pending.take()),
            ("dl", true) => {
                stack.pop();
            }
            ("a", false) => {
                let close = lower[i..].find("</a").map(|o| i + o).unwrap_or(i);
                let url = attr(raw_tag, "href").unwrap_or_default();
                let added = attr(raw_tag, "add_date").and_then(|s| s.parse().ok()).and_then(unix_time);
                out.push(RawBookmark {
                    url,
                    title: clean_text(&text[i..close]),
                    folder: stack.iter().flatten().cloned().collect(),
                    added,
                });
                i = close;
            }
            _ => {}
        }
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(name) {
        let at = from + pos;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - rest.len() + 1;
        let v = tag[value_start..].trim_start();
        let (quote, body) = match v.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &v[1..]),
            _ => (None, v),
        };
        let value = match quote {
            Some(q) => body.split(q).next().unwrap_or(""),
            None => body.split(char::is_whitespace).next().unwrap_or(""),
        };
        return Some(decode_entities(value));
    }
    None
}

fn clean_text(s: &str) -> String {
    let mut no_tags = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => no_tags.push(c),
            _ => {}
        }
    }
    decode_entities(no_tags.split_whitespace().collect::<Vec<_>>().join(" ").as_str())
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let Some(semi) = rest[..rest.len().min(10)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let decoded = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- reading

/// Read and parse an import source. Returns (label, browser name, items).
pub fn read_source(source: &ImportSource, local_app_data: &Path) -> Result<(String, String, Vec<RawBookmark>)> {
    match source {
        ImportSource::Browser { browser, profile } => {
            let path = profile_bookmarks_path(local_app_data, *browser, profile)?;
            let text = std::fs::read_to_string(&path).map_err(|_| {
                Error::Validation(format!(
                    "Couldn't read {}'s bookmarks. Try closing {} or import an exported HTML file instead.",
                    browser.label(),
                    browser.label()
                ))
            })?;
            let name = profile_names(&browser.user_data(local_app_data)).get(profile).cloned().unwrap_or_else(|| profile.clone());
            Ok((format!("{} — {}", browser.label(), name), browser.label().to_string(), parse_chromium_json(&text)?))
        }
        ImportSource::Html { path } => {
            let path = PathBuf::from(path);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext != "html" && ext != "htm" {
                return Err(Error::Validation("Choose an .html bookmarks file.".into()));
            }
            let meta = std::fs::metadata(&path).map_err(|_| Error::Validation("That file couldn't be opened.".into()))?;
            if !meta.is_file() || meta.len() > MAX_HTML_BYTES {
                return Err(Error::Validation("That file is too large to be a bookmarks export.".into()));
            }
            let bytes = std::fs::read(&path)?;
            let text = String::from_utf8_lossy(&bytes);
            let items = parse_netscape_html(&text);
            let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "HTML file".into());
            Ok((label, "HTML file".into(), items))
        }
    }
}

fn folder_key(path: &[String]) -> String {
    path.join(&KEY_SEP.to_string())
}

// ---------------------------------------------------------------- preview

pub fn preview(conn: &Connection, label: String, items: &[RawBookmark]) -> Result<ImportPreview> {
    let mut seen = HashSet::new();
    let (mut valid, mut invalid, mut dup_file, mut existing) = (0, 0, 0, 0);
    for item in items {
        match urls::parse(&item.url) {
            Ok(p) => {
                valid += 1;
                if !seen.insert(p.normalized.clone()) {
                    dup_file += 1;
                } else if bookmarks::check_url(conn, p.url.as_str(), None)?.duplicate_of.is_some() {
                    existing += 1;
                }
            }
            Err(_) => invalid += 1,
        }
    }

    // Folder tree in first-seen order, including empty parents of nested folders.
    let mut order: Vec<Vec<String>> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in items {
        for depth in 1..=item.folder.len() {
            let path = item.folder[..depth].to_vec();
            if !counts.contains_key(&folder_key(&path)) {
                counts.insert(folder_key(&path), 0);
                order.push(path);
            }
        }
        if item.folder.is_empty() && !counts.contains_key("") {
            counts.insert(String::new(), 0);
            order.insert(0, Vec::new());
        }
        *counts.get_mut(&folder_key(&item.folder)).unwrap() += 1;
    }
    order.sort_by(|a, b| a.cmp(b));
    // Keep each browser root's order as first seen (bar, other, mobile) while sorting within.
    let first_seen: Vec<String> = {
        let mut v = Vec::new();
        for item in items {
            if let Some(r) = item.folder.first()
                && !v.contains(r)
            {
                v.push(r.clone());
            }
        }
        v
    };
    order.sort_by_key(|p| (p.first().and_then(|r| first_seen.iter().position(|x| x == r)).unwrap_or(0), p.clone()));

    let folder_tree = order
        .iter()
        .map(|p| ImportFolder {
            key: folder_key(p),
            name: p.last().cloned().unwrap_or_else(|| "Not in a folder".into()),
            depth: p.len().saturating_sub(1),
            count: counts[&folder_key(p)],
        })
        .collect::<Vec<_>>();

    Ok(ImportPreview {
        source_label: label,
        total: items.len(),
        folders: folder_tree.iter().filter(|f| !f.key.is_empty()).count(),
        valid,
        invalid,
        duplicates_in_file: dup_file,
        already_in_library: existing,
        new_count: valid - dup_file - existing,
        folder_tree,
    })
}

// ---------------------------------------------------------------- commit

fn category_path(folder: &[String]) -> &[String] {
    match folder.first() {
        Some(root) if ROOT_FOLDERS.contains(&root.to_lowercase().as_str()) => &folder[1..],
        _ => folder,
    }
}

fn truncate(name: &str) -> String {
    let n: String = name.trim().chars().take(40).collect();
    if n.is_empty() { "Untitled folder".into() } else { n }
}

/// Import the selected folders in one transaction. Never touches the source file.
pub fn import(conn: &Connection, label: &str, browser: &str, items: &[RawBookmark], opts: &ImportOptions) -> Result<ImportSummary> {
    let selected: HashSet<&str> = opts.selected_folders.iter().map(String::as_str).collect();
    if let Some(dest) = &opts.destination_category_id {
        categories::get(conn, dest)?;
    }
    let started = now();
    let tx = conn.unchecked_transaction()?;
    let mut summary = ImportSummary::default();
    let mut cache: HashMap<(Option<String>, String), String> = HashMap::new();
    let mut palette_i: usize = tx.query_row("SELECT COUNT(*) FROM categories WHERE parent_id IS NULL", [], |r| r.get::<_, i64>(0))? as usize;

    for item in items.iter().filter(|i| selected.contains(folder_key(&i.folder).as_str())) {
        summary.total += 1;
        let mut category_id = opts.destination_category_id.clone();
        if opts.folders_as_categories {
            for name in category_path(&item.folder) {
                let name = truncate(name);
                let key = (category_id.clone(), name.to_lowercase());
                let id = if let Some(id) = cache.get(&key) {
                    id.clone()
                } else {
                    let found: Option<String> = tx
                        .query_row(
                            "SELECT id FROM categories WHERE COALESCE(parent_id, '') = COALESCE(?1, '') AND name = ?2 COLLATE NOCASE",
                            params![category_id, name],
                            |r| r.get(0),
                        )
                        .optional()?;
                    let id = match found {
                        Some(id) => id,
                        None => {
                            let color = match &category_id {
                                Some(parent) => categories::get(&tx, parent)?.color,
                                None => {
                                    palette_i += 1;
                                    PALETTE[(palette_i - 1) % PALETTE.len()].to_string()
                                }
                            };
                            summary.categories_created += 1;
                            categories::create(
                                &tx,
                                NewCategory { name: name.clone(), description: None, color, icon: None, parent_id: category_id.clone() },
                            )?
                            .id
                        }
                    };
                    cache.insert(key, id.clone());
                    id
                };
                category_id = Some(id);
            }
        }
        let input = NewBookmark {
            url: item.url.clone(),
            title: Some(item.title.clone()),
            category_id,
            allow_duplicate: !opts.skip_duplicates,
            source_browser: Some(browser.to_string()),
            source_folder: (!item.folder.is_empty()).then(|| item.folder.join(" › ")),
            ..Default::default()
        };
        match bookmarks::insert(&tx, input, item.added.as_deref()) {
            Ok(_) => summary.imported += 1,
            Err(Error::Duplicate { .. }) => summary.duplicates_skipped += 1,
            Err(Error::InvalidUrl(_)) => summary.invalid_skipped += 1,
            Err(e) => return Err(e),
        }
    }

    tx.execute(
        "INSERT INTO import_history (id, source, status, total, imported, duplicates, invalid, started_at, finished_at)
         VALUES (?1, ?2, 'completed', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            new_id(),
            label,
            summary.total as i64,
            summary.imported as i64,
            summary.duplicates_skipped as i64,
            summary.invalid_skipped as i64,
            started,
            now()
        ],
    )?;
    tx.commit()?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::BookmarkQuery;

    const CHROME: &str = r#"{
      "roots": {
        "bookmark_bar": { "type": "folder", "name": "Bookmarks bar", "children": [
          { "type": "url", "name": "Figma", "url": "https://figma.com/", "date_added": "13300000000000000" },
          { "type": "folder", "name": "Design", "children": [
            { "type": "url", "name": "Lucide", "url": "https://lucide.dev" },
            { "type": "folder", "name": "Fonts", "children": [
              { "type": "url", "name": "Google Fonts", "url": "https://fonts.google.com" }
            ]}
          ]},
          { "type": "url", "name": "Bookmarklet", "url": "javascript:alert(1)" }
        ]},
        "other": { "type": "folder", "name": "Other bookmarks", "children": [
          { "type": "url", "name": "Figma again", "url": "https://www.figma.com" }
        ]},
        "synced": { "type": "folder", "name": "Mobile bookmarks", "children": [] }
      }
    }"#;

    const HTML: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://react.dev/learn" ADD_DATE="1700000001">React &amp; friends</A>
        <DT><H3>Dev &lt;docs&gt;</H3>
        <DL><p>
            <DT><A href='https://doc.rust-lang.org/book/' add_date="1700000002">The Rust Book</A>
        </DL><p>
    </DL><p>
    <DT><A HREF="https://example.com/?a=1&amp;b=2">Top level</A>
</DL><p>"#;

    #[test]
    fn parses_chromium_json() {
        let items = parse_chromium_json(CHROME).unwrap();
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].folder, vec!["Bookmarks bar"]);
        assert_eq!(items[2].folder, vec!["Bookmarks bar", "Design", "Fonts"]);
        assert!(items[0].added.as_deref().unwrap().starts_with("2022-"));
    }

    #[test]
    fn parses_netscape_html() {
        let items = parse_netscape_html(HTML);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "React & friends");
        assert_eq!(items[0].folder, vec!["Bookmarks bar"]);
        assert_eq!(items[1].folder, vec!["Bookmarks bar", "Dev <docs>"]);
        assert_eq!(items[1].url, "https://doc.rust-lang.org/book/");
        assert_eq!(items[2].folder, Vec::<String>::new());
        assert_eq!(items[2].url, "https://example.com/?a=1&b=2");
        assert!(items[0].added.is_some());
    }

    #[test]
    fn preview_counts() {
        let conn = open_in_memory().unwrap();
        bookmarks::create(&conn, NewBookmark { url: "https://lucide.dev".into(), ..Default::default() }).unwrap();
        let items = parse_chromium_json(CHROME).unwrap();
        let p = preview(&conn, "Chrome".into(), &items).unwrap();
        assert_eq!((p.total, p.valid, p.invalid, p.duplicates_in_file, p.already_in_library, p.new_count), (5, 4, 1, 1, 1, 2));
        let names: Vec<_> = p.folder_tree.iter().map(|f| (f.name.as_str(), f.depth, f.count)).collect();
        assert_eq!(
            names,
            vec![("Bookmarks bar", 0, 2), ("Design", 1, 1), ("Fonts", 2, 1), ("Other bookmarks", 0, 1)]
        );
    }

    #[test]
    fn import_creates_nested_categories_and_skips_duplicates() {
        let conn = open_in_memory().unwrap();
        let items = parse_chromium_json(CHROME).unwrap();
        let p = preview(&conn, "Chrome".into(), &items).unwrap();
        let opts = ImportOptions {
            selected_folders: p.folder_tree.iter().map(|f| f.key.clone()).collect(),
            folders_as_categories: true,
            skip_duplicates: true,
            destination_category_id: None,
        };
        let s = import(&conn, "Chrome — Default", "Chrome", &items, &opts).unwrap();
        assert_eq!((s.total, s.imported, s.duplicates_skipped, s.invalid_skipped, s.categories_created), (5, 3, 1, 1, 2));

        let cats = categories::list(&conn).unwrap();
        let design = cats.iter().find(|c| c.name == "Design").unwrap();
        let fonts = cats.iter().find(|c| c.name == "Fonts").unwrap();
        assert_eq!(fonts.parent_id.as_deref(), Some(design.id.as_str()));
        assert_eq!(fonts.color, design.color);

        let all = bookmarks::list(&conn, &BookmarkQuery::default()).unwrap();
        let figma = all.iter().find(|b| b.domain == "figma.com").unwrap();
        assert_eq!(figma.category_id, None, "root-folder items stay uncategorized");
        assert_eq!(figma.source_browser.as_deref(), Some("Chrome"));
        assert!(figma.created_at.starts_with("2022-"), "keeps the browser's date added");

        // Importing again creates nothing new.
        let again = import(&conn, "Chrome — Default", "Chrome", &items, &opts).unwrap();
        assert_eq!((again.imported, again.categories_created), (0, 0));
    }

    #[test]
    fn only_selected_folders_are_imported() {
        let conn = open_in_memory().unwrap();
        let items = parse_chromium_json(CHROME).unwrap();
        let key = folder_key(&["Bookmarks bar".to_string(), "Design".to_string()]);
        let opts = ImportOptions { selected_folders: vec![key], folders_as_categories: false, skip_duplicates: true, destination_category_id: None };
        let s = import(&conn, "x", "Chrome", &items, &opts).unwrap();
        assert_eq!(s.imported, 1);
    }

    #[test]
    fn detects_profiles_with_names() {
        let dir = tempfile::tempdir().unwrap();
        let ud = dir.path().join("Google").join("Chrome").join("User Data");
        for p in ["Default", "Profile 1", "System Profile"] {
            std::fs::create_dir_all(ud.join(p)).unwrap();
        }
        std::fs::write(ud.join("Default").join("Bookmarks"), CHROME).unwrap();
        std::fs::write(ud.join("Profile 1").join("Bookmarks"), CHROME).unwrap();
        std::fs::write(ud.join("Local State"), r#"{"profile":{"info_cache":{"Profile 1":{"name":"Work"}}}}"#).unwrap();
        let found = detect_profiles(dir.path());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].dir, "Default");
        assert_eq!(found[1].name, "Work");
        assert_eq!(found[1].bookmark_count, 5);
        assert!(profile_bookmarks_path(dir.path(), Browser::Chrome, "..\\x").is_err());
    }

    #[test]
    fn html_source_requires_html_extension() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, "hi").unwrap();
        let src = ImportSource::Html { path: txt.to_string_lossy().into() };
        assert!(read_source(&src, dir.path()).is_err());
        let html = dir.path().join("bookmarks.html");
        std::fs::write(&html, HTML).unwrap();
        let src = ImportSource::Html { path: html.to_string_lossy().into() };
        let (label, browser, items) = read_source(&src, dir.path()).unwrap();
        assert_eq!((label.as_str(), browser.as_str(), items.len()), ("bookmarks.html", "HTML file", 3));
    }
}

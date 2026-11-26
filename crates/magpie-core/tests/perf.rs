//! Large-library timings against the PRD §11.1 targets.
//!
//! Not part of the normal test run. Run with:
//!     cargo test --release --test perf -- --ignored --nocapture
//! Set MAGPIE_PERF_N to change the size (default 10,000; the PRD's stress
//! target is 100,000).

use std::time::{Duration, Instant};

use magpie_core::import::{self, ImportOptions, RawBookmark};
use magpie_core::models::{BookmarkQuery, NewBookmark, View};
use magpie_core::{bookmarks, categories, db};

const WORDS: &[&str] = &[
    "react", "rust", "design", "figma", "tauri", "sqlite", "typescript", "research", "paper", "guide", "tutorial", "icons",
    "colour", "layout", "grid", "motion", "accessibility", "performance", "security", "testing", "docs", "api", "cloud",
    "notes", "recipes", "travel", "music", "photography", "finance", "health",
];
const DOMAINS: &[&str] = &["github.com", "dev.to", "medium.com", "figma.com", "react.dev", "docs.rs", "arxiv.org", "youtube.com", "stackoverflow.com", "dribbble.com"];

fn library(n: usize) -> Vec<RawBookmark> {
    // Deterministic pseudo-random text so runs are comparable.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    (0..n)
        .map(|i| {
            let w = |r: u64| WORDS[(r % WORDS.len() as u64) as usize];
            let title = format!("{} {} {} #{i}", w(next()), w(next()), w(next()));
            let domain = DOMAINS[(next() % DOMAINS.len() as u64) as usize];
            let top = format!("Folder {}", next() % 12);
            let sub = format!("Sub {}", next() % 5);
            RawBookmark {
                url: format!("https://{domain}/{}/{i}", w(next())),
                title,
                folder: vec!["Bookmarks bar".into(), top, sub],
                added: None,
            }
        })
        .collect()
}

fn time<T>(label: &str, target: Option<Duration>, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    let took = t.elapsed();
    let verdict = match target {
        Some(limit) if took > limit => format!("  ✗ over {} ms target", limit.as_millis()),
        Some(limit) => format!("  ✓ under {} ms", limit.as_millis()),
        None => String::new(),
    };
    println!("{label:<44} {:>8.1} ms{verdict}", took.as_secs_f64() * 1000.0);
    out
}

#[test]
#[ignore]
fn large_library_timings() {
    let n: usize = std::env::var("MAGPIE_PERF_N").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("magpie.db")).unwrap();
    let items = library(n);
    println!("\n--- {n} bookmarks ---");

    let preview = time("import preview", None, || import::preview(&conn, "perf".into(), &items).unwrap());
    let opts = ImportOptions {
        selected_folders: preview.folder_tree.iter().map(|f| f.key.clone()).collect(),
        folders_as_categories: true,
        skip_duplicates: true,
        destination_category_id: None,
    };
    let summary = time("import", None, || import::import(&conn, "perf", "Chrome", &items, &opts).unwrap());
    assert_eq!(summary.imported, n);

    let ms = Duration::from_millis;
    time("counts (sidebar)", Some(ms(50)), || bookmarks::counts(&conn).unwrap());
    let cats = time("categories with counts", Some(ms(50)), || categories::list(&conn).unwrap());
    time("facets (filter menus)", Some(ms(200)), || bookmarks::facets(&conn).unwrap());

    let all = time("list all (dashboard)", Some(ms(2000)), || bookmarks::list(&conn, &BookmarkQuery::default()).unwrap());
    assert_eq!(all.len(), n);
    let json = time("serialize all to JSON (IPC)", None, || serde_json::to_vec(&all).unwrap());
    println!("{:<44} {:>8.1} MB", "  IPC payload", json.len() as f64 / 1_048_576.0);

    let first = time("list first 200 (dashboard page)", Some(ms(200)), || {
        bookmarks::list(&conn, &BookmarkQuery { limit: Some(200), ..Default::default() }).unwrap()
    });
    assert_eq!(first.len(), 200.min(n));

    let cat = cats.iter().find(|c| c.parent_id.is_some()).unwrap();
    time("category switch", Some(ms(200)), || {
        bookmarks::list(&conn, &BookmarkQuery { view: View::Category, category_id: Some(cat.id.clone()), ..Default::default() }).unwrap()
    });
    time("favourites view", Some(ms(200)), || {
        bookmarks::list(&conn, &BookmarkQuery { view: View::Favourites, ..Default::default() }).unwrap()
    });
    let hits = time("search \"react\"", Some(ms(300)), || {
        bookmarks::list(&conn, &BookmarkQuery { search: Some("react".into()), ..Default::default() }).unwrap()
    });
    println!("{:<44} {:>8}", "  matches", hits.len());
    time("search prefix \"phot\"", Some(ms(300)), || {
        bookmarks::list(&conn, &BookmarkQuery { search: Some("phot".into()), ..Default::default() }).unwrap()
    });
    time("search two words \"rust guide\"", Some(ms(300)), || {
        bookmarks::list(&conn, &BookmarkQuery { search: Some("rust guide".into()), ..Default::default() }).unwrap()
    });
    time("search + domain filter", Some(ms(300)), || {
        bookmarks::list(&conn, &BookmarkQuery { search: Some("design".into()), domain: Some("figma.com".into()), ..Default::default() }).unwrap()
    });
    time("list page 10 (offset 1800)", Some(Duration::from_millis(200)), || {
        bookmarks::list(&conn, &BookmarkQuery { limit: Some(200), offset: Some(1800), ..Default::default() }).unwrap()
    });
    time("check_url (duplicate check)", Some(ms(100)), || bookmarks::check_url(&conn, "https://github.com/new/1", None).unwrap());
    time("create bookmark", Some(ms(500)), || {
        bookmarks::create(&conn, NewBookmark { url: "https://example.com/new-one".into(), ..Default::default() }).unwrap()
    });
    time("local security rescan (all)", None, || bookmarks::rescan(&conn, None).unwrap());
}

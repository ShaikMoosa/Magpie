//! Magpie core library: SQLite storage, bookmarks, categories, URL
//! validation and local security heuristics. Has no UI or Tauri dependency
//! so it can be unit-tested on its own.

pub mod bookmarks;
pub mod categories;
pub mod db;
pub mod error;
pub mod import;
pub mod models;
pub mod security;
pub mod urls;

pub use error::{Error, Result};

/// Re-exported so the app crate uses the same SQLite build.
pub use rusqlite;

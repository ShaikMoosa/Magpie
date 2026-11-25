-- Magpie schema v1

CREATE TABLE categories (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT NOT NULL,
    icon        TEXT,
    parent_id   TEXT REFERENCES categories(id) ON DELETE SET NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_categories_sibling_name
    ON categories (COALESCE(parent_id, ''), name COLLATE NOCASE);

CREATE TABLE bookmarks (
    id                     TEXT PRIMARY KEY,
    url                    TEXT NOT NULL,
    normalized_url         TEXT NOT NULL,
    title                  TEXT NOT NULL,
    description            TEXT,
    domain                 TEXT NOT NULL,
    favicon_path           TEXT,
    thumbnail_path         TEXT,
    category_id            TEXT REFERENCES categories(id) ON DELETE SET NULL,
    is_favourite           INTEGER NOT NULL DEFAULT 0,
    source_browser         TEXT,
    source_folder          TEXT,
    security_status        TEXT NOT NULL DEFAULT 'not_checked',
    security_reasons       TEXT,
    security_source        TEXT,
    security_checked_at    TEXT,
    ai_category_confidence REAL,
    created_at             TEXT NOT NULL,
    updated_at             TEXT NOT NULL,
    last_opened_at         TEXT,
    open_count             INTEGER NOT NULL DEFAULT 0,
    deleted_at             TEXT
);
CREATE INDEX idx_bookmarks_normalized ON bookmarks (normalized_url);
CREATE INDEX idx_bookmarks_category   ON bookmarks (category_id);
CREATE INDEX idx_bookmarks_created    ON bookmarks (created_at);
CREATE INDEX idx_bookmarks_deleted    ON bookmarks (deleted_at);

CREATE TABLE tags (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE bookmark_tags (
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (bookmark_id, tag_id)
);

CREATE TABLE security_scans (
    id          TEXT PRIMARY KEY,
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    provider    TEXT NOT NULL,
    status      TEXT NOT NULL,
    details     TEXT,
    scanned_at  TEXT NOT NULL
);

CREATE TABLE bookmark_history (
    id          TEXT PRIMARY KEY,
    bookmark_id TEXT NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
    opened_at   TEXT NOT NULL
);

CREATE TABLE application_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE import_history (
    id          TEXT PRIMARY KEY,
    source      TEXT NOT NULL,
    status      TEXT NOT NULL,
    total       INTEGER,
    imported    INTEGER,
    duplicates  INTEGER,
    invalid     INTEGER,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    error       TEXT
);

-- Full-text index, maintained by the application (see bookmarks::reindex).
CREATE VIRTUAL TABLE bookmarks_fts USING fts5(
    bookmark_id UNINDEXED,
    title,
    description,
    url,
    domain,
    category,
    tags,
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);

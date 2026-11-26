-- Performance for large libraries.
--
-- 1. The search index used `bookmark_id` (an unindexed FTS column) to find a
--    bookmark's row, so every insert/update scanned the whole index and big
--    imports were quadratic. Each bookmark now has a stable integer
--    `search_rowid` that is also its row number in the index. (SQLite's own
--    rowid isn't used because VACUUM may renumber it.)
-- 2. Scan history had no index on bookmark_id, so reading a bookmark's
--    latest results scanned all history.

ALTER TABLE bookmarks ADD COLUMN search_rowid INTEGER;
UPDATE bookmarks SET search_rowid = rowid;
CREATE UNIQUE INDEX idx_bookmarks_search_rowid ON bookmarks (search_rowid);

DROP TABLE bookmarks_fts;
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
INSERT INTO bookmarks_fts (rowid, bookmark_id, title, description, url, domain, category, tags)
SELECT b.search_rowid, b.id, b.title, COALESCE(b.description, ''), b.url, b.domain,
    COALESCE((SELECT name FROM categories c WHERE c.id = b.category_id), ''),
    COALESCE((SELECT group_concat(t.name, ' ') FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
              WHERE bt.bookmark_id = b.id), '')
FROM bookmarks b;

CREATE INDEX idx_security_scans_bookmark ON security_scans (bookmark_id, scanned_at);

-- Live bookmarks newest first (the default view) and per-category counts.
CREATE INDEX idx_bookmarks_live_created ON bookmarks (deleted_at, created_at);
CREATE INDEX idx_bookmarks_category_live ON bookmarks (category_id, deleted_at);

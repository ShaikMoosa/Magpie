-- Link preview state. NULL status = not fetched yet.
ALTER TABLE bookmarks ADD COLUMN metadata_status TEXT;
ALTER TABLE bookmarks ADD COLUMN metadata_fetched_at TEXT;
CREATE INDEX idx_bookmarks_metadata ON bookmarks (metadata_status);

-- Domains the user marked as safe ("report false positive").
CREATE TABLE security_overrides (
    domain     TEXT PRIMARY KEY,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_bookmarks_domain ON bookmarks (domain);
CREATE INDEX idx_bookmarks_security ON bookmarks (security_status);

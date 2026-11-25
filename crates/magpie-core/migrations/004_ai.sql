-- AI category suggestions (one per bookmark). Rejected ones are kept so a
-- batch run doesn't suggest the same thing again.
CREATE TABLE ai_suggestions (
    bookmark_id       TEXT PRIMARY KEY REFERENCES bookmarks(id) ON DELETE CASCADE,
    category_id       TEXT REFERENCES categories(id) ON DELETE CASCADE,
    new_category_name TEXT,
    tags              TEXT,
    confidence        REAL NOT NULL,
    reason            TEXT,
    provider          TEXT NOT NULL,
    model             TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'pending',
    created_at        TEXT NOT NULL
);
CREATE INDEX idx_ai_suggestions_status ON ai_suggestions (status);

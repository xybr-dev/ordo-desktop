-- Arrangement details (T2.6). An Arrangement title becomes optional and gains
-- optional source and rights notes. SQLite cannot relax NOT NULL in place, so
-- the table is rebuilt: rows are copied to a side table, the old table is
-- dropped, the new shape takes the same name, and rows are copied back. Child
-- rows are untouched. Deferred foreign keys cover the drop and recreate inside
-- the runner's transaction; the violation count is zero again by the end.

PRAGMA defer_foreign_keys = ON;

CREATE TABLE arrangement_rebuild (
    id TEXT NOT NULL,
    song_id TEXT NOT NULL,
    title TEXT NOT NULL,
    rendition_attribution TEXT,
    created_at TEXT NOT NULL,
    archived_at TEXT,
    pagination_state TEXT NOT NULL
);

INSERT INTO arrangement_rebuild (
    id, song_id, title, rendition_attribution, created_at, archived_at, pagination_state
)
SELECT id, song_id, title, rendition_attribution, created_at, archived_at, pagination_state
FROM arrangement;

DROP TABLE arrangement;

CREATE TABLE arrangement (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    song_id TEXT NOT NULL REFERENCES song (id),
    title TEXT CHECK (title IS NULL OR length(trim(title)) > 0),
    notes TEXT CHECK (notes IS NULL OR length(trim(notes)) > 0),
    rendition_attribution TEXT,
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    archived_at TEXT CHECK (archived_at IS NULL OR (archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', archived_at) IS archived_at AND substr(archived_at, 12, 2) <= '23')),
    pagination_state TEXT NOT NULL DEFAULT 'draft' CHECK (pagination_state IN ('draft', 'approved', 'needs_review'))
);

CREATE INDEX arrangement_by_song ON arrangement (song_id);

INSERT INTO arrangement (
    id, song_id, title, notes, rendition_attribution, created_at, archived_at, pagination_state
)
SELECT id, song_id, title, NULL, rendition_attribution, created_at, archived_at, pagination_state
FROM arrangement_rebuild;

DROP TABLE arrangement_rebuild;

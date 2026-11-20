-- Edit revisions (T2.2). Replaces the T2.1 placeholder, which no code wrote.
-- One linear chain per entity: a revision's parent is the entity's previous
-- latest revision, and no revision has two children (UNIQUE parent_id).
DROP TABLE edit_revision;

CREATE TABLE edit_revision (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    song_id TEXT REFERENCES song (id),
    arrangement_id TEXT REFERENCES arrangement (id),
    parent_id TEXT UNIQUE REFERENCES edit_revision (id),
    kind TEXT NOT NULL CHECK (kind IN ('creation', 'edit', 'import', 'replacement', 'restoration')),
    session_id TEXT CHECK (session_id IS NULL OR (length(session_id) = 36 AND session_id NOT GLOB '*[^0-9a-f-]*' AND length(replace(session_id, '-', '')) = 32 AND substr(session_id, 9, 1) = '-' AND substr(session_id, 14, 1) = '-' AND substr(session_id, 19, 1) = '-' AND substr(session_id, 24, 1) = '-' AND substr(session_id, 15, 1) = '4' AND substr(session_id, 20, 1) IN ('8', '9', 'a', 'b'))),
    restored_from_id TEXT REFERENCES edit_revision (id),
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    CHECK ((song_id IS NULL) <> (arrangement_id IS NULL)),
    CHECK ((kind = 'creation') = (parent_id IS NULL)),
    CHECK ((kind = 'edit') = (session_id IS NOT NULL)),
    CHECK ((kind = 'restoration') = (restored_from_id IS NOT NULL))
);

CREATE INDEX edit_revision_by_song ON edit_revision (song_id, created_at);
CREATE INDEX edit_revision_by_arrangement ON edit_revision (arrangement_id, created_at);
CREATE INDEX edit_revision_by_restored_from ON edit_revision (restored_from_id);

-- SQLite lets many rows hold a NULL parent_id under UNIQUE, so one root per
-- entity needs these partial unique indexes.
CREATE UNIQUE INDEX edit_revision_one_song_root ON edit_revision (song_id) WHERE parent_id IS NULL;
CREATE UNIQUE INDEX edit_revision_one_arrangement_root ON edit_revision (arrangement_id) WHERE parent_id IS NULL;

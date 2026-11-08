-- Parish foundation (T2.1). Identities are canonical lowercase UUIDv4 text,
-- times are canonical UTC text YYYY-MM-DDTHH:MM:SS.sssZ (domain::identity).
-- The database file is the Parish scope: one parish row, no parish_id columns.
-- No foreign key cascades; archive is non-cascading and deletion is checked by
-- the Storage Module (Decision 43).

CREATE TABLE schema_metadata (
    key TEXT PRIMARY KEY CHECK (key IN ('format')),
    value TEXT NOT NULL
) WITHOUT ROWID;

INSERT INTO schema_metadata (key, value) VALUES ('format', 'ordo-parish');

CREATE TABLE parish (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    singleton INTEGER NOT NULL DEFAULT 1 UNIQUE CHECK (singleton = 1),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23')
);

CREATE TABLE parish_setting (
    key TEXT PRIMARY KEY CHECK (length(key) > 0),
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', updated_at) IS updated_at AND substr(updated_at, 12, 2) <= '23')
) WITHOUT ROWID;

CREATE TABLE choir (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    archived_at TEXT CHECK (archived_at IS NULL OR (archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', archived_at) IS archived_at AND substr(archived_at, 12, 2) <= '23'))
);

CREATE TABLE song (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    original_attribution TEXT,
    provenance TEXT,
    rights_notes TEXT,
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    archived_at TEXT CHECK (archived_at IS NULL OR (archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', archived_at) IS archived_at AND substr(archived_at, 12, 2) <= '23'))
);

CREATE TABLE song_alias (
    song_id TEXT NOT NULL REFERENCES song (id),
    position INTEGER NOT NULL CHECK (position >= 0),
    alias TEXT NOT NULL CHECK (length(trim(alias)) > 0),
    PRIMARY KEY (song_id, position)
) WITHOUT ROWID;

CREATE TABLE song_liturgical_moment (
    song_id TEXT NOT NULL REFERENCES song (id),
    moment TEXT NOT NULL CHECK (length(trim(moment)) > 0),
    PRIMARY KEY (song_id, moment)
) WITHOUT ROWID;

CREATE TABLE song_occasion (
    song_id TEXT NOT NULL REFERENCES song (id),
    occasion TEXT NOT NULL CHECK (length(trim(occasion)) > 0),
    PRIMARY KEY (song_id, occasion)
) WITHOUT ROWID;

CREATE TABLE arrangement (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    song_id TEXT NOT NULL REFERENCES song (id),
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    rendition_attribution TEXT,
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    archived_at TEXT CHECK (archived_at IS NULL OR (archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', archived_at) IS archived_at AND substr(archived_at, 12, 2) <= '23'))
);

CREATE INDEX arrangement_by_song ON arrangement (song_id);

CREATE TABLE arrangement_language (
    arrangement_id TEXT NOT NULL REFERENCES arrangement (id),
    position INTEGER NOT NULL CHECK (position >= 0),
    language TEXT NOT NULL CHECK (length(trim(language)) > 0),
    PRIMARY KEY (arrangement_id, position),
    UNIQUE (arrangement_id, language)
) WITHOUT ROWID;

CREATE TABLE arrangement_choir (
    arrangement_id TEXT NOT NULL REFERENCES arrangement (id),
    choir_id TEXT NOT NULL REFERENCES choir (id),
    PRIMARY KEY (arrangement_id, choir_id)
) WITHOUT ROWID;

CREATE INDEX arrangement_choir_by_choir ON arrangement_choir (choir_id);

CREATE TABLE arrangement_occasion (
    arrangement_id TEXT NOT NULL REFERENCES arrangement (id),
    occasion TEXT NOT NULL CHECK (length(trim(occasion)) > 0),
    PRIMARY KEY (arrangement_id, occasion)
) WITHOUT ROWID;

CREATE TABLE arrangement_section (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    arrangement_id TEXT NOT NULL REFERENCES arrangement (id),
    position INTEGER NOT NULL CHECK (position >= 0),
    label TEXT CHECK (label IS NULL OR length(trim(label)) > 0),
    lyrics TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    UNIQUE (id, arrangement_id),
    UNIQUE (arrangement_id, position)
);

CREATE TABLE section_occurrence (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    arrangement_id TEXT NOT NULL REFERENCES arrangement (id),
    section_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    FOREIGN KEY (section_id, arrangement_id) REFERENCES arrangement_section (id, arrangement_id),
    UNIQUE (arrangement_id, position)
);

CREATE INDEX section_occurrence_by_section ON section_occurrence (section_id, arrangement_id);

CREATE TABLE managed_asset (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    media_type TEXT NOT NULL CHECK (media_type GLOB '?*/?*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23')
);

CREATE TABLE theme (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    background_asset_id TEXT REFERENCES managed_asset (id),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    archived_at TEXT CHECK (archived_at IS NULL OR (archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', archived_at) IS archived_at AND substr(archived_at, 12, 2) <= '23'))
);

CREATE INDEX theme_by_background_asset ON theme (background_asset_id);

CREATE TABLE edit_revision (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    song_id TEXT REFERENCES song (id),
    arrangement_id TEXT REFERENCES arrangement (id),
    parent_id TEXT REFERENCES edit_revision (id),
    content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23'),
    CHECK ((song_id IS NULL) <> (arrangement_id IS NULL))
);

CREATE INDEX edit_revision_by_song ON edit_revision (song_id);
CREATE INDEX edit_revision_by_arrangement ON edit_revision (arrangement_id);
CREATE INDEX edit_revision_by_parent ON edit_revision (parent_id);

CREATE TABLE presentation_snapshot (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23')
);

CREATE TABLE celebration_package (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23')
);

CREATE TABLE parish_backup (
    id TEXT NOT NULL PRIMARY KEY CHECK (length(id) = 36 AND id NOT GLOB '*[^0-9a-f-]*' AND length(replace(id, '-', '')) = 32 AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-' AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-' AND substr(id, 15, 1) = '4' AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')),
    manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at TEXT NOT NULL CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%fZ', created_at) IS created_at AND substr(created_at, 12, 2) <= '23')
);

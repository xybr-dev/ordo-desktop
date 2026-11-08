-- T2.1 parish foundation.
--
-- All durable entities use canonical lowercase UUIDv4 text ids (Decision 38).
-- The id CHECK enforces the exact 36-character spelling: only lowercase hex
-- and hyphens, exactly four hyphens at the separator positions, version nibble
-- 4, and an RFC 4122 variant nibble. Times use one canonical UTC text form so
-- SQL inspection stays readable.
--
-- Foreign keys never cascade (Decision 43): archival is explicit, and a
-- deletion that would orphan a retained record is refused.

CREATE TABLE parish (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    name TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    singleton INTEGER NOT NULL DEFAULT 1 UNIQUE CHECK (singleton = 1)
);

CREATE TABLE parish_setting (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    setting_key TEXT NOT NULL UNIQUE,
    setting_value TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE choir (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    name TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE song (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    title TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE song_alias (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    song_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (song_id, alias),
    FOREIGN KEY (song_id) REFERENCES song (id) ON DELETE RESTRICT
);

CREATE TABLE song_liturgical_moment (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    song_id TEXT NOT NULL,
    moment TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (song_id, moment),
    FOREIGN KEY (song_id) REFERENCES song (id) ON DELETE RESTRICT
);

CREATE TABLE song_occasion (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    song_id TEXT NOT NULL,
    occasion TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (song_id, occasion),
    FOREIGN KEY (song_id) REFERENCES song (id) ON DELETE RESTRICT
);

CREATE TABLE arrangement (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    song_id TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    FOREIGN KEY (song_id) REFERENCES song (id) ON DELETE RESTRICT
);

CREATE TABLE arrangement_language (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    arrangement_id TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (arrangement_id, language),
    FOREIGN KEY (arrangement_id) REFERENCES arrangement (id) ON DELETE RESTRICT
);

CREATE TABLE arrangement_choir (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    arrangement_id TEXT NOT NULL,
    choir_id TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (arrangement_id, choir_id),
    FOREIGN KEY (arrangement_id) REFERENCES arrangement (id) ON DELETE RESTRICT,
    FOREIGN KEY (choir_id) REFERENCES choir (id) ON DELETE RESTRICT
);

CREATE TABLE arrangement_occasion (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    arrangement_id TEXT NOT NULL,
    occasion TEXT NOT NULL,
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (arrangement_id, occasion),
    FOREIGN KEY (arrangement_id) REFERENCES arrangement (id) ON DELETE RESTRICT
);

CREATE TABLE arrangement_section (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    arrangement_id TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (id, arrangement_id),
    FOREIGN KEY (arrangement_id) REFERENCES arrangement (id) ON DELETE RESTRICT
);

CREATE TABLE section_occurrence (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    arrangement_id TEXT NOT NULL,
    section_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    UNIQUE (arrangement_id, position),
    FOREIGN KEY (section_id, arrangement_id)
        REFERENCES arrangement_section (id, arrangement_id)
        ON DELETE RESTRICT
);

CREATE TABLE managed_asset (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    sha256 TEXT NOT NULL CHECK (
        length(sha256) = 64
        AND sha256 NOT GLOB '*[^0-9a-f]*'
    ),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    media_type TEXT NOT NULL CHECK (length(media_type) > 0),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE theme (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    name TEXT NOT NULL,
    background_asset_id TEXT CHECK (
        background_asset_id IS NULL
        OR (
            length(background_asset_id) = 36
            AND background_asset_id NOT GLOB '*[^0-9a-f-]*'
            AND length(replace(background_asset_id, '-', '')) = 32
            AND substr(background_asset_id, 9, 1) = '-'
            AND substr(background_asset_id, 14, 1) = '-'
            AND substr(background_asset_id, 15, 1) = '4'
            AND substr(background_asset_id, 19, 1) = '-'
            AND substr(background_asset_id, 20, 1) IN ('8', '9', 'a', 'b')
            AND substr(background_asset_id, 24, 1) = '-'
        )
    ),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    FOREIGN KEY (background_asset_id) REFERENCES managed_asset (id) ON DELETE RESTRICT
);

CREATE TABLE edit_revision (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL CHECK (
        length(entity_id) = 36
        AND entity_id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(entity_id, '-', '')) = 32
        AND substr(entity_id, 9, 1) = '-'
        AND substr(entity_id, 14, 1) = '-'
        AND substr(entity_id, 15, 1) = '4'
        AND substr(entity_id, 19, 1) = '-'
        AND substr(entity_id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(entity_id, 24, 1) = '-'
    ),
    parent_id TEXT CHECK (
        parent_id IS NULL
        OR (
            length(parent_id) = 36
            AND parent_id NOT GLOB '*[^0-9a-f-]*'
            AND length(replace(parent_id, '-', '')) = 32
            AND substr(parent_id, 9, 1) = '-'
            AND substr(parent_id, 14, 1) = '-'
            AND substr(parent_id, 15, 1) = '4'
            AND substr(parent_id, 19, 1) = '-'
            AND substr(parent_id, 20, 1) IN ('8', '9', 'a', 'b')
            AND substr(parent_id, 24, 1) = '-'
        )
    ),
    content_hash TEXT NOT NULL CHECK (
        length(content_hash) = 64
        AND content_hash NOT GLOB '*[^0-9a-f]*'
    ),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    ),
    FOREIGN KEY (parent_id) REFERENCES edit_revision (id) ON DELETE RESTRICT
);

CREATE TABLE presentation_snapshot (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE celebration_package (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE parish_backup (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        length(id) = 36
        AND id NOT GLOB '*[^0-9a-f-]*'
        AND length(replace(id, '-', '')) = 32
        AND substr(id, 9, 1) = '-'
        AND substr(id, 14, 1) = '-'
        AND substr(id, 15, 1) = '4'
        AND substr(id, 19, 1) = '-'
        AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
        AND substr(id, 24, 1) = '-'
    ),
    created_at TEXT NOT NULL CHECK (
        created_at GLOB '[0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9].[0-9][0-9][0-9]Z'
    )
);

CREATE TABLE schema_metadata (
    format_name TEXT PRIMARY KEY NOT NULL,
    format_version INTEGER NOT NULL CHECK (format_version >= 1)
) WITHOUT ROWID;

INSERT INTO schema_metadata (format_name, format_version) VALUES ('ordo-parish', 1);

CREATE INDEX song_alias_song_id_idx ON song_alias (song_id);
CREATE INDEX song_liturgical_moment_song_id_idx ON song_liturgical_moment (song_id);
CREATE INDEX song_occasion_song_id_idx ON song_occasion (song_id);
CREATE INDEX arrangement_song_id_idx ON arrangement (song_id);
CREATE INDEX arrangement_language_arrangement_id_idx ON arrangement_language (arrangement_id);
CREATE INDEX arrangement_choir_choir_id_idx ON arrangement_choir (choir_id);
CREATE INDEX arrangement_occasion_arrangement_id_idx ON arrangement_occasion (arrangement_id);
CREATE INDEX arrangement_section_arrangement_id_idx ON arrangement_section (arrangement_id);
CREATE INDEX section_occurrence_section_id_idx ON section_occurrence (section_id);
CREATE INDEX theme_background_asset_id_idx ON theme (background_asset_id);
CREATE INDEX edit_revision_entity_idx ON edit_revision (entity_kind, entity_id);
CREATE INDEX edit_revision_parent_id_idx ON edit_revision (parent_id);

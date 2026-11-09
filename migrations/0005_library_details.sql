-- Library details (T2.4). Choirs gain optional details and preferred
-- languages; Arrangements gain a pagination state. Existing rows keep working:
-- details are NULL and pagination defaults to 'draft'.

ALTER TABLE choir ADD COLUMN short_label TEXT CHECK (short_label IS NULL OR length(trim(short_label)) > 0);

ALTER TABLE choir ADD COLUMN notes TEXT CHECK (notes IS NULL OR length(trim(notes)) > 0);

CREATE TABLE choir_language (
    choir_id TEXT NOT NULL REFERENCES choir (id),
    position INTEGER NOT NULL CHECK (position >= 0),
    language TEXT NOT NULL CHECK (length(trim(language)) > 0),
    PRIMARY KEY (choir_id, position),
    UNIQUE (choir_id, language)
) WITHOUT ROWID;

ALTER TABLE arrangement ADD COLUMN pagination_state TEXT NOT NULL DEFAULT 'draft' CHECK (pagination_state IN ('draft', 'approved', 'needs_review'));

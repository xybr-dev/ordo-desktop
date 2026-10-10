CREATE TABLE parish_note (
    id INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES parish_profile (id),
    body TEXT NOT NULL
);

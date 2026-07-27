CREATE TABLE IF NOT EXISTS profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    display_name TEXT NOT NULL,
    tagline_ko TEXT,
    tagline_en TEXT,
    avatar_url TEXT,
    bio_ko TEXT,
    bio_en TEXT,
    email TEXT,
    github_username TEXT,
    linkedin_url TEXT,
    education JSON NOT NULL DEFAULT '[]',
    custom_links JSON NOT NULL DEFAULT '[]',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

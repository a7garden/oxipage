-- doc/02 §2.6 blog_post
CREATE TABLE IF NOT EXISTS blog_post (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    lang TEXT NOT NULL DEFAULT 'ko' CHECK (lang IN ('ko','en')),
    translation_group_id INTEGER,
    tags JSON NOT NULL DEFAULT '[]',
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (translation_group_id) REFERENCES blog_post(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_blog_post_published ON blog_post(published_at);
CREATE INDEX IF NOT EXISTS idx_blog_post_lang ON blog_post(lang);

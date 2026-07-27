-- doc/02 §2.5 novels (Novel + NovelChapter)
CREATE TABLE IF NOT EXISTS novel (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    synopsis TEXT,
    cover_image TEXT,
    status TEXT NOT NULL DEFAULT 'ongoing' CHECK (status IN ('ongoing','completed','hiatus')),
    tags JSON NOT NULL DEFAULT '[]',
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_novel_published ON novel(published_at);

-- NovelChapter: order는 일관성을 위해 chapter_order 사용.
-- char_count: 공백 제외 자수 (한국어 word count 불규칙).
CREATE TABLE IF NOT EXISTS novel_chapter (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    novel_id INTEGER NOT NULL REFERENCES novel(id) ON DELETE CASCADE,
    chapter_order INTEGER NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    char_count INTEGER NOT NULL DEFAULT 0,
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (novel_id, chapter_order)
);
CREATE INDEX IF NOT EXISTS idx_chapter_novel ON novel_chapter(novel_id, chapter_order);
CREATE INDEX IF NOT EXISTS idx_chapter_published ON novel_chapter(published_at);

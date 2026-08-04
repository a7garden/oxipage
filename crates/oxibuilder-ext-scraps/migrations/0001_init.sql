-- doc/02 §2.7 scraps
-- 백그라운드 잡이 채우는 추천 큐(published_at IS NULL)와 사용자가 발행한 본문(published_at IS NOT NULL).
-- source='manual' 이거나 publish API로 published_at이 세팅되면 공개. 동일 source+source_item_id는 백그라운드 upsert 시 중복 회피.
CREATE TABLE IF NOT EXISTS scrap_item (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL CHECK (source IN ('hackernews','geeknews','manual')),
    source_item_id TEXT,
    source_url TEXT NOT NULL,
    title TEXT NOT NULL,
    og_image_url TEXT,
    note_ko TEXT,
    note_en TEXT,
    tags JSON NOT NULL DEFAULT '[]',
    scraped_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE (source, source_item_id)
);
CREATE INDEX IF NOT EXISTS idx_scrap_published ON scrap_item(published_at);
CREATE INDEX IF NOT EXISTS idx_scrap_source ON scrap_item(source, scraped_at);
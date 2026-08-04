-- doc/02 §2.11 link_card. `order`는 SQL 예약어 → display_order.
CREATE TABLE IF NOT EXISTS link_card (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    description_ko TEXT,
    description_en TEXT,
    thumbnail_url TEXT,
    tags JSON NOT NULL DEFAULT '[]',
    display_order INTEGER NOT NULL DEFAULT 0,
    featured INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_link_card_order ON link_card(display_order);
CREATE INDEX IF NOT EXISTS idx_link_card_featured ON link_card(featured);

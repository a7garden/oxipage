-- doc/02 §2.10 books
-- rating: 0~10 정수 (0.5점 단위) (oxipage_core::rating 계약).
-- review_ko/review_en: 마크다운 본문, 발행 시 FTS 본문으로 사용.
-- source: 'aladin' | 'google_books' | 'open_library' | 'manual'.
-- status: 'wishlist' | 'reading' | 'completed' | 'dropped'.
-- display_order: SQL `order` 예약어 회피 (메모리 결정).
CREATE TABLE IF NOT EXISTS book_entry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('aladin','google_books','open_library','manual')),
    external_id TEXT,
    isbn13 TEXT,
    title TEXT NOT NULL,
    author TEXT,
    cover_image_url TEXT,
    rating INTEGER NOT NULL DEFAULT 0,
    review_ko TEXT,
    review_en TEXT,
    status TEXT NOT NULL DEFAULT 'wishlist' CHECK (status IN ('wishlist','reading','completed','dropped')),
    started_at TEXT,
    finished_at TEXT,
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_book_published ON book_entry(published_at);
CREATE INDEX IF NOT EXISTS idx_book_status ON book_entry(status);
CREATE INDEX IF NOT EXISTS idx_book_isbn ON book_entry(isbn13);

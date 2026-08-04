-- doc/02 §2.9 movies (MovieEntry + SeriesGroup)
--
-- 개별 작품 평가와 시리즈 묶음 평가는 독립 공존:
-- movie_entry.series_group_id로 묶음에 옵트인. NULL이면 묶음 없음.
--
-- `display_order`/`series_order` 컬럼은 SQLite에서 `order`가 예약어/함수명이라
-- 우회. 시리즈 내 정렬(개봉 순 등)용.
--
-- `rating` / `group_rating`은 Rating 값객체(0~10 정수)로 저장.

CREATE TABLE IF NOT EXISTS series_group (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title_ko TEXT,
    title_en TEXT,
    cover_image TEXT,
    group_rating INTEGER CHECK (group_rating IS NULL OR (group_rating BETWEEN 0 AND 10)),
    group_review_ko TEXT,
    group_review_en TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS movie_entry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    tmdb_id INTEGER,
    media_type TEXT NOT NULL CHECK (media_type IN ('movie','tv')),
    title TEXT NOT NULL,
    poster_path TEXT,
    release_year INTEGER,
    watched_at TEXT,
    rating INTEGER NOT NULL DEFAULT 0 CHECK (rating BETWEEN 0 AND 10),
    review_ko TEXT,
    review_en TEXT,
    rewatch INTEGER NOT NULL DEFAULT 0 CHECK (rewatch IN (0,1)),
    series_group_id INTEGER,
    series_order INTEGER,
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (series_group_id) REFERENCES series_group(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_movie_entry_published ON movie_entry(published_at);
CREATE INDEX IF NOT EXISTS idx_movie_entry_series_group ON movie_entry(series_group_id, series_order);
CREATE INDEX IF NOT EXISTS idx_movie_entry_tmdb ON movie_entry(tmdb_id);

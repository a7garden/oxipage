-- 0002: bilingual titles + runtime + genres + cast/directors (doc/02 §2.9)
--
-- 한국어가 기본이되 영문 폴백을 갖는 이중언어 제목, 런타임, 장르, 출연진/감독.
-- TMDB ko-KR/en-US 메타로 자동 채운다 (수동 입력도 가능).
--
-- `title` 은 NOT NULL 캐노니컬(슬러그/FTS 원천)로 유지. 표시 계층은
-- title_ko/title_en 을 우선하고 없으면 title 로 폴백. 기존 행은 ko-KR 로
-- 페칭됐으므로 title_ko := title 로 백필.
--
-- 장르/인물은 name_en 을 정규키로 정규화: TMDB 는 항상 en 이름을 주므로.

ALTER TABLE movie_entry ADD COLUMN title_ko TEXT;
ALTER TABLE movie_entry ADD COLUMN title_en TEXT;
ALTER TABLE movie_entry ADD COLUMN runtime_min INTEGER;

-- 기존 행 백필 (저장본은 ko-KR 기준).
UPDATE movie_entry SET title_ko = title WHERE title_ko IS NULL;

CREATE TABLE IF NOT EXISTS movie_genre (
    movie_entry_id INTEGER NOT NULL,
    name_en TEXT NOT NULL,
    name_ko TEXT,
    PRIMARY KEY (movie_entry_id, name_en),
    FOREIGN KEY (movie_entry_id) REFERENCES movie_entry(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS movie_person (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_person_id INTEGER UNIQUE,
    slug TEXT NOT NULL UNIQUE,
    name_en TEXT NOT NULL,
    name_ko TEXT,
    profile_path TEXT,
    role TEXT NOT NULL DEFAULT 'actor',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS movie_entry_person (
    movie_entry_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL,
    character_name TEXT,
    billing INTEGER,
    PRIMARY KEY (movie_entry_id, person_id),
    FOREIGN KEY (movie_entry_id) REFERENCES movie_entry(id) ON DELETE CASCADE,
    FOREIGN KEY (person_id) REFERENCES movie_person(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_movie_genre_name ON movie_genre(name_en);
CREATE INDEX IF NOT EXISTS idx_movie_person_role ON movie_person(role);
CREATE INDEX IF NOT EXISTS idx_movie_entry_person_person ON movie_entry_person(person_id);
CREATE INDEX IF NOT EXISTS idx_movie_entry_person_billing ON movie_entry_person(movie_entry_id, billing);

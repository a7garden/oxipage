-- Theme configuration (doc/12 §12.7).
-- 싱글턴 행 (id=1), 각 사이트 DB에 존재.

CREATE TABLE IF NOT EXISTS theme_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    theme_id TEXT NOT NULL DEFAULT 'paper',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 기본 테마 시드
INSERT OR IGNORE INTO theme_config (id, theme_id) VALUES (1, 'paper');

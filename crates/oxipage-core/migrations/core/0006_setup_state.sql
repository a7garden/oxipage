-- setup_state: 첫 부팅 UX 마법사의 상태 저장 (doc/13).
-- id=1 singleton. setup_completed_at IS NULL = setup 모드 진행 중.
-- admin_password_hash는 Argon2id. PAT는 auth_token 테이블에 별도 저장.
CREATE TABLE IF NOT EXISTS setup_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    setup_completed_at TEXT,
    admin_password_hash TEXT,
    site_name TEXT,
    base_url TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

INSERT OR IGNORE INTO setup_state (id) VALUES (1);

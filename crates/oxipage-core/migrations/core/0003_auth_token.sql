-- doc/01 §1.8, doc/04 §4.2 PAT (Personal Access Token).
-- 평문은 ≥256비트 난수, 저장은 SHA-256 해시(hex). 빠른 해시 — 쓰기 API마다
-- 조회 비용 누적되므로 느린 해시는 DoS 자초.
CREATE TABLE IF NOT EXISTS auth_token (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    -- SHA-256(평문) hex 64자. 평문은 발급 시 1회만 반환.
    token_hash TEXT NOT NULL UNIQUE,
    -- token_id_prefix: 평문의 앞 8자 (표시용 마스킹 식별자). "oxp_<prefix>…" 형태.
    token_prefix TEXT NOT NULL,
    -- scopes JSON 배열: ["post:write","post:publish","read", ...].
    scopes JSON NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    last_used_at TEXT,
    revoked_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_auth_token_hash ON auth_token(token_hash) WHERE revoked_at IS NULL;

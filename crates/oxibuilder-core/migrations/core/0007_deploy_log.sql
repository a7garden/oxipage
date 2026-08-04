-- Durable GitHub Pages deploy history (per-site DB).
CREATE TABLE IF NOT EXISTS deploy_log(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    build_id TEXT NOT NULL,
    target TEXT NOT NULL,
    owner TEXT NOT NULL,
    repo TEXT NOT NULL,
    branch TEXT NOT NULL,
    base_path TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN('running','deployed','unchanged','failed')),
    url TEXT,
    commit_sha TEXT,
    error_code TEXT,
    error TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT
);
CREATE INDEX IF NOT EXISTS deploy_log_started_at_idx ON deploy_log(started_at DESC);

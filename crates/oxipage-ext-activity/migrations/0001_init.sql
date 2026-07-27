-- doc/02 §2.8 activity_event: read-only cache of public GitHub activity.
CREATE TABLE IF NOT EXISTS activity_event (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_full_name TEXT NOT NULL,
    event_type TEXT NOT NULL,
    summary TEXT NOT NULL,
    url TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    UNIQUE(repo_full_name, event_type, url, occurred_at)
);
CREATE INDEX IF NOT EXISTS idx_activity_event_occurred_at ON activity_event(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_event_repo_occurred ON activity_event(repo_full_name, occurred_at DESC);

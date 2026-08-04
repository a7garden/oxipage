-- doc/02 §2.4 project (구조적 이중언어 강제)
CREATE TABLE IF NOT EXISTS project (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title_ko TEXT,
    title_en TEXT,
    description_ko TEXT,
    description_en TEXT,
    tech_stack JSON NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'wip' CHECK (status IN ('active','archived','wip')),
    started_at TEXT,
    ended_at TEXT,
    links JSON NOT NULL DEFAULT '{}',
    featured INTEGER NOT NULL DEFAULT 0,
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    CHECK (title_ko IS NOT NULL OR title_en IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS idx_project_published ON project(published_at);
CREATE INDEX IF NOT EXISTS idx_project_status ON project(status);

-- doc/02 §2.4 screenshot (순서 있는 갤러리). `order`는 SQL 예약어 → display_order.
CREATE TABLE IF NOT EXISTS screenshot (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    alt_ko TEXT,
    alt_en TEXT,
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_screenshot_project ON screenshot(project_id, display_order);

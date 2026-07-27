CREATE TABLE IF NOT EXISTS lobby_config (
    extension_id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    display_mode TEXT NOT NULL DEFAULT 'grid' CHECK (display_mode IN ('canvas', 'grid', 'list')),
    display_order INTEGER NOT NULL DEFAULT 0,
    style_params JSON NOT NULL DEFAULT '{}'
);

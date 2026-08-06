-- Layout variant axis (docs/superpowers/specs/2026-08-06-editorial-layout-variant-design.md §2).
-- Orthogonal to theme_id. Defaults to 'shell' so existing sites keep their look.
ALTER TABLE theme_config ADD COLUMN layout TEXT NOT NULL DEFAULT 'shell';

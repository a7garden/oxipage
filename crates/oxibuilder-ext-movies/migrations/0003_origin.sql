-- 0003: production country origin (doc/02 §2.9 parity with blog-test nation stats)
-- Comma-separated ISO-3166 alpha-2 codes ("KR,US"). NULL-safe for existing rows;
-- country data lives in TMDB and is backfilled per-entry via `movies refresh`.
ALTER TABLE movie_entry ADD COLUMN origin TEXT;
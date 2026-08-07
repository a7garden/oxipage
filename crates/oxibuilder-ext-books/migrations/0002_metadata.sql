-- 0002: publisher metadata parity with blog-test book stats.
-- category/publisher/page_count come from Aladin/Google Books; NULL-safe,
-- backfilled per-book via `books refresh`.
ALTER TABLE book_entry ADD COLUMN category TEXT;
ALTER TABLE book_entry ADD COLUMN publisher TEXT;
ALTER TABLE book_entry ADD COLUMN page_count INTEGER;

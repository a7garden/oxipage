# Movies & Books Extensions — blog-test Parity

**Date:** 2026-08-07
**Status:** Draft (pending user review)
**Scope:** `oxibuilder-ext-movies`, `oxibuilder-ext-books`, `oxibuilder-core` (media/build), `web` (stats/cards/detail)

## 1. Background

`../blog-test` (the Astro reference site) and `oxibuilder` both implement
movies and books collections, but with a cross-cutting (not uniform) advantage
split. A direct code comparison established:

| Dimension | blog-test | oxibuilder | Lead |
|---|---|---|---|
| List filters (genre/year/cast/media-type/sort) | search + genre chips | full faceted filter | **oxibuilder** |
| Movie detail page | none | `MovieDetailPage` | **oxibuilder** |
| i18n (ko/en) | ko-only | full `pick(name_ko, name_en)` | **oxibuilder** |
| Data model | flat JSON | normalized relational (genres/people/series) | **oxibuilder** |
| Authoring UX | manual `.md` + scripts | CLI + Console + TMDB/Aladin search | **oxibuilder** |
| **Stats dimensions** | movie nation; book publisher/category/pages | **omitted** | **blog-test** |
| **Image optimization** | build-time AVIF/WebP/JPEG `<picture>` + eager/lazy + local cache | TMDB CDN single `<img>` | **blog-test** |
| SSG / first paint | pure static | SPA + prerender (WIP) | blog-test |

**Goal:** close the two gaps where blog-test leads, without regressing oxibuilder's
existing strengths. The decision is **full parity on both axes**, image format
**WebP-only** (reuse existing `media.rs` infrastructure), card layout **unchanged**
(horizontal card retained; poster-grid is a separate visual decision).

**Key enabler:** both data sources are *already* integrated in oxibuilder —
movies via TMDB (`OXIBUILDER_TMDB_KEY`), books via Aladin
(`OXIBUILDER_ALADIN_TTBKEY`) with Google Books fallback. The new fields are
fetched from the *same* clients; no new external dependency or API key.

## 2. Track A — Stats Dimensions (data model + backfill)

### 2.1 Movie `origin` (production country)

**Schema** — new migration `oxibuilder-ext-movies/migrations/0003_origin.sql`:
```sql
ALTER TABLE movie_entry ADD COLUMN origin TEXT;
```
Comma-separated ISO-3166 codes (`"KR,US"`), matching blog-test's `nation` shape.
NULL-safe for existing rows (no backfill in SQL; country data only exists in
TMDB, refreshed per-entry — §2.3).

**TMDB client** (`ext-movies/src/integration.rs`):
- Add `origin: Option<String>` to `MovieMeta`.
- Parse `production_countries[].iso_3166_1` from the **ko-KR** detail (TMDB
  returns localized country names there; ISO code is language-independent).
  Fall back to `origin_country` from the en-US detail.
- The `NATION_CODE_TO_KO` map + `normalizeNation()` from blog-test's
  `movieStats.ts` is ported into a shared module the front-end imports, so the
  stats layer can render Korean country names (`US` → `미국`).

**Repo** (`ext-movies/src/repo.rs`): add `origin` to `ENTRY_COLUMNS`,
`create_entry` (new param), `update_entry` (PATCH path), and the input/patch
models (`MovieEntryInput`, `MovieEntryPatch` in `model.rs`).

**Front-end**:
- `MovieEntry` type (`web/src/shared/api.ts`) gains `origin: string | null`.
- `computeMovieStats.ts`: restore the `nations` dimension (port the bucketing
  from blog-test `movieStats.ts`, canonicalized on ISO code with the shared
  `NATION_CODE_TO_KO` map). `MovieStats` gains `nationCount` + `nations: MovieCountRow[]`.
- `MoviesStatsPage.tsx`: add a "국가 / Countries" `BarRow` section + `nationCount`
  in the `SummaryBand`.

### 2.2 Book `category` / `publisher` / `page_count`

**Schema** — new migration `oxibuilder-ext-books/migrations/0002_metadata.sql`:
```sql
ALTER TABLE book_entry ADD COLUMN category TEXT;
ALTER TABLE book_entry ADD COLUMN publisher TEXT;
ALTER TABLE book_entry ADD COLUMN page_count INTEGER;
```
NULL-safe; existing rows untouched (backfill via refresh — §2.3).

**Book client** (`ext-books/src/client.rs`):
- Aladin: parse `categoryName` (→ `category`), `publisher`, and
  `subInfo.itemPage` (→ `page_count`) from `AladinItem`. (Aladin's
  `ItemSearch.aspx` returns these; blog-test's `fetch-books.mjs` proves the
  fields are populated.)
- Google Books: parse `categories[0]`, `publisher`, `pageCount` from
  `GoogleVolumeInfo`.
- `BookSearchResult`, `BookInput`, `BookPatch` (`model.rs`) gain the three fields.

**Repo** (`ext-books/src/repo.rs`): wire the three columns through
create/update/list column lists.

**Front-end**:
- `Book` type gains `category`/`publisher`/`page_count`.
- `computeBookStats.ts`: restore `categories`, `publishers`, `pageBuckets`
  dimensions (port from blog-test `bookStats.ts`). `BookStats` gains those rows.
- `BooksStatsPage.tsx`: add the three `BarRow` sections.
- `BooksPage.tsx`: add a **category chip filter** (top-8 by count, blog-test
  style) alongside the existing status filter / search / sort.

### 2.3 Backfill policy

Migrations are NULL-safe; **no SQL backfill** (the data lives in TMDB/Aladin,
not the local DB). Existing entries with a `tmdb_id` / `isbn13` get refreshed by
re-invoking the existing client:

- **CLI:** `oxibuilder movies refresh [--all | <slug>]` and
  `oxibuilder books refresh [--all | <slug>]` — fetch full meta from TMDB/Aladin
  and PATCH the new fields (and only the new fields; user edits to title/review
  are preserved). Idempotent; skipped when the source key is unset.
- **Console:** a "메타 새로고침 / Refresh metadata" action on the movie/book
  editor, calling the same repo path.

New entries created via search-picker auto-populate the fields (the clients now
return them).

## 3. Track B — External Image Build-Time Optimization (WebP)

### 3.1 Current pipeline (to be extended, not replaced)

```
run_image_pre_pass(pool, media_dir, data_dir)      // build.rs
  → scans blog bodies for `media/...` refs
  → media::optimize(refs, media_dir, staging_dir)   // reads LOCAL files
  → writes staging_dir/media/_derived/{sha8}-{w}.webp + ImageManifest
write_build_output                                  // build_writer.rs step 10b
  → copies staging _derived → out/media/_derived
  → writes out/data/image-manifest.json
web/src/shared/image-manifest.ts                    // SPA
  → isMediaRef(src) = /^\/?media\//                 // ONLY local refs
  → resolveMedia(src) → srcset <img>
```

Three constraints block external URLs today:
1. `media::optimize` reads source bytes from `media_dir` (a local path); it has
   no HTTP fetch path.
2. The pre-pass collects refs only from blog markdown bodies, never from
   extension data.
3. `isMediaRef` matches `media/` exclusively, so the SPA never consults the
   manifest for an external URL.

### 3.2 Changes

**`media.rs` — external-source path.** Add a function (working name
`optimize_external`) that takes `&[(String url)]`, HTTP-downloads each (reuse the
movies `reqwest::Client` timeout discipline: 5s connect / 15s total), and feeds
the bytes into the **existing** `generate()` (SHA-256 → 4-width WebP → cache
lookup). The manifest key is the **external URL verbatim**
(`https://image.tmdb.org/t/p/w500/abc.jpg`). Cache key stays SHA-256(bytes) so
the same poster image served from different URL widths reuses its variants.
Failed downloads are logged + skipped (never error), mirroring the existing
"missing ref skipped" contract.

**Pre-pass — collect external refs.** Extend the build-time ref collection so
the movies extension contributes `poster_path` URLs and the books extension
contributes `cover_image_url` (both only when they are `http(s)` URLs). The
cleanest seam is a new `BuildExt`-adjacent hook (or a small registry) that
returns the extension's external image URLs; `run_image_pre_pass` merges them
with the blog `media/` refs and calls both `optimize` (local) and
`optimize_external`. Concretely, candidates:
- a `fn external_image_urls(&self, pool) -> Vec<String>` on `BuildExt`
  (default empty), implemented by movies/books, OR
- a standalone collector in `oxibuilder-console` that queries both extensions.

The `BuildExt` hook is preferred (keeps the query local to each extension and
generalizes to future image-bearing extensions). Decision deferred to the plan.

**Manifest key namespace.** Local refs keep `media/...` keys; external URLs are
keys as-is. No collision possible (`media/` vs `https://`). `ImageManifest`
(`Record<string, ManifestEntry>`) already supports arbitrary string keys.

**Front-end — broaden the lookup.**
- `image-manifest.ts`: `isMediaRef` becomes `isOptimizableRef` — true for
  `media/...` **or** `https?://`. `resolveMedia` already takes an arbitrary key;
  the only change is the gate predicate and ensuring the external-URL key matches
  what the SPA emits.
- `MovieCard.tsx` / `MovieDetailPage.tsx` / `BookCard.tsx`: instead of emitting
  `image.tmdb.org/...` directly, consult the manifest and emit the local srcset
  when present (fall back to the raw CDN URL in live/preview mode or when the
  manifest has no entry — identical to the markdown rule's existing fallback).
  A small `resolvePoster(url)` / `resolveCover(url)` helper centralizes this.
- **Lazy/eager:** the first N (≈10) grid cards use
  `loading="eager" fetchPriority="high"`; the rest `loading="lazy"`. This matches
  blog-test's `EAGER_COUNT` and is a free LCP win.

### 3.3 Poster dimensions

TMDB posters are 2:3. The existing `WIDTHS = [640, 960, 1280, 1920]` are
landscape-biased for blog photos. Posters displayed at ~80–200px wide don't need
640px+. **Decision:** keep the shared width ladder for simplicity (a 640-wide
poster WebP is still ~30–50KB and cached permanently); do **not** add a poster
-specific width set in this pass. Revisit if build time or payload becomes an
issue. [INFERENCE — payload estimate]

## 4. Scope

**In:** `oxibuilder-ext-movies`, `oxibuilder-ext-books`, `oxibuilder-core`
(`media.rs`, `build.rs` pre-pass, `builder.rs` `BuildExt` hook),
`oxibuilder-cli` (`movies refresh`, `books refresh`), `oxibuilder-console`
(editor refresh action), `web` (`api.ts` types, `compute*Stats`,
`*StatsPage`, `MoviesPage`/`BooksPage` filters, card/detail image resolution,
`image-manifest.ts`).

**Out:**
- Other extensions (novels/scraps/projects/links/activity). The image pipeline
  generalizes, so they can opt in later by implementing the `external_image_urls`
  hook — but no changes this pass.
- Card layout redesign (poster-grid). Horizontal card retained.
- AVIF / JPEG fallback formats. WebP only.
- SSG/prerender improvements (separate WIP track).

## 5. Verification

- **Migrations:** add a core SSG build test asserting a pre-existing DB (without
  the new columns) still builds (mirrors existing
  `crates/oxibuilder-core/tests/ssg_build.rs` pattern). The new columns are
  nullable so old rows must pass through cleanly.
- **Stats units:** extend `computeMovieStats.test.ts` with a fixture asserting
  `nations` bucketing (multi-country split, unknown-nation handling); extend
  `computeBookStats.test.ts` with `categories`/`publishers`/`pageBuckets`.
- **Image pipeline:** add a `media.rs` test feeding a stubbed external URL (or a
  `data:`/local-file-stand-in) through `optimize_external`, asserting the
  manifest key is the URL and variants land in staging. Network calls must be
  injectable for a deterministic test.
- **CLI refresh:** `movies refresh` / `books refresh` smoke against a fixture
  pool (no network when the source key is unset → no-op).
- **Build smoke:** run `oxibuilder build` on a site with at least one movie
  poster + book cover; confirm `out/media/_derived/*.webp` exists and
  `out/data/image-manifest.json` carries the external-URL keys.

## 6. Risks

- **`BuildExt` hook for external images** touches a `Send + Sync` trait shared by
  all extensions. Default-empty impl keeps existing extensions compiling; the
  hook is `fn`-shaped (no state). Low risk, but the plan must verify all
  implementors.
- **Network at build time** is new for the image pass (today it's pure local
  decode). Mitigations: per-request timeout (already proven in movies TMDB
  client), skip-on-failure (existing contract), and the SHA cache means a
  re-build after a transient failure only re-fetches what's missing.
- **TMDB/Aladin field availability** for older entries: some books may lack
  `page_count` in the source; the stats layer already tolerates `null`/empty
  (the `pageBuckets` count is reported as "N books 기준" like blog-test). No
  hard dependency on completeness.

# Movies & Books Stats Parity — Implementation Plan (Track A)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the stat dimensions blog-test has that oxibuilder lacks — movie `origin` (country), book `category`/`publisher`/`page_count` — plus a refresh command to backfill them from the already-integrated TMDB/Aladin/Google clients.

**Architecture:** Schema migrations add nullable columns (NULL-safe for existing rows). The existing TMDB/Aladin/Google clients gain the new fields (no new external dependency). Front-end `compute*Stats` restore the dimensions ported from blog-test; stats pages render them. A `refresh` CLI + Console action re-invokes clients to backfill.

**Tech Stack:** Rust (sqlx, axum, reqwest), SQLite migrations, TypeScript/React (react-query, Vitest).

## Global Constraints

- Migrations are **nullable** — existing rows must pass through untouched (no SQL backfill; country/book metadata only exists in TMDB/Aladin).
- New fields are fetched from the **already-integrated** clients (`OXIBUILDER_TMDB_KEY`, `OXIBUILDER_ALADIN_TTBKEY`, Google Books keyless). No new API keys or crates.
- Commit convention: conventional commits, English (`feat:`, `test:`, `chore:`).
- `cargo test` and `bun test` are the test runners. Rust trait `BuildExt` is `Send + Sync`; any new method needs a default impl.
- Bilingual labels follow the existing `pick(name_ko, name_en)` pattern.

---

## File Structure

**Rust — movies:**
- Create: `crates/oxibuilder-ext-movies/migrations/0003_origin.sql`
- Modify: `crates/oxibuilder-ext-movies/src/model.rs` (`MovieEntry`, `MovieEntryInput`, `MovieEntryPatch`)
- Modify: `crates/oxibuilder-ext-movies/src/integration.rs` (`MovieMeta`, TMDB parse)
- Modify: `crates/oxibuilder-ext-movies/src/repo.rs` (`ENTRY_COLUMNS`, `create_entry`, `update_entry`)
- Modify: `crates/oxibuilder-ext-movies/src/routes.rs` (`create`, `update` meta-merge)

**Rust — books:**
- Create: `crates/oxibuilder-ext-books/migrations/0002_metadata.sql`
- Modify: `crates/oxibuilder-ext-books/src/model.rs` (`Book`, `BookSearchResult`, `BookInput`, `BookPatch`)
- Modify: `crates/oxibuilder-ext-books/src/client.rs` (`AladinItem`, `GoogleVolumeInfo`, `BookSearchResult` mapping)
- Modify: `crates/oxibuilder-ext-books/src/repo.rs` (`COLUMNS`, `create`, `update`)

**Front-end:**
- Modify: `web/src/shared/api.ts` (`MovieEntry.origin`, `Book.category/publisher/page_count`)
- Create: `web/src/shared/nations.ts` (shared `NATION_CODE_TO_KO` + `normalizeNations`)
- Modify: `web/src/shared/stats/computeMovieStats.ts`, `web/src/extensions/movies/MoviesStatsPage.tsx`
- Modify: `web/src/shared/stats/computeBookStats.ts`, `web/src/extensions/books/BooksStatsPage.tsx`, `web/src/extensions/books/BooksPage.tsx`

**CLI/Console:**
- Modify: `crates/oxibuilder-cli/src/commands/` (new `refresh` subcommand wiring)
- Modify: `crates/oxibuilder-ext-movies/src/routes.rs`, `crates/oxibuilder-ext-books/src/routes.rs` (refresh endpoints)

---

### Task 1: Movie `origin` column + model + TMDB parse + repo/routes

**Files:**
- Create: `crates/oxibuilder-ext-movies/migrations/0003_origin.sql`
- Modify: `crates/oxibuilder-ext-movies/src/model.rs`, `integration.rs`, `repo.rs`, `routes.rs`

**Interfaces:**
- Produces: `MovieEntry.origin: Option<String>` (comma-separated ISO-3166 codes, e.g. `"KR,US"`); `MovieEntryInput.origin`/`MovieEntryPatch.origin: Option<String>`; `MovieMeta.origin: Option<String>`.

- [ ] **Step 1: Write the migration**

Create `crates/oxibuilder-ext-movies/migrations/0003_origin.sql`:
```sql
-- 0003: production country origin (doc/02 §2.9 parity with blog-test nation stats)
-- Comma-separated ISO-3166 alpha-2 codes ("KR,US"). NULL-safe for existing rows;
-- country data lives in TMDB and is backfilled per-entry via `movies refresh`.
ALTER TABLE movie_entry ADD COLUMN origin TEXT;
```

- [ ] **Step 2: Add the field to the Rust model**

In `model.rs` `MovieEntry` struct, add after `poster_path`:
```rust
/// 콤마 구분 ISO-3166 alpha-2 ("KR,US"). TMDB production_countries 기반.
pub origin: Option<String>,
```
Add `pub origin: Option<String>` to `MovieEntryInput` and `MovieEntryPatch` (both `#[derive(Deserialize)]`; PATCH is already `Default`).

- [ ] **Step 3: Extend the TMDB client**

In `integration.rs`:
- Add `pub origin: Option<String>` to `MovieMeta` (line ~31, the struct at 26-35).
- Add `production_countries: Option<Vec<CountryCode>>` to `MovieDetailKo` (the ko-KR detail DTO, ~307-318). `CountryCode` is a new Deserialize struct:
  ```rust
  #[derive(Debug, Deserialize)]
  struct CountryCode { iso_3166_1: Option<String> }
  ```
- In `fetch_movie_full` (104-247), after assembling ko/en, collect origin from ko-KR `production_countries` first, fall back to en-US. Join the ISO codes with `,`:
  ```rust
  let origin = ko.as_ref()
      .and_then(|k| k.production_countries.as_ref())
      .filter(|v| !v.is_empty())
      .or_else(|| en.as_ref().and_then(|e| e.production_countries.as_ref()))
      .map(|v| v.iter()
          .filter_map(|c| c.iso_3166_1.clone().filter(|s| !s.is_empty()))
          .collect::<Vec<_>>().join(","))
      .filter(|s| !s.is_empty());
  ```
  Set it on the returned `MovieMeta`.

- [ ] **Step 4: Wire origin through repo**

In `repo.rs`:
- Add `origin` to the `ENTRY_COLUMNS` string constant (after `poster_path`).
- `create_entry` (line 90): add `origin: Option<String>` param; add `origin` to the INSERT column list and `?N` placeholders, and `.bind(origin.as_deref())`.
- `update_entry` (385-510): add an `origin` branch to the dynamic SET builder — when `patch.origin` is `Some`, push `"origin = ?"` and bind it (mirror the existing `runtime_min`/`poster_path` PATCH handling).

- [ ] **Step 5: Wire origin through routes**

In `routes.rs` `create` (30-165): after the `runtime_min` resolution (~114-116), add:
```rust
let origin = input.origin.clone().or_else(|| meta.as_ref().and_then(|m| m.origin.clone()));
```
Pass `origin` as the new last argument to `repo::create_entry(...)` (145-156).
In `update` (182-220): `patch.origin` flows through unchanged (repo handles it).

- [ ] **Step 6: Verify compile + run movies tests**

Run: `cargo test -p oxibuilder-ext-movies`
Expected: compiles; existing tests pass (new column is nullable, no behavior change).

- [ ] **Step 7: Commit**

```bash
git add crates/oxibuilder-ext-movies
git commit -m "feat(movies): add origin (production country) field + TMDB parse"
```

---

### Task 2: Movie front-end `nations` stat dimension

**Files:**
- Create: `web/src/shared/nations.ts`
- Modify: `web/src/shared/api.ts`, `web/src/shared/stats/computeMovieStats.ts`, `web/src/extensions/movies/MoviesStatsPage.tsx`

**Interfaces:**
- Consumes: `MovieEntry.origin: string | null` (Task 1).
- Produces: `computeMovieStats` returns `nations: MovieCountRow[]` + `nationCount: number`; `MoviesStatsPage` renders a Nations section.

- [ ] **Step 1: Add `origin` to the MovieEntry type**

In `web/src/shared/api.ts` `MovieEntry` (282-320), add `origin: string | null;` (after `poster_path`).

- [ ] **Step 2: Create the shared nations module**

Create `web/src/shared/nations.ts` (ported from blog-test `movieStats.ts` `NATION_CODE_TO_KO`):
```ts
/** ISO-3166 alpha-2 → Korean display name. Ported from blog-test. */
export const NATION_CODE_TO_KO: Record<string, string> = {
  KR: "한국", US: "미국", JP: "일본", GB: "영국", FR: "프랑스", DE: "독일",
  CN: "중국", CA: "캐나다", AU: "호주", ES: "스페인", IT: "이탈리아",
  HK: "홍콩", TW: "대만", IN: "인도", RU: "러시아", SE: "스웨덴",
  DK: "덴마크", NL: "네덜란드", IE: "아일랜드", NZ: "뉴질랜드", MX: "멕시코",
  BR: "브라질", AR: "아르헨티나", BE: "벨기에", NO: "노르웨이", FI: "핀란드",
  PL: "폴란드", CH: "스위스", AT: "오스트리아", PT: "포르투갈", TR: "튀르키예",
  TH: "태국", ID: "인도네시아", PH: "필리핀", VN: "베트남", SG: "싱가포르",
  MY: "말레이시아", ZA: "남아프리카공화국", EG: "이집트", IL: "이스라엘",
  AE: "아랍에미리트", IR: "이란", IQ: "이라크", KZ: "카자흐스탄",
};

/** Split "KR,US" → ["KR","US"], trim, drop empties. */
export function parseNationCodes(origin: string | null | undefined): string[] {
  return (origin ?? "")
    .split(",")
    .map((c) => c.trim().toUpperCase())
    .filter((c) => c.length > 0);
}

/** Display name: known code → Korean, else the raw code. */
export function nationLabel(code: string): string {
  return NATION_CODE_TO_KO[code] ?? code;
}
```

- [ ] **Step 3: Write the failing stats test**

In `web/src/shared/stats/computeMovieStats.test.ts`, add:
```ts
it("buckets nations from comma-separated origin", () => {
  const stats = computeMovieStats([
    { origin: "KR,US" } as never,
    { origin: "KR" } as never,
    { origin: "us" } as never,
    { origin: null } as never,
  ]);
  expect(stats.nationCount).toBe(2); // KR + US
  const kr = stats.nations.find((n) => n.name === "한국");
  const us = stats.nations.find((n) => n.name === "미국");
  expect(kr?.count).toBe(2);
  expect(us?.count).toBe(2); // "US" + "us" normalized
});
```

- [ ] **Step 4: Run test to verify it fails**

Run: `bun test web/src/shared/stats/computeMovieStats.test.ts`
Expected: FAIL — `nations`/`nationCount` undefined.

- [ ] **Step 5: Implement the nations dimension**

In `computeMovieStats.ts`:
- Import `parseNationCodes`, `nationLabel` from `../nations`.
- Add `nationCounts: Map<string, number>` (key = ISO code).
- In the movie loop (~53-75), for each movie call `parseNationCodes(m.origin)`; for each code increment `nationCounts[code]`.
- Add to `MovieStats` interface: `nationCount: number;` and `nations: MovieCountRow[];`.
- After the loop, build nations rows: map entries → `{ name: nationLabel(code), count }`, sort by count desc then name, take top 15. Set `nationCount = nationCounts.size`.
- Return them in the stats object (~106-120).

- [ ] **Step 6: Run test to verify it passes**

Run: `bun test web/src/shared/stats/computeMovieStats.test.ts`
Expected: PASS.

- [ ] **Step 7: Render nations in the stats page**

In `MoviesStatsPage.tsx`:
- Add `nationCount` to the `SummaryBand` items (label ko "국가" / en "Countries").
- After the years `ColumnChart` section (~65-69), add a nations `Section`:
```tsx
{stats.nations.length > 0 && (
  <Section title={ko ? "국가" : "Countries"}>
    {stats.nations.map((n) => (
      <BarRow key={n.name} name={n.name} count={n.count} max={stats.nations[0].count} />
    ))}
  </Section>
)}
```

- [ ] **Step 8: Verify front-end builds**

Run: `cd web && bun run build` (or the repo's typecheck command)
Expected: typecheck passes, no errors.

- [ ] **Step 9: Commit**

```bash
git add web/src/shared/nations.ts web/src/shared/api.ts web/src/shared/stats/computeMovieStats.ts web/src/shared/stats/computeMovieStats.test.ts web/src/extensions/movies/MoviesStatsPage.tsx
git commit -m "feat(movies): restore nations stat dimension with localized labels"
```

---

### Task 3: Book `category`/`publisher`/`page_count` column + model + client + repo

**Files:**
- Create: `crates/oxibuilder-ext-books/migrations/0002_metadata.sql`
- Modify: `crates/oxibuilder-ext-books/src/model.rs`, `client.rs`, `repo.rs`

**Interfaces:**
- Produces: `Book.category: Option<String>`, `Book.publisher: Option<String>`, `Book.page_count: Option<i64>`; same on `BookInput`/`BookPatch`/`BookSearchResult`.

- [ ] **Step 1: Write the migration**

Create `crates/oxibuilder-ext-books/migrations/0002_metadata.sql`:
```sql
-- 0002: publisher metadata parity with blog-test book stats.
-- category/publisher/page_count come from Aladin/Google Books; NULL-safe,
-- backfilled per-book via `books refresh`.
ALTER TABLE book_entry ADD COLUMN category TEXT;
ALTER TABLE book_entry ADD COLUMN publisher TEXT;
ALTER TABLE book_entry ADD COLUMN page_count INTEGER;
```

- [ ] **Step 2: Add fields to the Rust model**

In `model.rs`:
- `Book` struct (5-23): add `pub category: Option<String>`, `pub publisher: Option<String>`, `pub page_count: Option<i64>` (after `cover_image_url`).
- `BookSearchResult` (26-34): add the same three fields.
- `BookInput` (36-53): add `pub category: Option<String>`, `pub publisher: Option<String>`, `pub page_count: Option<i64>` with `#[serde(default)]`.
- `BookPatch` (81-96): add the three as `Option<String>`/`Option<i64>` (already `Default`).

- [ ] **Step 3: Parse the new fields in the book client**

In `client.rs`:
- `AladinItem` (168-176): add `category_name: Option<String>` (serde rename `categoryName`), `publisher: Option<String>`, and `sub_info: Option<AladinSubInfo>` where:
  ```rust
  #[derive(Debug, Deserialize, Default)]
  struct AladinSubInfo { item_page: Option<i64> } // serde rename "itemPage"
  ```
  Use `#[serde(rename_all = "camelCase")]` on `AladinItem` and `AladinSubInfo`.
- `GoogleVolumeInfo` (190-198): add `categories: Option<Vec<String>>`, `publisher: Option<String>`, `page_count: Option<i64>` (serde rename `pageCount`).
- In `search_aladin` mapping (88-99): set `category: i.category_name`, `publisher: i.publisher`, `page_count: i.sub_info.and_then(|s| s.item_page)`.
- In `search_google` mapping (123-133): set `category: v.categories.and_then(|c| c.into_iter().next())`, `publisher: v.publisher`, `page_count: v.page_count`.

- [ ] **Step 4: Wire fields through repo**

In `repo.rs`:
- `COLUMNS` constant: add `category`, `publisher`, `page_count` (after `cover_image_url`).
- `create` (8-31): add the three columns to the INSERT list + placeholders `?13, ?14, ?15`, and `.bind(&input.category)`, `.bind(&input.publisher)`, `.bind(input.page_count)`.
- `update` (88-171): add three branches to the dynamic SET builder mirroring the existing field pattern — for each, when `patch.X` is `Some`, push `"X = ?"` and bind.

- [ ] **Step 5: Verify compile + run books tests**

Run: `cargo test -p oxibuilder-ext-books`
Expected: compiles; existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-ext-books
git commit -m "feat(books): add category/publisher/page_count fields + provider parsing"
```

---

### Task 4: Book front-end stats (categories/publishers/pages) + category filter

**Files:**
- Modify: `web/src/shared/api.ts`, `web/src/shared/stats/computeBookStats.ts`, `web/src/extensions/books/BooksStatsPage.tsx`, `web/src/extensions/books/BooksPage.tsx`

**Interfaces:**
- Consumes: `Book.category`/`publisher`/`page_count` (Task 3).

- [ ] **Step 1: Add fields to the Book type**

In `web/src/shared/api.ts` `Book` (324-341), add `category: string | null; publisher: string | null; page_count: number | null;`.

- [ ] **Step 2: Write the failing stats test**

In `web/src/shared/stats/computeBookStats.test.ts`, add tests asserting:
- `categories` rows from `Book.category` (non-null), sorted by count desc.
- `publishers` rows from `Book.publisher`.
- `pageBuckets` from `Book.page_count` bucketed by edges `[300, 500]` (`< 300`, `300–500`, `500+`), labeled.

- [ ] **Step 3: Run test to verify it fails**

Run: `bun test web/src/shared/stats/computeBookStats.test.ts`
Expected: FAIL.

- [ ] **Step 4: Implement the dimensions**

In `computeBookStats.ts` (port from blog-test `bookStats.ts`):
- Add `categories`, `publishers` via `topRows` over `Book.category`/`Book.publisher` strings.
- Add `pageBuckets` using the existing `bucketKey(value, edges)` helper (already in `computeMovieStats.ts` — extract to a shared spot or re-implement) with edges `[300, 500]`. Count only books with non-null `page_count`.
- Extend `BookStats` with `categories`, `publishers`, `pageBuckets: BookCountRow[]`.

- [ ] **Step 5: Run test to verify it passes**

Run: `bun test web/src/shared/stats/computeBookStats.test.ts`
Expected: PASS.

- [ ] **Step 6: Render in BooksStatsPage**

In `BooksStatsPage.tsx`, add three `Section` blocks (Categories, Publishers, Pages) using `BarRow`, gated on `stats.categories.length > 0` etc. For pages, label ko "페이지" / en "Pages".

- [ ] **Step 7: Add category chip filter to BooksPage**

In `BooksPage.tsx` (mirrors `MoviesPage` genre chips + blog-test category chips):
- Add `const [category, setCategory] = useState<string | null>(null);`.
- Compute `categoryCounts` from `books` (top 8 by count) in a `useMemo`.
- Add `category` to the `visible` filter (`books ?? []).filter((b) => !category || b.category === category)`.
- Render a chip row (reuse the genre-chip JSX pattern from `MoviesPage` ~257-279).
- Add category to `hasFilters` and `clearAll`.

- [ ] **Step 8: Verify front-end builds**

Run: `cd web && bun run build` (or typecheck)
Expected: passes.

- [ ] **Step 9: Commit**

```bash
git add web/src/shared/api.ts web/src/shared/stats/computeBookStats.ts web/src/shared/stats/computeBookStats.test.ts web/src/extensions/books
git commit -m "feat(books): restore category/publisher/pages stats + category filter"
```

---

### Task 5: `movies refresh` / `books refresh` (CLI + Console endpoint)

**Files:**
- Modify: `crates/oxibuilder-cli/src/commands/` (subcommand wiring), `crates/oxibuilder-ext-movies/src/routes.rs`, `crates/oxibuilder-ext-books/src/routes.rs`, `crates/oxibuilder-ext-{movies,books}/src/lib.rs` (route registration)

**Interfaces:**
- Produces: `POST /api/console/movies/:slug/refresh` and `POST /api/console/books/:id/refresh` returning the refreshed detail; CLI `oxibuilder movies refresh <slug|--all>` and `oxibuilder books refresh <id|--all>`.

- [ ] **Step 1: Add the movies refresh endpoint**

In `ext-movies/src/routes.rs`, add:
```rust
pub async fn refresh(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    let entry = repo::find_entry_by_slug(&pool.db, &slug).await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    let tmdb = TmdbClient::from_env();
    let tmdb_id = entry.tmdb_id;
    if !tmdb.enabled() || tmdb_id.is_none() {
        return Err(ApiError::validation("tmdb", "TMDB key or tmdb_id not configured"));
    }
    let meta = tmdb.fetch_movie_full(tmdb_id.unwrap()).await
        .map_err(|e| { tracing::warn!(error=?e, "refresh fetch failed"); ApiError::internal(e) })?;
    // PATCH only origin (and other TMDB-sourced fields the user hasn't overridden);
    // minimal safe version: update origin.
    repo::update_entry(&pool.db, &slug, &MovieEntryPatch {
        origin: meta.origin,
        ..Default::default()
    }).await.map_err(ApiError::internal)?;
    let detail = repo::find_entry_detail_by_slug(&pool.db, &slug).await
        .map_err(ApiError::internal)?.ok_or_else(|| not_found(&slug))?;
    Ok(Json(DataEnvelope { data: detail }))
}
```
Register the route in `ext-movies/src/lib.rs` (follow the existing `publish` route registration pattern).

- [ ] **Step 2: Add the books refresh endpoint**

In `ext-books/src/routes.rs`, add `refresh` that loads the book, and if `source` is `aladin`/`google_books` and a key/search is available, re-searches by `isbn13`/`title` and PATCHes `category`/`publisher`/`page_count`. Register in `ext-books/src/lib.rs`.

- [ ] **Step 3: Add CLI subcommands**

In `crates/oxibuilder-cli/src/commands/`, follow the existing `profile`/`site` subcommand pattern to add `movies refresh` and `books refresh` that hit the refresh endpoints (or call the repo+client directly when run against the local DB). `--all` iterates every entry.

- [ ] **Step 4: Verify compile + run integration tests**

Run: `cargo test -p oxibuilder-ext-movies -p oxibuilder-ext-books`
Expected: compiles; tests pass. (Network calls are guarded by env-key checks; without keys the endpoints return a validation error, not a panic.)

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-ext-movies crates/oxibuilder-ext-books crates/oxibuilder-cli
git commit -m "feat: add movies/books refresh for metadata backfill"
```

---

### Task 6: Integration build test + final verification

**Files:**
- Modify: `crates/oxibuilder-core/tests/ssg_build.rs` (or a new test) — assert a DB without the new columns still builds (migration applies cleanly).

- [ ] **Step 1: Add a migration-compat build test**

Following the existing `ssg_build.rs` pattern, add a test that seeds a DB in the pre-migration shape (no `origin`/`category`/`publisher`/`page_count` columns), runs `run_image_pre_pass` + `build_site`, and asserts it succeeds (migrations apply, nullable columns don't break the build).

- [ ] **Step 2: Run the full test suite**

Run: `cargo test` and `cd web && bun test`
Expected: all pass.

- [ ] **Step 3: Smoke-build a site**

Run: `cargo build --release && ./target/release/oxibuilder build` against a fixture site with ≥1 movie + ≥1 book (or the dev DB).
Expected: build completes; `out/data/movies.json` / `books.json` carry the new fields.

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-core/tests/ssg_build.rs
git commit -m "test: migration compat for origin/book metadata columns"
```

# Movies Extension Upgrade — Design

Date: 2026-08-06
Status: Approved for autonomous implementation (user away)

## Context assessment (current state)

The movies extension already runs on **TMDB** (not KMDB as the user believed). TMDB
natively serves both `language=ko-KR` and `language=en-US`, which is exactly the
localization substrate the user wants. The gaps:

1. **Titles are monolingual.** `movie_entry.title` is a single `TEXT NOT NULL`.
   TMDB search uses `language=ko-KR`, so stored titles are Korean. An English
   visitor sees the same Korean title — the "mixing" the user dislikes. (Reviews
   and `series_group` titles are already bilingual `*_ko`/`*_en`.)
2. **No cast / genre data at all.** TMDB provides credits + genres; nothing is stored.
3. **No filters.** `ListQuery` only has `series_group` / `limit` / `draft`. The
   public `MoviesPage` is a flat grid — no year / genre / actor / type facets, no sort.
4. **No per-movie public detail route** (pre-existing gap, out of scope).

`sqlx::raw_sql(...)` runs the whole migration file (multi-statement) inside one
transaction, tracked by `schema_migrations`. Public SPA data resolves from
`build_data` output → `data/movies.json` (static) or `/api/console/movies` (live).

## Goals

- **Bilingual titles** (Korean default, English fallback) — no language mixing.
- **Cast + genre + year metadata**, auto-populated from TMDB, editable manually.
- **Public page filters**: text search, media type, genre, year, **lead actor**,
  plus sort. Rich cards showing localized title, genres, lead cast.
- Korean is the site default; both languages rendered via the existing
  `pick(ko, en)` mechanism.

## Non-goals

- Adding KMDB (redundant — TMDB covers Korean well; user's premise was mistaken).
- Per-movie public detail page (separate work).
- Server-side pagination (1-person site, ≤200 titles, client-side filtering).

## Design

### 1. Schema — migration 0002 (`movies`)

```sql
ALTER TABLE movie_entry ADD COLUMN title_ko TEXT;
ALTER TABLE movie_entry ADD COLUMN title_en TEXT;
ALTER TABLE movie_entry ADD COLUMN runtime_min INTEGER;

CREATE TABLE movie_genre (
    movie_entry_id INTEGER NOT NULL,
    name_en TEXT NOT NULL,        -- canonical key (TMDB always has en)
    name_ko TEXT,
    PRIMARY KEY (movie_entry_id, name_en),
    FOREIGN KEY (movie_entry_id) REFERENCES movie_entry(id) ON DELETE CASCADE
);

CREATE TABLE movie_person (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_person_id INTEGER UNIQUE,
    slug TEXT NOT NULL UNIQUE,
    name_en TEXT NOT NULL,
    name_ko TEXT,
    profile_path TEXT,
    role TEXT NOT NULL DEFAULT 'actor',   -- 'actor' | 'director'
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE movie_entry_person (
    movie_entry_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL,
    character_name TEXT,
    billing INTEGER,              -- cast order, lower = lead
    PRIMARY KEY (movie_entry_id, person_id),
    FOREIGN KEY (movie_entry_id) REFERENCES movie_entry(id) ON DELETE CASCADE,
    FOREIGN KEY (person_id) REFERENCES movie_person(id) ON DELETE CASCADE
);
```

`title` stays `NOT NULL` (canonical / slug source / FTS). Display layer picks
`title_ko` / `title_en`, falling back to `title`. Existing rows get
`title_ko := title` backfilled (data was fetched ko-KR).

### 2. TMDB enrichment (`integration.rs`)

`fetch_movie_full(tmdb_id) -> MovieMeta`:
- `/movie/{id}?language=ko-KR` → `title_ko`, `poster_path`, `release_date`,
  `runtime`, genres (ko names + ids).
- `/movie/{id}?language=en-US&append_to_response=credits` → `title_en`,
  genres (en names + ids), top-10 cast (person id, name, character, order),
  directors (crew where `job == "Director"`).

Genre pairs joined by TMDB genre `id` → `(name_en, name_ko)`.
`search()` unchanged (picker shows ko title); full enrichment happens at create.

### 3. Repo (`repo.rs`)

- `ENTRY_COLUMNS` += `title_ko, title_en, runtime_min`.
- `create_entry` takes new fields; then `replace_genres(entry_id, &[(en,ko)])`,
  `replace_people(entry_id, &people)` upsert genres + people + join rows.
- `update_entry` handles `title_ko/title_en/runtime_min/genres/cast` patches.
- `list_entries_detail(pool, draft) -> Vec<MovieEntryDetail>`: fetch entries,
  then all genres + all people in two batched queries, group in a `HashMap`.
- `find_entry_detail_by_slug` for show.
- `list_genre_facets()` / `list_person_facets(role)` for future server-side use.

`MovieEntryDetail { #[serde(flatten)] entry, genres: Vec<GenreName>, cast: Vec<PersonSummary>, directors: Vec<PersonSummary> }`.

### 4. Routes (`routes.rs`)

- `list` → `DataEnvelope<Vec<MovieEntryDetail>>`.
- `create`: on `tmdb_id` + key, call `fetch_movie_full`; merge client-explicit >
  TMDB > None for every field (titles, genres, cast, runtime, poster, year).
- `update`: accept new patch fields; re-replace genres/cast when provided.
- `validate_input`: title still required (or tmdb_id).

### 5. Build (`lib.rs`)

- `build_data` → `Vec<MovieEntryDetail>` (static site gets localized titles +
  genres + cast).
- `build_pages` / `build_search_docs` use localized title for SEO.

### 6. Frontend — types

- `shared/api.ts`: `MovieEntry` gains `title_ko`, `title_en`, `runtime_min`,
  `genres: {name_en, name_ko}[]`, `cast: PersonSummary[]`, `directors`.
  `fetchMovies` returns enriched array.

### 7. Frontend — public `MoviesPage` redesign

- **Filter bar**: text search; media type toggle (All/Movie/TV); genre chips
  (localized); year dropdown; **lead-actor** dropdown (people with ≥1 credit,
  localized). Active-facet chips with clear.
- **Sort**: Recently watched / Rating / Year / Title.
- **Card**: poster, localized title, year·runtime, rating stars, genre chips,
  top-2 lead cast (clickable → filters by that person), rewatch badge.
- Client-side filter/sort over the fetched collection; memoized.
- Empty states: no movies / no matches-for-filters.

### 8. Frontend — admin `MoviesTab`

- Form: `Title (Korean)` + `Title (English)` (replaces single Title; canonical
  `title` derived as `title_ko ?? title_en`). Runtime field.
- Genre editor (chips, add/remove, localized name).
- Cast editor (list of name/character, add/remove). Auto-populated on TMDB pick.
- TMDB pick pre-fills title_ko (search is ko-KR); the full bilingual + cast +
  genre data is fetched server-side at save (authoritative).
- Table row shows localized title.

## Migration safety

Single migration, transactional, idempotent via `schema_migrations` versioning.
Backfill `title_ko` from `title` for existing rows.

## Verification

- `cargo build -p oxibuilder-ext-movies` + workspace build.
- `bun run build:static` (tsc + vite) — catches type regressions.
- Run the migration against the dev DB via `oxibuilder console` startup; query
  the new tables to confirm structure.

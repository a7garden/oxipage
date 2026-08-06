# Editorial Layout Variant + Feature Parity — Design

Date: 2026-08-06
Status: Approved (design confirmed with user)

## Context assessment (current state)

The user runs `../blog-test` (an Astro 7 + Tailwind v4 static site) as their actual
personal site and loves its aesthetic. They want oxibuilder's public site to reach the
same visual and functional level — "ideally identical." Investigation compared both:

**blog-test's identity is structural, not chromatic.** What makes it look good is:

1. **Layout** — centered identity lobby + per-page headers (no global nav chrome).
2. **Minimal chrome** — whitespace-driven, no decorative animation/gradient/blur,
   typography-centric.
3. **Feature completeness** — books/movies both have chip-filter + search + responsive
   grid **and stats pages** (JS-free CSS bar/column charts).
4. **Typography** — SUIT (body) + SUITE (display).

**oxibuilder today** has a 6-color-theme axis (`[data-public-theme]`: paper/midnight/
sepia/forest/neon/canvas) that controls only accent hue + surface tone. It cannot encode
blog-test's structural differences. Gaps vs blog-test:

| Area | blog-test | oxibuilder |
|---|---|---|
| Lobby | centered hub, 3 cards | canvas/grid/list modes (canvas = floating anim) |
| Global chrome | none (per-page headers) | sticky header + nav + footer shell |
| Books | chips + search + sort + **stats** | no filters, basic grid |
| Movies | chips + search + **stats** | facets best-in-class ✓, but **no detail route** (SSG shell → SPA 404) |
| Stats pages | books + movies CSS charts | **none** |
| Fonts | SUIT/SUITE | Fraunces serif (UNIFIED-DESIGN.md already names SUITE as migration target) |

**Key insight:** mapping blog-test onto the color-theme axis is a category error —
blog-test's impression comes from layout/typography/chrome structure, not color. The fix
is a **second, orthogonal axis**: a *layout variant*.

## Goals

- **New "editorial" layout variant** — a faithful blog-test reproduction (hub lobby,
  no global chrome, per-page headers, SUIT/SUITE typography), orthogonal to the 6 color
  themes (any theme × either layout).
- **Preserve "shell"** (current look) as the default; editorial is opt-in.
- **Feature parity** (all layouts): stats pages (books/movies), books filter parity,
  fix the movie detail-route 404.
- **Reuse** the existing per-site DB theme singleton pipeline for the layout axis — no
  new storage/transport plumbing.
- **Fix the FOUC bug** found during investigation: `index.html` hardcodes
  `<meta name="oxibuilder-theme" content="paper">` but the active theme is midnight →
  initial paint is always paper, repainted by JS after.

## Non-goals

- Book detail page (blog-test has none; oxi data shape doesn't need it yet).
- Novel/links detail pages.
- Dynamic extension registry / runtime plugin loading (stays compile-time registration).
- Stats pages for extensions beyond books/movies this cycle.
- Replacing blog-test's data scripts — oxi extensions already integrate Aladin/TMDB.

## Design

### 1. Orthogonal axis: layout variant

Two layout variants, modeled as a catalog mirroring `theme.rs`:

- `shell` (default) — current look: sticky header + nav + footer, grid/canvas/list lobby.
- `editorial` (new) — blog-test faithful: no global chrome, centered hub lobby,
  per-page headers.

Color themes (6) apply independently on top (e.g. editorial × midnight).

**Implementation: hybrid** — `[data-layout]` CSS attribute for typography/spacing
micro-tuning **plus** React conditional rendering for structural differences (shell
present/absent, hub vs grid lobby). Pure-CSS cannot express chrome presence; pure-React
scatters typographic tweaks.

### 2. Data model & storage

Mirror the theme singleton exactly:

```sql
-- core migration 0008 (0007 deploy_log is applied ad-hoc; 0008 avoids filename collision)
ALTER TABLE theme_config ADD COLUMN layout TEXT NOT NULL DEFAULT 'shell';
```

`theme_config` is the id=1 singleton already used for `theme_id`.

**Rust (`theme.rs`):**

```rust
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LayoutDefinition {
    pub id: &'static str,            // "shell" | "editorial"
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub description_ko: &'static str,
    pub description_en: &'static str,
}

pub const ALL_LAYOUTS: &[LayoutDefinition] = &[
    LayoutDefinition { id: "shell", name_ko: "셸", name_en: "Shell",
        description_ko: "스티키 헤더·네비·푸터, 그리드/캔버스 로비",
        description_en: "Sticky header/nav/footer, grid/canvas lobby" },
    LayoutDefinition { id: "editorial", name_ko: "에디토리얼", name_en: "Editorial",
        description_ko: "크롬 없음, 중앙 허브 로비, 페이지별 헤더",
        description_en: "No chrome, centered hub lobby, per-page headers" },
];

pub fn find_layout(id: &str) -> Option<&'static LayoutDefinition> { ... }
pub fn is_known_layout(id: &str) -> bool { ... }
pub async fn active_layout_id(db: &sqlx::SqlitePool) -> String { ... } // mirrors active_theme_id
```

**`config.rs`:** `[lobby]` gains `layout = "shell"` (default) — first-seed value; DB wins
when present (same as theme).

```toml
[lobby]
default_mode = "grid"
layout = "editorial"      # "shell" (default) | "editorial"
```

### 3. API & transport

Reuse the theme endpoint; extend its envelope rather than adding a route:

- `ThemeResponse` gains `layout: String`.
- `GET /api/console/theme` and `/s/{slug}/theme` return `layout`.
- `PUT` validates `layout` via `is_known_layout` before UPDATE (mirrors theme validation).
- `Manifest.site` gains `layout` → flows into `data/lobby.json` (static) and the live
  lobby manifest (same contract, same `assemble()`).

### 4. Build / SSG + FOUC fix

`build_writer.rs`:

- `data/theme.json` snapshot includes `layout`.
- **Fix:** stop hardcoding the `<meta name="oxibuilder-theme" content="paper">` constant
  in `web/index.html`. Inject the *actual* active `theme_id` and `layout` at build time
  into the per-build `index.html` (and the embedded SPA template) so initial paint is
  correct. `<meta name="oxibuilder-layout" content="editorial">` is added alongside.

`web/public/theme-boot.js`: read both metas pre-paint and set `html[data-layout]` +
`html[data-public-theme]` + `--accent-hue`. No layout-switch flicker.

### 5. Component architecture (editorial look)

`App.tsx` branches on layout. The layout must be known at first paint to avoid a
shell/editorial flash, so the **initial** value is read synchronously from
`document.documentElement.dataset.layout` (set pre-paint by `theme-boot.js` from the
`<meta name="oxibuilder-layout">`), then confirmed once the manifest resolves:

```tsx
const layout = document.documentElement.dataset.layout ?? "shell";
return layout === "editorial" ? <EditorialShell/> : <Shell/>;
```

Both share `QueryClientProvider`, `AssetResolverProvider`, `BrowserRouter`, routes table.

- **`EditorialShell`** (new, `web/src/shell/EditorialShell.tsx`): no header/nav/footer.
  `LanguageProvider` + `<Routes>`. Each page renders its own header.
- **`HubLobby`** (new, `web/src/lobby/HubLobby.tsx`): blog-test lobby — centered site
  identity (name + short tagline) + section-card grid (one card per active
  extension/mount). Border cards, hover `border-strong + bg-surface-sunken`. Because there
  is no chrome, **theme + language toggles live as a small control row at the lobby
  bottom** (blog-test pattern), reusing existing `ThemeToggle`/`LangToggle`.
- **`EditorialPageHeader`** (new shared, `web/src/shell/EditorialPageHeader.tsx`):
  `← back` (to `/`) + `<h1>` + count + optional stats link. Each extension list page
  renders it at top in editorial mode; `shell` mode keeps `PageTitle`.
- **Card tone:** `[data-layout="editorial"]` scope in `tokens.css` adjusts whitespace,
  border emphasis, typography. movie/book cards reuse structure, editorial tone overrides.

### 6. Feature parity (all layouts)

#### 6a. Stats pages (new)

- **`web/src/shared/stats/StatsKit.tsx`**: `BarRow` (label + count + `bg-interactive`
  progress bar by %), `SummaryBand` (numeric summary row), `ColumnChart` (year columns).
  All **JS-free pure CSS charts** (ported from blog-test's `BarRow.astro`).
- **`web/src/shared/stats/computeBookStats.ts`** and **`computeMovieStats.ts`**: pure
  functions ported from blog-test's `bookStats.ts`/`movieStats.ts` — year-span continuity,
  top category/author/publisher, runtime/rating buckets, averages.
- **Routes** `/books/stats`, `/movies/stats` → `BooksStatsPage`, `MoviesStatsPage`. Data
  computed client-side from existing `data/books.json`/`movies.json` collections (no
  build-time stats JSON needed).
- `EditorialPageHeader` shows the stats link; in `shell` mode the list page surfaces it.

#### 6b. Books filter parity

Bring `BooksPage` to `MoviesPage` level: text search (title/author), category chips
(with counts), status filter (wishlist/reading/completed/dropped), sort. Extract
`MoviesPage`'s facet logic into a shared **`useFacets(collection, facetDefs)`** hook so
both books and movies reuse it (also de-dupes movies' inline facet code).

#### 6c. Movie detail route (bug fix)

- Add `/movies/:slug` route + **`MovieDetailPage`** (new). SSG already builds
  `movies/{slug}/index.html` SEO shells but the SPA lacked the route → 404. Detail =
  large poster + meta (year/genre/runtime/rating) + synopsis + cast/directors.
- Book detail stays out of scope.

### 7. Font migration (UNIFIED-DESIGN.md target)

`web/index.html` Fraunces/Noto Serif KR → **SUITE (display) + SUIT (body)** via jsDelivr.
Already documented as Step 2 of the v1→unified migration. Include line-height re-tuning
(body 1.55→1.50, small 1.5→1.45) per the migration note. Applies to both layouts; it is
the editorial look's typographic identity.

## Migration safety

- Single core migration adds a `NOT NULL DEFAULT 'shell'` column to the existing
  singleton table — no data loss, every existing site stays `shell`.
- Validated at PUT boundary via `is_known_layout`.
- Schema migration tracked by `schema_migrations` versioning (same as existing).

## Verification

- `cargo build` workspace (theme.rs/config.rs/build_writer.rs/console router changes).
- `bun run build:static` (tsc + vite) catches type regressions; `bun run build` (admin).
- Core migration applied on dev-DB startup; `SELECT layout FROM theme_config` confirms.
- **Smoke:** `oxibuilder build` → inspect `out/index.html` meta = actual theme + layout
  (not hardcoded paper). `out/data/theme.json` carries `layout`.
- **Visual:** set layout=`editorial` → `oxibuilder console` preview → confirm chromeless
  hub lobby, per-page headers, stats pages render; switch to `shell` → current look intact.
- **Bug fix:** confirm initial paint meta no longer always paper.
- Stats computations: port blog-test's `movieStats.test.ts` assertions (e.g. 미국/한국/일본
  counts, top actors) as a guard.

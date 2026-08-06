# Editorial Layout Variant + Feature Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in "editorial" layout variant (a faithful reproduction of the `../blog-test` personal-site aesthetic — hub lobby, no global chrome, per-page headers) orthogonal to the existing 6 color themes, plus close the feature-parity gaps (stats pages, books filters, movie detail route, font migration).

**Architecture:** A second, orthogonal layout axis mirrors the existing per-site DB theme singleton pipeline. The layout id is stored in `theme_config.layout`, transported over the same theme API + manifest contract, snapshotted into `data/theme.json` at build time, and applied pre-paint via `theme-boot.js` (`html[data-layout]`). React branches `App.tsx` between `Shell` (existing) and `EditorialShell` (new). Feature parity (stats, filters, detail route) is layout-agnostic and shared by both.

**Tech Stack:** Rust (axum, sqlx, serde), React 19 + Vite 7 + Tailwind v4, TypeScript, SQLite. Fonts via jsDelivr (SUIT/SUITE).

**Design spec:** `docs/superpowers/specs/2026-08-06-editorial-layout-variant-design.md`

## Global Constraints

- Migration is single-file, transactional, tracked by `schema_migrations`; the new column is `NOT NULL DEFAULT 'shell'` so every existing site stays on the current look — no data loss, no behavior change for sites that don't opt in.
- Layout validation uses `is_known_layout()` at the PUT boundary (mirrors theme validation).
- The layout must be known at first paint — read `document.documentElement.dataset.layout` (set by `theme-boot.js` from `<meta name="oxibuilder-layout">`) synchronously in `App.tsx`; confirm from the manifest once it resolves.
- Color themes (6) apply independently on top of either layout (e.g. editorial × midnight).
- **Data-shape realism:** oxi's `Book` has fields `{title, author, isbn13, cover_image_url, rating, status, review_ko/en, started_at, finished_at, published_at, created_at}` — NO category/publisher/pages. oxi's `MovieEntry` has NO `nation`. Stats ports must compute from oxi's actual shapes (not blog-test's `RawBook`/`RawMovie`), dropping nation/category/publisher/pages-only slices.
- Web type-check gate is `bun run build` (runs `tsc --noEmit`); `bun run build:static` does NOT run tsc. Always run `bun run build` to catch type regressions.
- Rust gate: `cargo build` (whole workspace). Commit messages in English, conventional (`feat:`/`fix:`/`refactor:`/`test:`/`docs:`/`chore:`).
- Do NOT touch the admin console shell's light/dark/system behavior — only the public site layout axis is new.

---

## File Structure

### Rust core (`crates/oxibuilder-core/src/`)
- `theme.rs` — add `LayoutDefinition`, `ALL_LAYOUTS`, `find_layout`, `is_known_layout`, `active_layout_id(db, default)`.
- `config.rs` — `[lobby].layout` field on `LobbySection` (default `"shell"`).
- `builder.rs` — add `layout_id: String` to `BuildInputs`; populate from `active_layout_id`.
- `build_writer.rs` — include `layout` in `data/theme.json`; inject real theme+layout into the served `index.html` `<meta>` (FOUC fix); add a `layout_id` field to `BuildManifest`.
- `manifest.rs` — add `layout` to the assembled `Manifest.site`.
- `migrations/core/0006_layout.sql` — `ALTER TABLE theme_config ADD COLUMN layout TEXT NOT NULL DEFAULT 'shell'`.

### Console (`crates/oxibuilder-console/src/`)
- `per_site.rs` — `theme_get`/`theme_put` + default-theme handler read/write `layout`; `ThemePutInput.layout: Option<String>`.
- `router.rs` — `get_default_theme` returns `layout`.

### Frontend (`web/src/`)
- `shared/api.ts` — `ManifestSite.layout`, `fetchSiteTheme` returns layout.
- `shared/theme.ts` — `applyServerTheme` publishes `html[data-layout]`.
- `public/theme-boot.js` — read layout meta, set `html[data-layout]`.
- `index.html` — add `<meta name="oxibuilder-layout" content="shell">`; swap fonts to SUIT/SUITE.
- `App.tsx` — branch `Shell` vs `EditorialShell`; add `/movies/:slug`, `/books/stats`, `/movies/stats` routes.
- `shell/EditorialShell.tsx` (new), `shell/EditorialPageHeader.tsx` (new) — chromeless shell + per-page header.
- `lobby/HubLobby.tsx` (new) — centered hub lobby.
- `shared/tokens.css` — `[data-layout="editorial"]` scope.
- `shared/stats/StatsKit.tsx` (new) — `BarRow`, `SummaryBand`, `ColumnChart`.
- `shared/stats/computeBookStats.ts`, `computeMovieStats.ts` (new) — adapted to oxi shapes.
- `shared/useCollectionFilter.ts` (new) — generic filter/sort/active-chip hook.
- `extensions/books/BooksPage.tsx` — filter parity (uses `useCollectionFilter`).
- `extensions/books/BooksStatsPage.tsx` (new), `extensions/movies/MoviesStatsPage.tsx` (new).
- `extensions/movies/MovieDetailPage.tsx` (new) — detail route.

### Config / docs
- `oxibuilder.toml.example` — document `[lobby].layout`.

---

## Task 1: Layout catalog in `theme.rs`

**Files:**
- Modify: `crates/oxibuilder-core/src/theme.rs`

**Interfaces:**
- Produces: `LayoutDefinition`, `ALL_LAYOUTS: &[LayoutDefinition]`, `find_layout(id)->Option`, `is_known_layout(id)->bool`, `active_layout_id(db, default)->String`.

- [ ] **Step 1: Add the layout catalog + helpers after `active_theme_id`**

Append after the `active_theme_id` function (around line 120):

```rust
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LayoutDefinition {
    pub id: &'static str,
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub description_ko: &'static str,
    pub description_en: &'static str,
}

pub const ALL_LAYOUTS: &[LayoutDefinition] = &[
    LayoutDefinition {
        id: "shell",
        name_ko: "셸",
        name_en: "Shell",
        description_ko: "스티키 헤더·네비·푸터, 그리드/캔버스 로비",
        description_en: "Sticky header/nav/footer, grid/canvas lobby",
    },
    LayoutDefinition {
        id: "editorial",
        name_ko: "에디토리얼",
        name_en: "Editorial",
        description_ko: "크롬 없음, 중앙 허브 로비, 페이지별 헤더",
        description_en: "No chrome, centered hub lobby, per-page headers",
    },
];

pub fn find_layout(id: &str) -> Option<&'static LayoutDefinition> {
    ALL_LAYOUTS.iter().find(|l| l.id == id)
}

pub fn is_known_layout(id: &str) -> bool {
    find_layout(id).is_some()
}

/// Read the active layout for a site. Falls back to `default` (from config)
/// when the table/row/column is absent — never blocks a build.
pub async fn active_layout_id(db: &sqlx::SqlitePool, default: &str) -> String {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT layout FROM theme_config WHERE id = 1")
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
    match row {
        Some((l,)) if is_known_layout(&l) => l,
        _ => default.to_string(),
    }
}
```

- [ ] **Step 2: Add a unit test**

```rust
#[test]
fn layout_catalog_is_complete() {
    assert_eq!(ALL_LAYOUTS.len(), 2);
    assert!(is_known_layout("shell"));
    assert!(is_known_layout("editorial"));
    assert!(!is_known_layout("bogus"));
    assert_eq!(find_layout("editorial").unwrap().id, "editorial");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p oxibuilder-core`
Expected: PASS (the new column does not exist yet; `active_layout_id` is only called after migration 0008 lands in Task 2 — do not wire callers yet).

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-core/src/theme.rs
git commit -m "feat(core): add layout variant catalog (shell/editorial)"
```

---

## Task 2: Migration `0008_layout.sql`

**Files:**
- Create: `crates/oxibuilder-core/migrations/core/0008_layout.sql`
- Modify: `crates/oxibuilder-core/src/migrate.rs` (`CORE_MIGRATIONS` array) — add version 8.

> Version note: `CORE_MIGRATIONS` registers versions 1,2,4,5,6. Version 7 (`0007_deploy_log.sql`) exists on disk but is applied ad-hoc via inline `CREATE TABLE IF NOT EXISTS` at deploy time (`deploy_run.rs`), NOT through this array. Use version 8 + filename `0008_layout.sql` to avoid filename collision with the orphaned 0007 file. Non-contiguous versions are already established (the array skips 3).

**Interfaces:**
- Produces: `theme_config.layout TEXT NOT NULL DEFAULT 'shell'` column.

- [ ] **Step 1: Write the migration**

```sql
-- Layout variant axis (docs/superpowers/specs/2026-08-06-editorial-layout-variant-design.md §2).
-- Orthogonal to theme_id. Defaults to 'shell' so existing sites keep their look.
ALTER TABLE theme_config ADD COLUMN layout TEXT NOT NULL DEFAULT 'shell';
```

- [ ] **Step 2: Register version 8**

In `CORE_MIGRATIONS` (`crates/oxibuilder-core/src/migrate.rs`), append after the version-6 `setup_state` entry:

```rust
    Migration {
        version: 8,
        name: "layout",
        sql: include_str!("../migrations/core/0008_layout.sql"),
    },
```

(Same `include_str!` + `raw_sql` pattern as version 5. Do NOT add an entry for the orphaned 0007 deploy_log.)

- [ ] **Step 3: Verify the migration applies**

Run: `cargo build -p oxibuilder-core && cargo test -p oxibuilder-core migrations 2>/dev/null; cargo test -p oxibuilder-core layout_catalog`
Expected: catalog test PASS. Then start the console against the dev DB once and `sqlite3 data/oxibuilder.db "SELECT layout FROM theme_config"` returns `shell`.

- [ ] **Step 4: Commit**

```bash
git add crates/oxibuilder-core/migrations/core/0008_layout.sql crates/oxibuilder-core/src/migrate.rs
git commit -m "feat(core): migration 0008 adds theme_config.layout"
```

---

## Task 3: `[lobby].layout` config field

**Files:**
- Modify: `crates/oxibuilder-core/src/config.rs` (`LobbySection`, lines ~142-154)

**Interfaces:**
- Produces: `LobbySection { default_mode: String, layout: String }`.

- [ ] **Step 1: Add the field + default**

In `LobbySection`:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LobbySection {
    pub default_mode: String,
    pub layout: String,
}

impl Default for LobbySection {
    fn default() -> Self {
        Self {
            default_mode: "grid".to_string(),
            layout: "shell".to_string(),
        }
    }
}
```

- [ ] **Step 2: Add `validate_layout()` to `impl Config`**

This repo validates config via focused `Result<(), String>` methods (see `validate_mounts`), not a generic `validate()`. Follow that pattern:

```rust
/// Validate [lobby].layout against the known layout catalog.
pub fn validate_layout(&self) -> Result<(), String> {
    if !crate::theme::is_known_layout(&self.lobby.layout) {
        return Err(format!(
            "'{}' is not a valid [lobby].layout (expected 'shell' or 'editorial')",
            self.lobby.layout
        ));
    }
    Ok(())
}
```

- [ ] **Step 3: Add config tests**

```rust
#[test]
fn lobby_layout_defaults_to_shell() {
    let cfg = Config::default();
    assert_eq!(cfg.lobby.layout, "shell");
}

#[test]
fn lobby_layout_rejects_unknown() {
    let toml = "[lobby]\nlayout = \"bogus\"\n";
    let cfg = toml::from_str::<Config>(toml).expect("parses");
    assert!(cfg.validate_layout().is_err());
}
```

- [ ] **Step 4: Verify**

Run: `cargo test -p oxibuilder-core lobby_layout`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/config.rs
git commit -m "feat(core): add [lobby].layout config field"
```

---

## Task 4: Console theme API carries `layout`

**Files:**
- Modify: `crates/oxibuilder-console/src/per_site.rs` (`theme_get`, `theme_put`, `ThemePutInput` — lines ~595-657)
- Modify: `crates/oxibuilder-console/src/router.rs` (`get_default_theme` — lines ~226-260)

**Interfaces:**
- Produces: `GET /api/console/s/{slug}/theme` and `GET /api/console/theme` return `{theme_id, definition, layout}`; `PUT` accepts optional `layout` (validated by `is_known_layout`).

- [ ] **Step 1: Extend `theme_get` to SELECT + return layout**

Change the query to `SELECT theme_id, layout FROM theme_config WHERE id = 1`, bind to `(String, String)`, and include `"layout": layout` in the JSON `data`. Fall back to `"shell"` when absent.

- [ ] **Step 2: Extend `theme_put`**

```rust
#[derive(Deserialize)]
pub struct ThemePutInput {
    pub theme_id: String,
    #[serde(default)]
    pub layout: Option<String>,
}
```

In the handler: if `input.layout` is `Some(l)`, validate `is_known_layout(&l)` (else 400), and write BOTH columns:

```rust
sqlx::query(
    "INSERT INTO theme_config (id, theme_id, layout, updated_at) VALUES (1, ?1, ?2, datetime('now'))
     ON CONFLICT(id) DO UPDATE SET theme_id = ?1, layout = ?2, updated_at = datetime('now')",
)
.bind(&input.theme_id)
.bind(input.layout.as_deref().unwrap_or("shell"))
```

Return the same `{theme_id, definition, layout}` envelope (read the stored layout back or use the input/fallback).

- [ ] **Step 3: Extend `get_default_theme` (router.rs)** to SELECT + return `layout` (mirror Step 1).

- [ ] **Step 4: Verify**

Run: `cargo build -p oxibuilder-console`
Expected: PASS. (End-to-end verified in Task 19 smoke after frontend lands.)

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-console/src/per_site.rs crates/oxibuilder-console/src/router.rs
git commit -m "feat(console): theme API carries layout variant"
```

---

## Task 5: Manifest + BuildInputs carry `layout`

**Files:**
- Modify: `crates/oxibuilder-core/src/manifest.rs` (`assemble`)
- Modify: `crates/oxibuilder-core/src/builder.rs` (`BuildInputs`)

**Interfaces:**
- Produces: `Manifest.site.layout: String` in both the live `/lobby/manifest` and `data/lobby.json`; `BuildInputs.layout_id: String`.

- [ ] **Step 1: Add layout to `BuildInputs`**

```rust
pub struct BuildInputs {
    // …existing fields…
    pub layout_id: String,
}
```

- [ ] **Step 2: Populate it at every `BuildInputs` construction site**

Grep for `BuildInputs {` — every constructor must set `layout_id`. The build command (in `oxibuilder-cli` or core build entry) computes it via `active_layout_id(&db, &config.lobby.layout)`.

- [ ] **Step 3: Add `layout` to the assembled manifest**

In `assemble(...)`, read the active layout (same `active_layout_id`) and include `layout` in the `site` object serialized to both the API response and `data/lobby.json`.

- [ ] **Step 4: Verify**

Run: `cargo build`
Expected: PASS (all `BuildInputs` sites updated).

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/manifest.rs crates/oxibuilder-core/src/builder.rs <build entry>
git commit -m "feat(core): carry layout through manifest + BuildInputs"
```

---

## Task 6: Build `data/theme.json` layout + FOUC meta fix

**Files:**
- Modify: `crates/oxibuilder-core/src/build_writer.rs` (theme.json snapshot ~117-124; `transform_spa_index` ~387; `BuildManifest`)

**Interfaces:**
- Produces: `data/theme.json` includes `layout`; served `index.html` `<meta name="oxibuilder-theme">` + `<meta name="oxibuilder-layout">` carry the ACTUAL active values (not the hardcoded `paper`/`shell`).

- [ ] **Step 1: Include layout in the theme.json snapshot**

At lines ~117-124, change the snapshot to also emit layout (from `inputs.layout_id`):

```rust
let theme_def = crate::theme::find_theme(&inputs.theme_id);
let layout_def = crate::theme::find_layout(&inputs.layout_id);
if let Some(def) = theme_def {
    let theme_json = serde_json::to_string_pretty(&serde_json::json!({
        "theme_id": def.id,
        "definition": def,
        "layout": layout_def.map(|l| l.id).unwrap_or("shell"),
    }))?;
    fs::write(data_dir.join("theme.json"), &theme_json)?;
}
```

- [ ] **Step 2: Fix the hardcoded meta in the served index.html**

The embedded `web/index.html` carries placeholder metas (`content="paper"`, and a new `content="shell"` from Task 9). In `transform_spa_index`, replace them with the real build-time values BEFORE writing. Pass `theme_id` + `layout_id` into the transform:

```rust
fn transform_spa_index(html: &str, deployment_base: &str, theme_id: &str, layout_id: &str) -> String {
    let with_base = ensure_base_href(html, deployment_base);
    // Replace the placeholder meta contents injected by index.html so first paint
    // matches the active theme/layout (the embedded template ships neutral defaults).
    let with_theme = replace_meta(with_base.as_str(), "oxibuilder-theme", theme_id);
    replace_meta(&with_theme, "oxibuilder-layout", layout_id)
}

fn replace_meta(html: &str, name: &str, value: &str) -> String {
    let needle_start = format!(r#"name="{name}" content="#");
    html.split(&needle_start).enumerate().fold(
        String::new(),
        |mut acc, (i, part)| {
            if i == 0 { acc.push_str(part); return acc; }
            // part begins right after `content="`; replace up to the closing quote.
            if let Some(end) = part.find('"') {
                acc.push_str(&needle_start);
                acc.push_str(value);
                acc.push('"');
                acc.push_str(&part[end + 1..]);
            } else {
                acc.push_str(&needle_start);
                acc.push_str(part);
            }
            acc
        },
    )
}
```

Update the call site in `write_embedded_assets` to pass `inputs.theme_id` / `inputs.layout_id` (thread them through).

- [ ] **Step 3: Add `layout_id` to `BuildManifest`**

In `build_manifest.rs`, add `pub layout_id: String` and set it from `inputs.layout_id` at line ~228.

- [ ] **Step 4: Add a test for `replace_meta`**

```rust
#[test]
fn replace_meta_swaps_content() {
    let html = r#"<meta name="oxibuilder-theme" content="paper">"#;
    assert_eq!(replace_meta(html, "oxibuilder-theme", "midnight"),
        r#"<meta name="oxibuilder-theme" content="midnight">"#);
    // leaves unrelated tags intact
    let html2 = r#"<meta name="viewport" content="width=device-width">"#;
    assert_eq!(replace_meta(html2, "oxibuilder-theme", "x"), html2);
}
```

- [ ] **Step 5: Verify**

Run: `cargo test -p oxibuilder-core replace_meta`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-core/src/build_writer.rs crates/oxibuilder-core/src/build_manifest.rs
git commit -m "fix(core): inject actual theme+layout into build index.html meta; snapshot layout"
```

---

## Task 7: `theme-boot.js` + `index.html` layout meta

**Files:**
- Modify: `web/public/theme-boot.js`
- Modify: `web/index.html`

**Interfaces:**
- Consumes: `<meta name="oxibuilder-layout" content="shell">` (Task 6 rewrites at build time).
- Produces: `document.documentElement.dataset.layout` set pre-paint.

- [ ] **Step 1: Add layout to `index.html`**

Add next to the existing theme meta:

```html
<meta name="oxibuilder-layout" content="shell">
```

- [ ] **Step 2: Read + apply layout in `theme-boot.js`**

In the `// public` branch (after setting `data-public-theme`), add:

```js
var layoutMeta = document.querySelector('meta[name="oxibuilder-layout"]');
var layoutId = (layoutMeta && layoutMeta.content) || "shell";
document.documentElement.dataset.layout = layoutId;
```

- [ ] **Step 3: Verify manually**

Run: `bun run dev` → inspect `<html>` in DevTools → `data-layout="shell"` present before React mounts.

- [ ] **Step 4: Commit**

```bash
git add web/public/theme-boot.js web/index.html
git commit -m "feat(web): theme-boot reads layout meta, sets html[data-layout]"
```

---

## Task 8: Frontend types + `applyServerTheme` layout

**Files:**
- Modify: `web/src/shared/api.ts` (`ManifestSite` ~6-11)
- Modify: `web/src/shared/theme.ts` (`applyServerTheme`)

**Interfaces:**
- Produces: `ManifestSite.layout: 'shell' | 'editorial'`; `applyServerTheme` sets `html[data-layout]`.

- [ ] **Step 1: Add layout to `ManifestSite`**

```ts
export interface ManifestSite {
  name: string;
  base_url: string;
  default_lang: string;
  languages: string[];
  layout: 'shell' | 'editorial';
}
```

Make it optional-safe where the manifest may predate the field: `layout?: 'shell' | 'editorial'`.

- [ ] **Step 2: Publish layout in `applyServerTheme`**

In `theme.ts`, after resolving the theme definition, also set the layout. The theme fetch returns `layout`; for static mode read `data/theme.json.layout`. Set `document.documentElement.dataset.layout = layout ?? 'shell'`.

- [ ] **Step 3: Verify type-check**

Run: `bun run build`
Expected: PASS (tsc clean).

- [ ] **Step 4: Commit**

```bash
git add web/src/shared/api.ts web/src/shared/theme.ts
git commit -m "feat(web): ManifestSite.layout + applyServerTheme publishes data-layout"
```

---

## Task 9: `EditorialShell` + `EditorialPageHeader` + `HubLobby`

**Files:**
- Create: `web/src/shell/EditorialShell.tsx`
- Create: `web/src/shell/EditorialPageHeader.tsx`
- Create: `web/src/lobby/HubLobby.tsx`

**Interfaces:**
- Consumes: `fetchManifest`, `ThemeToggle`, `LangToggle`, `useLanguage`, existing UI primitives (`Container`, `Button`).
- Produces: `<EditorialShell/>` rendering the same `<Routes>` as `Shell` but chromeless; `<HubLobby/>`; `<EditorialPageHeader title count statsHref onBack/>`.

- [ ] **Step 1: `EditorialShell`**

```tsx
import { LanguageProvider, useLanguage, type Lang } from "../shared/language";
import { Routes, Route, Link } from "react-router";
// …lazy imports mirror App.tsx's route table…

export function EditorialShell({ routes }: { routes: React.ReactNode }) {
  // No header/nav/footer. Each page renders its own EditorialPageHeader.
  return (
    <LanguageProvider defaultLang={"ko" as Lang}>
      <main className="mx-auto w-full max-w-5xl px-5 py-10 sm:py-16">
        {routes}
      </main>
    </LanguageProvider>
  );
}
```

Note: factor the shared `<Routes>` tree out of `Shell` so both shells render the identical route table (Task 10). Keep `EditorialShell` chromeless — no `<header>`, no `<SiteFooter>`.

- [ ] **Step 2: `EditorialPageHeader`**

```tsx
import { ArrowLeft } from "lucide-react";
import { Link } from "react-router";
import { useLanguage } from "../shared/language";

export function EditorialPageHeader({
  title, count, statsHref,
}: { title: string; count?: number; statsHref?: string }) {
  const { lang } = useLanguage();
  return (
    <header className="mb-8 flex items-end justify-between gap-4 border-b border-line pb-4">
      <div className="flex items-center gap-3">
        <Link to="/" className="text-subtle hover:text-foreground transition-colors" aria-label="back">
          <ArrowLeft className="size-5" />
        </Link>
        <h1 className="font-serif text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
      </div>
      <div className="flex items-center gap-4 text-sm text-subtle">
        {count != null && <span>{count}</span>}
        {statsHref && (
          <Link to={statsHref} className="hover:text-foreground transition-colors">
            {lang === "en" ? "Stats" : "통계"}
          </Link>
        )}
      </div>
    </header>
  );
}
```

- [ ] **Step 3: `HubLobby`**

A centered identity block + a card grid of active extensions/mounts (reuse the manifest `extensions` + `mounts`). Border cards, hover `border-strong bg-surface-sunken`. At the bottom, a minimal control row with `ThemeToggle` + `LangToggle` (since there is no chrome to host them).

```tsx
export function HubLobby() {
  const { data: manifest } = useQuery({ queryKey: ["manifest"], queryFn: fetchManifest });
  const { pick, lang } = useLanguage();
  // …centered h1 = site name, subtitle = tagline…
  // …grid of cards: each extension → Link to `/${ext.id}`, each mount → external/plain link…
  // …footer row: <ThemeToggle/> <LangToggle/>…
}
```

Use the existing `EXT_ICONS` map from `Lobby.tsx` (export it or duplicate).

- [ ] **Step 4: Verify type-check**

Run: `bun run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/src/shell/EditorialShell.tsx web/src/shell/EditorialPageHeader.tsx web/src/lobby/HubLobby.tsx
git commit -m "feat(web): editorial chromeless shell, hub lobby, per-page header"
```

---

## Task 10: `App.tsx` layout branch + new routes

**Files:**
- Modify: `web/src/App.tsx`

**Interfaces:**
- Consumes: `EditorialShell`, `Shell`, `HubLobby`, `Lobby`, existing route components.

> **Route sequencing:** Task 10 does the layout branch + factored route table (EXISTING routes only). The new routes — `/movies/:slug` (Task 16), `/books/stats` (Task 14), `/movies/stats` (Task 13) — are added BY those tasks when their components are created, NOT here (the components don't exist yet; adding the routes here would break the build).

- [ ] **Step 1: Extract `LangToggle` to shared (addresses a Task-9 minor)**

The existing `LangToggle` is a non-exported local fn in `App.tsx` (~line 56), and `HubLobby` duplicated it. Move it to `web/src/shared/LangToggle.tsx` (exported), keep the `<Languages/>` icon, and consume it from BOTH `Shell` (App.tsx) and `HubLobby` (remove HubLobby's local copy). One source of truth.

- [ ] **Step 2: Factor the route table into `<SiteRoutes/>`**

Extract the EXISTING `<Routes>…</Routes>` block (App.tsx ~127-219) into a `<SiteRoutes/>` component. Both shells render it. Do NOT add the movie-detail/stats routes here. The `/` route element swaps on layout:

```tsx
<Route path="/" element={layout === "editorial" ? <HubLobby/> : <Lobby/>} />
```

- [ ] **Step 3: Branch `App` on layout**

```tsx
function App() {
  const layout = document.documentElement.dataset.layout ?? "shell";
  return (
    <QueryClientProvider client={queryClient}>
      <AssetResolverProvider mode="public">
        <BrowserRouter>
          {layout === "editorial" ? <EditorialShell><SiteRoutes/></EditorialShell>
                                  : <Shell><SiteRoutes/></Shell>}
        </BrowserRouter>
      </AssetResolverProvider>
    </QueryClientProvider>
  );
}
```

Adjust `Shell` to accept children (the `<SiteRoutes/>` table) instead of inlining routes. Keep `Shell`'s sticky header (now using the shared LangToggle)/nav/footer. `EditorialShell` stays chromeless.

- [ ] **Step 4: Verify type-check + runtime**

Run: `cd web && bun run build` → PASS. Then `bun run dev`, set `html[data-layout="editorial"]` in DevTools + reload → hub lobby renders, no sticky header; switch back to shell → sticky header returns.

- [ ] **Step 5: Commit**

```bash
git add web/src/App.tsx web/src/shared/LangToggle.tsx web/src/lobby/HubLobby.tsx
git commit -m "feat(web): branch App on layout; factor route table + shared LangToggle"
```

---

## Task 11: `[data-layout="editorial"]` token scope

**Files:**
- Modify: `web/src/shared/tokens.css`

- [ ] **Step 1: Add the editorial scope**

Add a `[data-layout="editorial"]` block adjusting whitespace density, card borders, and heading scale to match blog-test's calmer rhythm (e.g. larger page padding, lighter borders, tighter max-width). This is fine-tuning — keep it small and APCA-safe.

- [ ] **Step 2: Verify visually**

Run: `bun run dev`, toggle `data-layout` → editorial pages reflect the calmer spacing; shell pages unchanged.

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/tokens.css
git commit -m "style(web): editorial layout token scope"
```

---

## Task 12: `StatsKit` (BarRow, SummaryBand, ColumnChart)

**Files:**
- Create: `web/src/shared/stats/StatsKit.tsx`

**Interfaces:**
- Produces: `<BarRow name count max/>`, `<SummaryBand items={[{label,value}]}/>`, `<ColumnChart data={[{label,value}]} maxLabelLen={n}/>`. Pure CSS, no JS animation.

- [ ] **Step 1: Implement the three primitives**

```tsx
export interface CountRow { name: string; count: number; }

export function BarRow({ name, count, max }: { name: string; count: number; max: number }) {
  const pct = max > 0 ? (count / max) * 100 : 0;
  return (
    <div className="flex items-center gap-3 py-1">
      <span className="w-40 shrink-0 truncate text-sm text-foreground">{name}</span>
      <div className="relative h-2 flex-1 overflow-hidden rounded-full bg-surface-sunken">
        <div className="h-full rounded-full bg-interactive/70" style={{ width: `${pct}%` }} />
      </div>
      <span className="w-8 shrink-0 text-right text-sm text-subtle tabular-nums">{count}</span>
    </div>
  );
}

export function SummaryBand({ items }: { items: { label: string; value: string | number }[] }) {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
      {items.map((it) => (
        <div key={it.label} className="rounded-lg border border-line bg-canvas p-3">
          <div className="text-2xl font-semibold tabular-nums text-foreground">{it.value}</div>
          <div className="text-xs text-subtle">{it.label}</div>
        </div>
      ))}
    </div>
  );
}

export function ColumnChart({ data, max }: { data: { year: number; count: number }[]; max: number }) {
  const top = Math.max(max, 1);
  return (
    <div className="flex items-end gap-1 h-40">
      {data.map((d) => (
        <div key={d.year} className="flex-1 flex flex-col items-center justify-end gap-1" title={`${d.year}: ${d.count}`}>
          <div className="w-full rounded-t bg-interactive/70" style={{ height: `${(d.count / top) * 100}%`, minHeight: d.count > 0 ? "2px" : "0" }} />
          <span className="text-[10px] text-subtle tabular-nums">{d.year}</span>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Verify type-check**

Run: `bun run build`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/stats/StatsKit.tsx
git commit -m "feat(web): StatsKit pure-CSS chart primitives"
```

---

## Task 13: `computeMovieStats` + `MoviesStatsPage`

**Files:**
- Create: `web/src/shared/stats/computeMovieStats.ts`
- Create: `web/src/extensions/movies/MoviesStatsPage.tsx`

**Interfaces:**
- Consumes: `MovieEntry` (api.ts), `StatsKit`.
- Produces: `computeMovieStats(movies: MovieEntry[]) -> MovieStats`; `/movies/stats` page.

- [ ] **Step 1: Adapt stats to oxi's `MovieEntry` shape**

Port from `../blog-test/src/lib/movieStats.ts` but use oxi fields. oxi `MovieEntry` has `release_year`, `runtime_min`, `rating`, `genres[].name_en/ko`, `cast[]`, `directors[]`, `media_type` — NO `nation`. Compute: `total`, `directorCount`, `actorCount`, `yearMin/Max`, `avgRuntime`, `ratingMean`, plus `years[]`, `genres[]`, `actors[]` (from `cast`), `directors[]`, `runtimeBuckets[]` (<90/90-120/120-150/>150), `ratingBuckets[]` (0.5 steps). Drop `nations`.

```ts
import type { MovieEntry, MoviePerson } from "../../shared/api";

export interface MovieCountRow { name: string; count: number; }
export interface MovieStats {
  total: number;
  directorCount: number;
  actorCount: number;
  yearMin: number; yearMax: number;
  avgRuntime: number;
  ratingMean: number;
  years: { year: number; count: number }[];
  genres: MovieCountRow[];
  actors: MovieCountRow[];
  directors: MovieCountRow[];
  runtimeBuckets: MovieCountRow[];
  ratingBuckets: MovieCountRow[];
}

function topRows(map: Map<string, number>, limit = 15): MovieCountRow[] {
  return [...map.entries()].map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count).slice(0, limit);
}

export function computeMovieStats(movies: MovieEntry[]): MovieStats {
  // tally genres by localized name (en canonical), actors by name, directors by name,
  // years as a contiguous span yearMin..yearMax, runtime buckets, rating buckets (0.5 step).
  // …implement following blog-test's computeStats structure, reading oxi fields…
}
```

Use `pick(ko, en)`-style name resolution at the page layer (pass a `nameOf`), or canonicalize on `name_en` in the stats fn and localize in the page. Prefer canonicalizing on `name_en` (matches blog-test's TMDB-keyed approach).

- [ ] **Step 2: Port the assertion test**

Create `web/src/shared/stats/computeMovieStats.test.ts` (bun test) mirroring blog-test's `movieStats.test.ts` against a small fixture — assert total, top-genre, year-span continuity, runtime bucket sums equal total.

- [ ] **Step 3: `MoviesStatsPage`**

`SummaryBand` (편수/감독/배우/연도범위/평균 러닝타임/평균 평점) + `ColumnChart` (years) + `BarRow` lists (genres/actors/directors/runtime/rating). Wrap in `EditorialPageHeader title="영화 통계" statsHref` omitted (it IS the stats page) — use `PageTitle` in shell mode. Use `useQuery(fetchMovies)`.

- [ ] **Step 4: Verify**

Run: `bun test computeMovieStats` then `bun run build`
Expected: test PASS, tsc clean.

- [ ] **Step 5: Commit**

```bash
git add web/src/shared/stats/computeMovieStats.ts web/src/shared/stats/computeMovieStats.test.ts web/src/extensions/movies/MoviesStatsPage.tsx
git commit -m "feat(web): movie stats page (oxi-adapted)"
```

---

## Task 14: `computeBookStats` + `BooksStatsPage`

**Files:**
- Create: `web/src/shared/stats/computeBookStats.ts`
- Create: `web/src/extensions/books/BooksStatsPage.tsx`

**Interfaces:**
- Consumes: `Book` (api.ts), `StatsKit`.

- [ ] **Step 1: Adapt stats to oxi's `Book` shape**

oxi `Book` has `title`, `author` (single string), `rating`, `status` (wishlist/reading/completed/dropped), `published_at`, `created_at` — NO category/publisher/pages. Compute: `total`, `authorCount`, status distribution (`byStatus[]`), rating buckets (0.5 step) + `ratingMean`, year span from `published_at` (fallback `created_at`).

```ts
import type { Book } from "../../shared/api";

export interface BookStats {
  total: number;
  authorCount: number;
  ratingMean: number;
  yearMin: number; yearMax: number;
  years: { year: number; count: number }[];
  authors: { name: string; count: number }[];
  byStatus: { name: string; count: number }[];
  ratingBuckets: { name: string; count: number }[];
}

// parseAuthors: split oxi `author` on comma/semicolon, strip "(지은이)"/"(옮긴이)".
export function parseAuthorList(author: string | null): string[] { /* … */ }
export function computeBookStats(books: Book[]): BookStats { /* … */ }
```

- [ ] **Step 2: `BooksStatsPage`**

`SummaryBand` (총 권수/저자 수/평균 평점/연도 범위) + `ColumnChart` (years) + `BarRow` (authors, status, rating). `useQuery(fetchBooks)`.

- [ ] **Step 3: Verify**

Run: `bun run build`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add web/src/shared/stats/computeBookStats.ts web/src/extensions/books/BooksStatsPage.tsx
git commit -m "feat(web): book stats page (oxi-adapted)"
```

---

## Task 15: `useCollectionFilter` + `BooksPage` filter parity

**Files:**
- Create: `web/src/shared/useCollectionFilter.ts`
- Modify: `web/src/extensions/books/BooksPage.tsx`
- Modify: `web/src/extensions/movies/MoviesPage.tsx` (refactor to use the hook — optional but de-dupes)

**Interfaces:**
- Produces: `useCollectionFilter<T>({ items, textPred, sortFns, initialSort })` returning `{ query, setQuery, filtered, hasFilters, clearAll }`. Facet *computation* stays page-specific (genuinely differs between movies and books); only the generic search/sort/active-state chrome is shared.

- [ ] **Step 1: Implement the shared hook**

```ts
export function useCollectionFilter<T>(
  items: T[] | undefined,
  opts: {
    matches: (item: T, q: string) => boolean;
    sort: (items: T[], key: string) => T[];
    initialSort?: string;
  },
) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState(opts.initialSort ?? "default");
  const filtered = useMemo(() => {
    if (!items) return [];
    const q = query.trim().toLowerCase();
    let list = q ? items.filter((it) => opts.matches(it, q)) : [...items];
    return opts.sort(list, sort);
  }, [items, query, sort]);
  return { query, setQuery, sort, setSort, filtered, hasFilters: !!query, clearAll: () => setQuery("") };
}
```

- [ ] **Step 2: Bring `BooksPage` to parity**

Add: text search (title/author), status filter chips (wishlist/reading/completed/dropped with counts), category-free (no category field), sort (recent/rating/title). Use `useCollectionFilter` + local status state. Render with the existing `BookCard`. Keep the responsive 1→2→3 grid.

- [ ] **Step 3: (Optional) refactor `MoviesPage`** to use `useCollectionFilter` for the search/sort spine, keeping its bespoke genre/year/person facets. Skip if it risks regressions; the de-dupe is secondary.

- [ ] **Step 4: Verify**

Run: `bun run build`
Expected: PASS. Manually exercise books filters in `bun run dev`.

- [ ] **Step 5: Commit**

```bash
git add web/src/shared/useCollectionFilter.ts web/src/extensions/books/BooksPage.tsx
git commit -m "feat(web): books filter parity via useCollectionFilter"
```

---

## Task 16: Movie detail route + `MovieDetailPage`

**Files:**
- Create: `web/src/extensions/movies/MovieDetailPage.tsx`

**Interfaces:**
- Consumes: `fetchMovies` (find by `slug` param), `RatingStars`, `EditorialPageHeader`/`PageTitle`.

- [ ] **Step 1: Implement the detail page**

```tsx
export function MovieDetailPage() {
  const { slug } = useParams();
  const { pick, lang } = useLanguage();
  const { data: movies } = useQuery({ queryKey: ["movies"], queryFn: fetchMovies });
  const movie = movies?.find((m) => m.slug === slug);
  if (!movies) return <Skeleton/>;
  if (!movie) return <EmptyState title={lang === "en" ? "Not found" : "없는 영화"} />;
  // Large poster (TMDB w500), localized title (title_ko ?? title_en ?? title),
  // meta row (year · media_type · runtime · rating stars), genres chips,
  // synopsis (review_ko/en), cast list (name + character), directors.
}
```

The route is registered in Task 10. The SSG already emits `movies/{slug}/index.html` shells — the SPA route now resolves instead of 404.

- [ ] **Step 2: Verify**

Run: `bun run build` then `bun run dev`, navigate to `/movies/<a real slug>` → detail renders.

- [ ] **Step 3: Commit**

```bash
git add web/src/extensions/movies/MovieDetailPage.tsx
git commit -m "feat(web): movie detail page (fixes SSG shell 404)"
```

---

## Task 17: Font migration Fraunces → SUIT/SUITE

**Files:**
- Modify: `web/index.html` (font links)
- Modify: `web/src/shared/global.css` and/or `tokens.css` (`--font-sans`, `--font-display`, line-heights)

- [ ] **Step 1: Swap font links**

Replace the Fraunces / Noto Serif KR `<link>`s with SUIT + SUITE via jsDelivr:

```html
<link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/sun-typeface/SUIT@2/static/css/SUIT.css">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/sun-typeface/SUITE@2/static/css/SUITE.css">
```

- [ ] **Step 2: Map tokens**

`--font-sans: "SUIT", …;` `--font-display: "SUITE", …;` Update the `@theme inline` aliases so `font-serif`/display resolves to SUITE. Re-tune body `line-height` 1.55→1.50, small 1.5→1.45 (per UNIFIED-DESIGN.md Step 2).

- [ ] **Step 3: Verify visually**

Run: `bun run build` then `bun run dev` → headings render in SUITE, body in SUIT; no layout shift; both layouts.

- [ ] **Step 4: Commit**

```bash
git add web/index.html web/src/shared/global.css web/src/shared/tokens.css
git commit -m "feat(web): migrate fonts to SUIT/SUITE"
```

---

## Task 18: Admin ThemesPage layout selector

**Files:**
- Modify: `web/src/admin/themes/ThemesPage.tsx`
- Modify: `web/src/admin/shared/api.ts` (`setTheme` sends `layout`)

- [ ] **Step 1: Add a layout picker below the color-theme cards**

Two cards (Shell / Editorial) with previews; selecting PUTs `{ theme_id, layout }` to `/s/{slug}/theme` (Task 4). Read current `layout` from the GET response.

- [ ] **Step 2: Verify**

Run: `bun run build` (admin) → admin Themes page shows both axes; switching layout updates the live preview.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/themes/ThemesPage.tsx web/src/admin/shared/api.ts
git commit -m "feat(admin): layout variant selector on Themes page"
```

---

## Task 19: Config docs + end-to-end verification

**Files:**
- Modify: `oxibuilder.toml.example`

- [ ] **Step 1: Document `[lobby].layout`**

```toml
[lobby]
default_mode = "grid"
layout = "shell"      # "shell" (default) | "editorial"
```

- [ ] **Step 2: Rebuild web bundles**

```bash
cd web && bun run build && bun run build:static && cd ..
```

- [ ] **Step 3: Full workspace build + build a site**

```bash
cargo build
oxibuilder build
```

- [ ] **Step 4: Verify the FOUC fix + layout snapshot**

Inspect `out/index.html`: `<meta name="oxibuilder-theme" content="midnight">` (the ACTUAL active theme, not hardcoded `paper`) and `<meta name="oxibuilder-layout" content="...">`. Inspect `out/data/theme.json` carries `layout`. Inspect `.oxibuilder-build.json` carries `layout_id`.

- [ ] **Step 5: Smoke both layouts**

Set layout `editorial` via admin → `oxibuilder console` preview → hub lobby, chromeless, per-page headers, `/movies/<slug>` detail, `/books/stats` + `/movies/stats` render. Switch to `shell` → current look intact, sticky header + nav + footer.

- [ ] **Step 6: Commit**

```bash
git add oxibuilder.toml.example
git commit -m "docs: document [lobby].layout in config example"
```

---

## Self-Review notes

**Spec coverage:** §1 axis (Tasks 1-3) ✓ · §2 data model (Tasks 2-3) ✓ · §3 API (Task 4) ✓ · §4 build/FOUC (Tasks 5-6) ✓ · §5 components (Tasks 7-11) ✓ · §6a stats (Tasks 12-14) ✓ · §6b books filters (Task 15) ✓ · §6c movie detail (Task 16) ✓ · §7 fonts (Task 17) ✓ · admin (Task 18) ✓ · docs/verify (Task 19) ✓.

**Refinement vs spec:** the spec said "extract `useFacets`"; Task 15 implements `useCollectionFilter` for the generic search/sort spine only, keeping facet *computation* page-specific — movies' genre/year/person and books' status are genuinely different, so a one-size facet model would be forced. This satisfies the spec's intent (de-dupe the chrome) without over-abstracting.

**Data-shape realism:** stats tasks (13, 14) explicitly drop blog-test-only slices (movie `nation`, book `category`/`publisher`/`pages`) that oxi's models don't carry — flagged in Global Constraints.

**Type consistency:** `layout: 'shell' | 'editorial'` consistent across `ManifestSite`, `theme-boot.js`, `App.tsx`, `ThemePutInput`. `active_layout_id(db, default)` signature used in Tasks 1, 5.

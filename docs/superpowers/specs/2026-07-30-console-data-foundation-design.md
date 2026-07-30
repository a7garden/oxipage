# Console Data Foundation — Design Spec

> **Date:** 2026-07-30
> **Sub-project:** 1 of the decomposed "remaining console work" (Phases 7–14)
> **Scope:** P13 (server endpoints) + P11 (dashboard) + P7 (client-side search) + P10 (settings: integrations, language, Danger Zone)
> **Predecessor:** `2026-07-30-console-shell-redesign.md` (ConsoleShell + 6-page skeleton — complete)

## 1. Goal

Make the site-scoped console **data-functional**: replace the dashboard's 7-request count fan-out and blog-only recent list with single aggregated endpoints, wire the 7 content tabs' search inputs to real client-side filtering, activate Settings editing for integrations + languages, and enable registry-only site deletion.

This is dependency-layer-0. Later sub-projects (extension feature gaps P8, deploy streaming P9, global UX P12, chores P14) build on these endpoints.

## 2. Scope

### In scope
- **P13 — Server endpoints:** `GET /s/{slug}/stats`, `GET /s/{slug}/content/recent`, `DELETE /sites/{slug}` (registry-only), extend `PUT /s/{slug}/config` with integrations + languages.
- **P11 — Dashboard:** consume `/stats` + `/content/recent`; add storage stat card and last-build status.
- **P7 — Search:** client-side row filtering wired to the 7 content tabs' search inputs.
- **P10 — Settings:** integrations editing (3 fields), language array management, Danger Zone "Delete Site" activation.

### Out of scope (flagged, deferred)
- **Server-side full-text search:** the FTS5 `search_documents` index (`oxipage-core/src/search.rs`, migration `0002_search_documents.sql`) is populated at **publish** time and indexes published content only. It powers a future "global search" sub-project. Per-tab search in this spec is client-side filter only.
- **"Purge All Data":** the second Danger Zone button stays `disabled`. Bulk content deletion is a separate destructive action.
- **`set_default` server stub:** `router.rs::set_default` returns `{"ok":true}` without persisting. Separate issue.
- **P8, P9, P12, P14:** subsequent sub-projects.

## 3. Current State (grounding)

| Concern | Current state | File |
|---------|--------------|------|
| Dashboard counts | 7 parallel `contentClient.list(...).then(r => r.length)` | `web/src/admin/dashboard/DashboardPage.tsx:38-53` |
| Dashboard recent | fetch `/blog`, slice 5 (blog-only) | `DashboardPage.tsx:27-36` |
| `config_put` | accepts only `site?` + `lobby?` | `crates/oxipage-console/src/per_site.rs:59-63` |
| `SiteUpdate` | `name`, `base_url`, `default_lang` — no `languages` | `per_site.rs:65-70` |
| Integrations UI | read-only display, no inputs | `web/src/admin/settings/SettingsPage.tsx:136-148` |
| Danger Zone | both buttons `disabled` | `SettingsPage.tsx:150-161` |
| `removeSite` client | exists, calls `DELETE /api/console/sites/{slug}` | `web/src/admin/shared/api.ts:38-40` |
| DELETE server handler | **absent** — no route, `SiteRegistry` has no `remove_site` | `router.rs`, `sites_runtime.rs` |
| Build history | `build_log` table read by `builds_list` | `per_site.rs:179-202` |
| Extension tables | each extension declares `table_names()`; validated by `is_safe_ident` | `oxipage-core/src/extension.rs:172-175`, `http.rs:521-529` |
| FTS5 search index | `search_documents(extension_id, doc_id, title, body, lang, published_at)`; populated at publish via `reindex()`; drafts excluded | `oxipage-core/src/search.rs`, ext `routes.rs` reindex calls |
| Extension schemas | heterogeneous: blog `blog_post` & books `book_entry` have published_at+updated_at; links `link_card` has updated_at but **no published_at** | `oxipage-ext-*/migrations/0001_init.sql` |

## 4. Architecture

Aggregated per-site endpoints (Approach A from brainstorming). One focused request per dashboard concern instead of N extension fan-outs.

```
Dashboard ─┬─ GET /s/{slug}/stats         (counts + storage + last_build)
           └─ GET /s/{slug}/content/recent (cross-extension recent, updated_at)

Content tabs (7) ── client-side useRowFilter() on already-loaded list  (no endpoint)

Settings ─┬─ PUT /s/{slug}/config  {integrations?, site.languages?}
          └─ DELETE /sites/{slug}  (Danger Zone)
```

New server handlers live in `per_site.rs` (stats, recent) and `router.rs` (delete), consistent with existing per-site handler placement. Each handler receives `Extension<Arc<SiteContext>>` (stats, recent) or `State<Arc<SiteRegistry>>` (delete), matching the existing injection pattern.

## 5. Server Endpoints (P13) — contracts

### 5.1 `GET /api/console/s/{slug}/stats`

**Response 200:**
```jsonc
{
  "data": {
    "counts": { "blog": 12, "projects": 3, "books": 0 /* …per enabled extension */ },
    "storage_bytes": 4823104,
    "last_build": { "status": "success", "started_at": "2026-07-30T10:01:22", "finished_at": "2026-07-30T10:01:31" }
  }
}
```
- `counts`: for each **enabled** extension, `SELECT COUNT(*) FROM <table>` against that extension's **primary content table** (the main entity table — e.g. `blog_post`, `book_entry`), **not** every name in `table_names()` (which for multi-table extensions includes junction/secondary tables like chapters and would double-count). The plan maps extension id → primary table; for single-table extensions it is the only declared table. Table name interpolated only after `is_safe_ident` (defensive — names are `&'static str`, but follow the established guard). A table that fails to query is skipped (count omitted) — a disabled/broken extension must not 500 the whole stats call.
- `storage_bytes`: recursive sum of regular-file sizes under `ctx.path`, **excluding** the generated `out/` directory. Run in `tokio::task::spawn_blocking` (filesystem walk is blocking).
- `last_build`: reuse the same `build_log` read as `builds_list`, take row 0. `null` if no builds.

### 5.2 `GET /api/console/s/{slug}/content/recent?limit=5`

**Approach C (chosen): per-extension explicit recent queries, merged by `updated_at`.** Catches in-progress drafts; robust via explicit per-extension schema mapping rather than guessing columns.

**Response 200:**
```jsonc
{
  "data": [
    { "ext": "blog", "id": 42, "title": "…", "updated_at": "2026-07-30T09:00:00", "published_at": "2026-07-29T00:00:00" }
  ]
}
```
- For each **enabled** extension, run an **explicit** recent query against its primary content table selecting `(id, title, updated_at, published_at)` ordered `updated_at DESC LIMIT <limit>`. **No `published_at` filter** — drafts are included so the dashboard shows in-progress work.
- Merge all extensions' results by `updated_at DESC`, truncate to `limit`.
- `limit` clamped to `[1, 50]`, default `5`.
- **The plan enumerates each extension's exact recent query** (correct table + column names). Known starting points: blog `blog_post(id, slug, title, updated_at, published_at)`; books `book_entry(id, title, updated_at, published_at)`; links `link_card(id, <title-or-label>, updated_at)` — links has no `published_at` (return `null`). Movies/novels/projects/scraps verified in the plan against their `0001_init.sql`.
- An extension whose content table genuinely lacks `updated_at` is **omitted** from recent (not an error). The plan confirms none of the 7 core extensions are in that state; if one is, it is excluded and noted.

### 5.3 `DELETE /api/console/sites/{slug}` (registry-only)

**Response 200:** `{ "data": { "slug": "<slug>", "removed": true } }`
**Response 404:** unknown slug.

- Add `SiteRegistry::remove_site(&self, slug: &str)`:
  1. Remove the entry from the in-memory `sites: RwLock<HashMap<…>>`.
  2. Remove the slug from the persisted `sites_file` and write it back (mirror the existing `register_in_file` write path).
- **Files on disk are preserved.** Only the registration is removed (user-approved safe behavior).
- The per-site router is built once at startup; the removed slug's `/s/{slug}/*` routes remain mounted but `inject_site_context` returns `None` → 404. A deleted site is unreachable via the console until restart. Acceptable.
- Route registered in `build_top_level_router` next to the existing `sites` routes.

### 5.4 `PUT /api/console/s/{slug}/config` (extended)

Extend `ConfigUpdate` (`per_site.rs:59`) with:
```rust
pub struct ConfigUpdate {
    pub site: Option<SiteUpdate>,
    pub lobby: Option<LobbyUpdate>,
    pub integrations: Option<IntegrationsUpdate>,   // NEW
}

pub struct IntegrationsUpdate {
    pub github_username: Option<String>,
    pub tmdb_api_key_env: Option<String>,
    pub aladin_ttbkey_env: Option<String>,
}
```
Extend `SiteUpdate` (`per_site.rs:65`) with:
```rust
pub struct SiteUpdate {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub default_lang: Option<String>,
    pub languages: Option<Vec<String>>,   // NEW
}
```
- The TOML writer in `config_put` gains a branch for `[integrations]` (mirror the `[site]`/`[lobby]` table-merge pattern: `entry("integrations").or_insert(Table::new())`, write each present field) and writes `site.languages` as a TOML array when `Some`.
- **Response shape fix:** `config_put`'s response body (`per_site.rs:146-158`) currently omits `server`/`extensions`/`integrations` that the client `ConfigResponse` (api.ts:113-123) expects. Factor a shared `build_config_response(&Config)` helper used by both `config_get` and `config_put` so the client's `setQueryData` always gets a complete object.

## 6. Dashboard (P11)

Replace the two inefficient queries in `DashboardPage.tsx`:
- Remove the `["site", slug, "counts"]` 7-request fan-out (lines 38-53). Replace with a single `useQuery(["site", slug, "stats"], () => getStats(slug))`.
- Remove the blog-only recent query (lines 27-36). Replace with `useQuery(["site", slug, "recent"], () => getRecent(slug))`.
- StatCards: one card per count from `stats.counts` (ext label) **plus** a Storage card (human-readable bytes via a `formatBytes` util). Keep the responsive grid; allow wrap.
- Last-build status: a `Badge` (success/failed) + relative timestamp near the header actions.
- Recent list: cross-extension rows with an extension-name badge column; edit action navigates to `/s/${slug}/content` (existing behavior).

New client functions in `api.ts`: `getStats(slug): Promise<Stats>` and `getRecent(slug, limit?): Promise<RecentItem[]>`, both via `siteScopedFetch` + `jsonOrThrow`.

## 7. Search (P7) — client-side filter

- Shared util `web/src/admin/shared/useRowFilter.ts`:
  ```ts
  export function useRowFilter<T>(rows: T[], query: string, keys: (row: T) => string[]): T[]
  ```
  Debounced (150ms) case-insensitive substring match across the concatenated key values. Empty query returns all rows.
- Each of the 7 content tabs (`BlogTab`, `BooksTab`, `LinksTab`, `MoviesTab`, `NovelsTab`, `ProjectsTab`, `ScrapsTab`) wires its existing search-input state into `useRowFilter(rows, query, rowKeys)`, where `rowKeys` is the relevant text field(s) per extension (title / name / slug — tab-specific).
- No server changes. No new endpoints.

## 8. Settings (P10)

### 8.1 Integrations editing
- Replace the read-only `<div>` block (`SettingsPage.tsx:143-147`) with three editable `SettingsField` inputs bound to local state (`githubUsername`, `tmdbApiKeyEnv`, `aladinTtbkeyEnv`), initialized from `data.integrations`.
- Extend the `save` mutation to send `integrations: { github_username, tmdb_api_key_env, aladin_ttbkey_env }` alongside `site`/`lobby` (requires the §5.4 client `updateConfig` patch type to accept `integrations?`).

### 8.2 Language management
- Replace the hardcoded `["ko","en"]` `defaultLang` select with a select populated from `data.site.languages`.
- Add a `languages[]` chip editor: list of language codes with add (text input + add button) and remove (× on chip). Bound to a `languages: string[]` local state.
- `save` sends `site: { name, base_url, default_lang, languages }`.

### 8.3 Danger Zone — Delete Site
- Activate the "Delete Site" button. "Purge All Data" stays `disabled`.
- Click → confirmation modal requiring the user to type the site slug exactly to enable the confirm button (prevents accidental destructive action).
- Confirm → `removeSite(slug)` (api.ts:38) → on success `navigate("/sites")` and invalidate `["sites"]`. Inline success/error message.

## 9. Constraints

- All new server handlers reuse the existing `Extension<Arc<SiteContext>>` / `State<Arc<SiteRegistry>>` injection — no new middleware.
- Table-name interpolation must pass `is_safe_ident` (never format a raw string into SQL).
- `storage_bytes` traversal runs in `spawn_blocking`.
- Every new client query handles loading (Skeleton), empty (EmptyState), and error (inline retry) per the shell-redesign global constraints.
- Reuse existing `web/src/shared/ui/` components (Button, Badge, Input, Skeleton, EmptyState) — no new UI primitives.
- API path prefix is `/api/console` (not `/api/v1`) — all fetches go through `siteScopedFetch` / the `/api/console` base.

## 10. Testing

- **Server (Rust):**
  - `stats` returns correct counts for a fixture site with known table rows; `storage_bytes > 0`; `last_build` reflects an inserted `build_log` row; `null` last_build when empty.
  - `content/recent` merges across two extensions ordered by `updated_at`, respects `limit`, and includes a draft (published_at NULL).
  - `delete_site` removes the slug from an in-memory registry + writes the sites file; 404 for unknown slug; site directory files untouched.
  - `config_put` with `integrations` + `languages` round-trips through TOML and the response carries the full `ConfigResponse` shape.
- **Client (TS):** `useRowFilter` filters by substring, empty query passes through, is case-insensitive.
- **Manual smoke:** `oxipage console` → dashboard loads in ≤2 requests; each content tab filters live; settings integrations + languages save and round-trip; Danger Zone deletes a throwaway site and redirects to `/sites`.

## 11. File map

```
crates/oxipage-console/src/
├── per_site.rs              # +stats_get, +recent_get; extend ConfigUpdate/SiteUpdate + IntegrationsUpdate; fix config_put response
├── router.rs                # +delete_site handler + route; register in build_top_level_router
└── sites_runtime.rs         # +SiteRegistry::remove_site

web/src/admin/
├── shared/
│   ├── api.ts               # +getStats, +getRecent; extend updateConfig patch (integrations, languages)
│   └── useRowFilter.ts      # NEW — shared client filter hook
├── dashboard/DashboardPage.tsx   # replace fan-out + blog-only recent; storage card; last-build badge
├── settings/SettingsPage.tsx     # integrations inputs; languages chip editor; Danger Zone delete modal
└── content/*Tab.tsx             # wire search input → useRowFilter (7 files)
```

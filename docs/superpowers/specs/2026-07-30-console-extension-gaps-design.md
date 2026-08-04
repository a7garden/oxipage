# Console Extension Gaps — Design Spec

> **Date:** 2026-07-30
> **Sub-project:** 2 of the decomposed "remaining console work" (Phases 8)
> **Scope:** Novels chapter management, Movies series groups, Projects screenshots, WASM runtime install UI.
> **Predecessor:** `2026-07-30-console-data-foundation-design.md` (Sub-project 1 — stats/recent/delete/config).

## 1. Goal

Complete the four extension feature gaps left as list-only skeletons after Sub-project 1. The console currently manages top-level content rows only; child entities (novel chapters, movie screenshots' siblings, project screenshots) and WASM runtime install have no UI.

**Key correction from the source task list:** the source doc assumed the chapter/series/screenshot *endpoints* needed to be created. They mostly **already exist**. S2 is ~70% frontend + ~30% small backend fills (three missing handlers). Novels chapter backend is **complete** — zero backend work there.

## 2. Scope

### In scope
- **Novels chapters (P8):** chapter management UI inside the novel edit Drawer. Backend untouched (full CRUD already mounted).
- **Movies series groups (P8):** `[Movies | Series]` sub-view toggle; series group CRUD + member assignment. Backend gap: add `update_group` + `delete_group`.
- **Projects screenshots (P8):** screenshot management inside the project edit Drawer. Backend gap: add `update_screenshot` (alt + display_order).
- **WASM install (P8):** "Available from Registry" section in ExtensionsPage. Backend gap: add `GET /extensions/registry` (list installable).
- **Client infrastructure:** sub-resource client functions (the flat `contentClient` does not fit nested paths).

### Out of scope (flagged, deferred)
- **File upload for screenshots:** screenshots are URL-based (`screenshot.url: TEXT`). A media/upload endpoint is a separate concern.
- **Chapter body markdown preview:** plain `<Textarea>` here; the live preview is Sub-project 5.
- **Movies TMDB auto-fill in console:** the `/movies/search` endpoint exists but a guided add-from-TMDB flow is out of scope (manual entry via the existing drawer).

## 3. Current State (grounding)

| Concern | Current state | File |
|---------|--------------|------|
| Novels chapter backend | **COMPLETE** — list/draft-list/create/show/update/delete/publish | `ext-novels/routes.rs:114-228`, `lib.rs:135-149` |
| Novels chapter UI | none — `NovelsTab` manages the novel list only | `web/src/admin/content/NovelsTab.tsx` |
| Movies series backend | PARTIAL — `create_group`/`list_groups`/`show_group`; **no update/delete** | `ext-movies/routes.rs:233-312`, `lib.rs:133-137` |
| Movies series UI | none | `web/src/admin/content/MoviesTab.tsx` |
| Movies entry→series link | exists — `MovieEntryInput`/`Patch` carry `series_group_slug` + `series_order` | `ext-movies/model.rs`, `repo.rs:244-334` |
| Projects screenshot backend | PARTIAL — `add_screenshot`/`delete_screenshot`; `GET project` returns screenshots via `ProjectDetail`; **no update/reorder** | `ext-projects/routes.rs:141-175`, `repo.rs:244-305` |
| Projects screenshot UI | none | `web/src/admin/content/ProjectsTab.tsx` |
| WASM install | `POST /api/console/extensions/install {name}` exists (global); **no registry-list endpoint** | `oxibuilder-core/src/http.rs:645-796` |
| WASM install UI | none — `ExtensionsPage` toggles installed only | `web/src/admin/extensions/ExtensionsPage.tsx` |
| `contentClient` | flat `/{extId}/{id}` — does not fit `/{extId}/{slug}/chapters` | `web/src/admin/shared/api.ts:228-289` |

## 4. Architecture

Sub-resource UI pattern (user-approved): **parent edit Drawer gains a child section** (accordion), with inline child editing inside the same Drawer. Exception: **Movies series groups are top-level entities** (not children of a single movie), so they get a dedicated `[Movies | Series]` sub-view rather than a drawer section.

```
NovelsTab
  └ novel edit Drawer
       └ "Chapters" accordion  ─ chapter list (order) + inline add/edit/publish/delete + ↑↓ reorder

MoviesTab
  ├ [Movies | Series] toggle
  ├ Movies view (existing)
  │    └ movie edit Drawer  └ "Series" field (group dropdown + series_order)
  └ Series view (new)  ─ group list + create/edit/delete Drawer; group row → show_group members + unassign

ProjectsTab
  └ project edit Drawer
       └ "Screenshots" accordion  ─ URL-based list + alt_ko/alt_en + ↑↓ reorder + delete

ExtensionsPage
  └ "Available from Registry" section  ─ GET /extensions/registry → [Install] → POST install
```

New server handlers mirror existing extension handler placement (`routes.rs` + `repo.rs` + `lib.rs` route mount). The registry-list handler lives in `oxibuilder-core/src/http.rs` next to `extension_install` (it reads the same embedded `_registry.json`).

## 5. Backend fills — contracts

### 5.1 Movies series group update/delete

**`PATCH /api/console/s/{slug}/movies/series/{groupslug}`**
```jsonc
// request
{ "title_ko"?: "...", "title_en"?: "...", "cover_image"?: "...",
  "group_rating"?: 8, "group_review_ko"?: "...", "group_review_en"?: "..." }
// 200 → { "data": SeriesGroup }   // 404 unknown group
```
- `repo::update_group(pool, slug, patch)` — partial update, `None` = unchanged (mirror `update_entry`).
- `routes::update_group` + mount `PATCH /series/{slug}` next to the existing `GET /series/{slug}`.

**`DELETE /api/console/s/{slug}/movies/series/{groupslug}`**
- `repo::delete_group(pool, slug)` — deletes the `series_group` row. Member `movie_entry` rows are **unassigned** (set `series_group_id = NULL`), not deleted (referential safety). Confirm in plan.
- `routes::delete_group` + mount `DELETE /series/{slug}`.

Member assignment/removal uses the **existing** `MovieEntryPatch.series_group_slug` + `series_order` (PATCH `/movies/{slug}`) — no new endpoint.

### 5.2 Projects screenshot update

**`PATCH /api/console/s/{slug}/projects/{slug}/screenshots/{sid}`**
```jsonc
{ "alt_ko"?: "...", "alt_en"?: "...", "display_order"?: 2 }
// 200 → { "data": Screenshot }
```
- `repo::update_screenshot(pool, project_slug, sid, patch)` — partial update of alt + display_order.
- `routes::update_screenshot` + mount `PATCH /{slug}/screenshots/{sid}` next to `DELETE`.
- Reorder = a series of `display_order` PATCHes (client-side ↑↓ buttons → optimistic + invalidate). No dedicated bulk-reorder endpoint.

### 5.3 WASM registry list

**`GET /api/console/extensions/registry`** (global, not site-scoped)
```jsonc
{ "data": [
  { "name": "wasm-demo", "runtime_loadable": true, "installed": false,
    "description": "…", "source": "embedded" | "remote" }
] }
```
- Reads the embedded `_registry.json` (`REGISTRY_INDEX_JSON`), filters `runtime_loadable == true`, annotates each with `installed` by checking the active registry/`extension_state`. Lives in `http.rs` next to `extension_install`.
- `installed`: `state.registry.find(name).is_some()`.

## 6. Frontend

### 6.1 Client (`api.ts`)

The flat `contentClient` (`/{extId}/{id}`) is reused for top-level rows. New sub-resource functions use `siteScopedFetch` directly:

```ts
// chapters
listChapters(slug, novelSlug, draft=false): Promise<NovelChapter[]>
createChapter(slug, novelSlug, input): Promise<NovelChapter>
updateChapter(slug, novelSlug, order, patch): Promise<NovelChapter>
deleteChapter(slug, novelSlug, order): Promise<void>
publishChapter(slug, novelSlug, order): Promise<NovelChapter>

// series
listSeries(slug): Promise<SeriesGroup[]>
createSeries(slug, input): Promise<SeriesGroup>
showSeries(slug, groupSlug): Promise<SeriesGroupDetail>  // { group, entries }
updateSeries(slug, groupSlug, patch): Promise<SeriesGroup>
deleteSeries(slug, groupSlug): Promise<void>

// screenshots (also returned inline by showExtension<Project>)
addScreenshot(slug, projectSlug, input): Promise<Screenshot>
updateScreenshot(slug, projectSlug, sid, patch): Promise<Screenshot>
deleteScreenshot(slug, projectSlug, sid): Promise<void>

// registry
listRegistry(): Promise<RegistryEntry[]>
installExtension(name): Promise<{ name, activated: boolean, note?: string }>
```

Query keys: `["site",slug,"novels",novelSlug,"chapters"]`, `["site",slug,"movies","series"]`, `["site",slug,"movies","series",groupSlug]`, `["site",slug,"projects",projSlug,"screenshots"]` (or reuse the `show` ProjectDetail inline list), `["extensions","registry"]`.

### 6.2 NovelsTab — Chapters accordion

Inside the novel edit Drawer (existing `editing`/`form` state), add a collapsible **"Chapters"** section:
- Fetch `listChapters(slug, novel.slug)` (or reuse `show_novel` if it embeds chapters — plan confirms; currently `show_novel` returns `Novel` only, so a separate fetch is needed).
- List rows ordered by `chapter_order`: title, `char_count`, publish badge.
- Inline add: `chapter_order` auto-assigned (max+1); title + body `<Textarea>` (Drawer scrolls for long prose).
- Inline edit (expand row) + publish toggle + delete.
- ↑↓ reorder → `updateChapter(..., { chapter_order })` optimistic + invalidate.

### 6.3 MoviesTab — Series sub-view

- Add `[Movies | Series]` toggle at the top of `MoviesTab`.
- **Movies view** (existing list): the movie edit Drawer gains a **"Series"** field — a `<select>` of `listSeries(slug)` (or "None") + `series_order` number input. Saving PATCHes the entry with `series_group_slug` + `series_order`.
- **Series view** (new): group list (title, member count) + `[New Series]` → create/edit Drawer (`title_ko/title_en/cover_image/group_rating/group_review`). Group row click → `showSeries` member list with `[Unassign]` per member (PATCH entry `series_group_slug = null`) + `[Delete Group]` (member entries unassigned, not deleted).

### 6.4 ProjectsTab — Screenshots accordion

Inside the project edit Drawer, add a **"Screenshots"** accordion:
- `showExtension<Project>` (`GET /projects/{slug}`) already returns `ProjectDetail { ...project, screenshots }` — reuse the inline list; no separate fetch.
- Each row: thumbnail (`<img src={url}>`), `alt_ko`/`alt_en` editable, `display_order`, ↑↓ reorder (`updateScreenshot`), delete.
- Add row: `addScreenshot({ url, alt_ko?, alt_en?, display_order })`. URL-based — paste an image URL, no file upload.

### 6.5 ExtensionsPage — Available from Registry

Below the existing "Installed" grid, add **"Available from Registry"**:
- `useQuery(["extensions","registry"], listRegistry)` → render `runtime_loadable` entries not yet installed.
- Each card: name + `[Install]` button → `installExtension(name)` mutation → on success: invalidate `["extensions"]` + `["extensions","registry"]`. Show the response `activated` flag: `true` → "Activated", `false` → "Restart oxibuilder-console (built with `--features wasm`) to activate".

## 7. Constraints

- Reuse existing UI primitives (`Drawer`, `DrawerField`, `Button`, `Badge`, `Input`, `Textarea`, `Skeleton`, `EmptyState`) — no new primitives.
- Sub-resource client functions use `siteScopedFetch` + `jsonOrThrow` (not the flat `contentClient`).
- Every child list honors loading (Skeleton), empty (EmptyState), error (inline retry) per the shell-redesign global constraints.
- Optimistic updates for reorder operations; `onError` rollback + `onSettled` invalidate.
- Table-name interpolation in any new repo SQL passes `is_safe_ident` (defensive; slugs are path params).
- API path prefix is `/api/console` (per-site via `/s/{slug}/…`; registry/install are global `/extensions/…`).

## 8. Testing

- **Server (Rust):**
  - `update_group` partial patch round-trips; `delete_group` removes the row and NULLs member `series_group_id`; 404 for unknown group slug.
  - `update_screenshot` updates alt + display_order; 404 for unknown sid.
  - `GET /extensions/registry` returns only `runtime_loadable` entries, with correct `installed` flags.
- **Client (manual smoke):** `oxibuilder console` → open a novel, add/reorder/publish/delete chapters in the Drawer; in Movies, create a series, assign two movies, unassign one, delete the group; in Projects, add a screenshot URL, reorder, edit alt; in Extensions, install `wasm-demo` from the registry and see activation status.

## 9. File map

```
crates/oxibuilder-ext-movies/src/
├── routes.rs   # +update_group, +delete_group handlers
├── repo.rs     # +update_group, +delete_group (NULL members)
└── lib.rs      # mount PATCH/DELETE /series/{slug}

crates/oxibuilder-ext-projects/src/
├── routes.rs   # +update_screenshot handler
├── repo.rs     # +update_screenshot
└── lib.rs      # mount PATCH /{slug}/screenshots/{sid}

crates/oxibuilder-core/src/
└── http.rs     # +registry_list handler + route GET /extensions/registry

web/src/admin/
├── shared/
│   └── api.ts                    # +chapters/series/screenshots/registry client fns
├── content/
│   ├── NovelsTab.tsx             # +Chapters accordion in edit Drawer
│   ├── MoviesTab.tsx             # +[Movies|Series] toggle; Series field in edit Drawer; Series view
│   └── ProjectsTab.tsx           # +Screenshots accordion in edit Drawer
└── extensions/
    └── ExtensionsPage.tsx        # +Available from Registry section
```

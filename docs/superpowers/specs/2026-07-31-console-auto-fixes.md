# Oxibuilder Console Fixes — Design Spec

> **Date:** 2026-07-31 · **Mode:** Autonomous (no interactive approval; standard choices)
> **Task source:** `.omp/auto-task-2026-07-31.md` (5 issues)

## Context — actual state vs. task assumptions

The task document's premises were **heavily stale**. A forensic pass (build + curl
reproduction + three read-only scouts) against *current* code shows most of the
requested work is already shipped. This spec records the real gaps and the focused
fixes that close them.

| # | Issue (as documented) | Actual state | Verdict |
|---|---|---|---|
| 1 | `/sites` serves raw HTML | Clean rebuild serves identical `admin.html` (1007 B) for `/`, `/sites`, `/admin.html`, nested routes; all hashed assets 200; `AdminErrorBoundary` recovers stale-build chunk-load failures | **DONE — no change** |
| 2 | Theme system incomplete | Console appearance (data-theme + localStorage via `theme-boot.js` → `theme.ts` → shared `ThemeToggle`) fully wired; `/api/console/themes` catalog + per-site GET/PUT implemented; `ThemesPage` picker works. **One real bug** + minor gaps | **FIX** |
| 3 | No preview button | `DeployPage` already has a "Preview Site ↗" button; `/api/console/preview/{slug}/*` fully implemented | **DONE — no change** |
| 4 | No GitHub Pages deploy UI | Full UI in `SettingsPage` (owner/repo/branch → `[deploy.github_pages]`), preflight, `POST /deploy` (202) → SSE stream → real git worktree + `gh-pages` push via `gh` CLI | **DONE — no change** |
| 5 | Extension content screen gaps | Image upload (ImageField → media endpoint), Profile admin tab both **already exist**. Real bugs: BooksTab drops `cover_image_url`, ProjectsTab broken thumbnails, dead validators, disabled-ext tabs shown | **FIX** |

## Fixes

### F1 — Per-site theme palette never reaches the console (Issue #2, the real bug)

**Root cause.** `ThemeBootstrap` (`App.tsx:14`) is mounted as a sibling of `<Routes>`
(`App.tsx:71`), *outside* any `<Route>`. `useParams()` therefore always returns `{}`
→ `slug` is always `undefined` → `applyServerTheme()` only ever fetches the
**default** site theme (`GET /api/console/theme`), never the per-site palette
(`GET /api/console/s/{slug}/theme`). The effect also runs once (deps never change).

**Fix.** Delete `ThemeBootstrap`. Move the `applyServerTheme(slug)` effect into
`ConsoleShell`, which is the layout element of the matched route hierarchy —
`useParams()` there correctly resolves `:slug` for `/s/:slug/*` and falls back to
`undefined` (→ default theme) on `/sites`, `/`. React Router resolves the full
match before rendering, so the layout element sees descendant params.

### F2 — ThemesPage apply doesn't republish (Issue #2, secondary)

After `PUT /theme` succeeds, `ThemesPage.apply` only updates the query cache; it
does not call `applyServerTheme`, so the in-console public-site preview does not
live-update. Add `applyServerTheme(slug)` to `onSuccess`.

### F3 — EditorPreviewDrawer hardcodes `paper` (Issue #2, secondary)

`PublicThemeScope` wraps its subtree in `data-public-theme="paper"` regardless of
the site's configured theme. Read the active theme from
`document.documentElement.dataset.publicTheme` (set by `applyServerTheme`) instead,
falling back to `paper`.

### F4 — BooksTab silently drops cover image (Issue #5, data-loss bug)

The save payload (`BooksTab.tsx:82`) omits `cover_image_url` even though the form
captures it (`L44`, `L128`). Add `cover_image_url: form.cover_image_url || null`.

### F5 — Wire dead validators (Issue #5)

`validateIsbn13` and `clampRating` exist (`validation.ts`) but are unused. Wire
ISBN-13 + rating validation into BooksTab's save path; reject the submit with an
inline error on failure (server remains authoritative).

### F6 — ProjectsTab broken screenshot thumbnails (Issue #5)

`<img src={s.url}>` (`ProjectsTab.tsx:319`) renders raw logical `media/` paths,
which 404 to the SPA fallback (broken images). Resolve through
`adminAssetResolver(slug)` (the same resolver ImageField uses for previews).

### F7 — ContentPage shows tabs for disabled extensions (Issue #5, UX)

`ContentPage` hardcodes all 8 tabs unconditionally. Query
`listExtensions(slug)` and hide tabs whose extension is not enabled (`profile`
stays always-on as the default tab).

### F8 — Remove dead constant (Issue #3/#4 cleanup)

`DeployPage.tsx` defines `PREVIEW_DISABLED_CODES` which is unused. Delete it.

## Out of scope (documented, not silently dropped)

- **Generalizing `EditorPreviewDrawer` to all 7 content editors.** The preview
  infra is tailored to `ProfileView` (a full public render component). Each
  extension would need its own public render component embedded in the drawer —
  a genuine multi-file feature, not a bug fix. Risk of shipping broken stubs in
  an autonomous run is high. Recommend a dedicated follow-up.
- **Broad `onError` hardening across every sub-mutation in all tabs.** Real but
  diffuse; better as a focused error-surface pass (e.g. a shared toast hook).
- **Sidebar token/Tailwind `@theme inline` exposure (5/9).** Cosmetic; the
  sidebar is already theme-aware via inline CSS vars.

## Verification

`cd web && bun run build` (tsc + vite) then `cargo build` (re-embed SPA). Smoke
each fix: theme palette switches with slug; book cover persists; project
thumbnails render; disabled-ext tabs hidden.

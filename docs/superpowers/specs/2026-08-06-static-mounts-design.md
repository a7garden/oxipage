# Static Mounts — Design Spec

**Date:** 2026-08-06
**Status:** Draft (pending user spec-review)
**Feature:** Graft arbitrary external static sites into the oxibuilder site at configurable URL prefixes, discoverable from the lobby.

## Goal

Let a user mount an external directory of static files (hand-written HTML, or the build output of any external tool — Astro, another SSG, etc.) at a URL prefix inside the oxibuilder site. The mounted pages are served as-is (their own styling), and appear as a link card in the lobby so visitors can discover them.

**Concrete driver:** a hand-crafted HTML portfolio at `../portfolio` to be served at `/portfolio/`, with a "Portfolio" card in the lobby.

## Background

Oxibuilder is a static site generator. `oxibuilder build` runs every `BuildExt` extension in parallel (`build_site`), then `write_build_output` materializes `out/`:

```
out/
├── index.html              SPA lobby (embedded bundle, <base href> injected)
├── 404.html                SPA deep-link fallback
├── assets/…                hashed JS/CSS (embedded bundle)
├── <ext>/<slug>/index.html extension SEO shells (hydrate SPA)
├── data/<ext>.json         SPA collection data
├── data/lobby.json         lobby manifest (static mode)
├── media/ , media/_derived/  raw uploads + optimized WebP
└── .oxibuilder-build.json  BuildManifest (deployment_base, theme, revision)
```

A static host (GitHub Pages) serves files by path, so `out/<path>/index.html` is reachable at `{deployment_base}<path>/` with no server-side routing.

**Key constraint.** `BuildExt` emits only `StaticPage` (an HTML string + path), JSON `data`, and `search_docs`. It cannot emit arbitrary binary assets (images, fonts, CSS, JS) at nested paths. A portfolio carries exactly those (`index.html` + `styles.css` + `assets/` images + sub-project dirs). A static mount therefore needs a **new core mechanism — a raw directory copy** — not a `BuildExt` extension.

The lobby manifest is assembled in **one** function, `manifest::assemble()`, consumed by both the live `GET /api/console/lobby/manifest` handler (`http.rs`) and the SSG build (`data/lobby.json`). Adding mounts there propagates to both automatically with no drift.

**Lobby serving boundary.** The live console server (`oxibuilder console`) serves only `admin.html` via its `static_handler` fallback — the public lobby (`index.html`) is *not* served live; it exists only in the built `out/` (and `--preview` serves `out/`). Therefore the lobby card only ever renders where mounts are already materialized, so no live-console mount serving is needed.

## Decisions (from brainstorming)

1. **Integration depth — lobby-integrated.** Mounts appear as generic link cards; their styling stays self-contained (not wrapped in the oxibuilder theme).
2. **Source model — live reference.** A mount points to an external directory; the build copies its *current* contents. The source must exist at build time. Edits flow through on rebuild.
3. **Approach — config-driven (A).** `[[mounts]]` in `oxibuilder.toml`. No DB table, no new CLI subcommand, no console management UI (beyond the lobby card). Approach B (DB + `oxibuilder mount` CLI + console UI) is a documented future path that reuses this manifest field + SPA card unchanged.
4. **Default chosen:** Source path resolution base = the directory containing the active config file.

   *A second candidate — also serving mounts from the live console (`/{path}/*` → source dir) — was investigated and dropped. The lobby `index.html` is not served by the live console at all (the `static_handler` serves only `admin.html`); the lobby exists only in the built `out/`, where mounts are already materialized by the build. So the lobby card never renders against a missing mount, and live-console serving is unnecessary (YAGNI).*

## Design

### 1. Config schema — `crates/oxibuilder-core/src/config.rs`

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MountConfig {
    pub id: String,                 // unique; reused as the manifest mount id
    pub source: PathBuf,            // dir path; resolved to absolute at load
    pub path: String,               // URL prefix, e.g. "portfolio" -> /portfolio/
    pub title_ko: String,
    pub title_en: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub open_in_new_tab: bool,
}
```

Add to `Config` (with `#[serde(default)]` so existing configs are unaffected):

```rust
#[serde(default)]
pub mounts: Vec<MountConfig>,
```

TOML example:

```toml
[[mounts]]
id        = "portfolio"
source    = "../portfolio"
path      = "portfolio"
title_ko  = "포트폴리오"
title_en  = "Portfolio"
description = "Hand-crafted work"
icon      = "🖼️"
```

**Path resolution.** `source` is normalized to an absolute path at load time, relative to the config file's parent directory. Add `Config::resolve_mount_sources(base: &Path)`; `Config::load(path)` calls it with `path.parent()`. Absolute `source` values pass through unchanged. Callers that build a `Config` via `from_toml_str` (tests) pass an explicit base dir.

### 2. Validation

Performed in `resolve_mount_sources` (or a small `validate_mounts` helper it calls):

- `id` unique across mounts.
- `path` unique; normalized by stripping leading/trailing `/`.
- `path` rejects: empty; any segment equal to `..` or `.`; absolute paths.
- `path` rejects reserved prefixes: `assets`, `data`, `media`, `api`, `search`, `s`, `admin`, `lobby`, `theme`. (These collide with core `out/` paths.)
- `source` missing at load → `tracing::warn!` (non-fatal; the config may be loaded where the source is legitimately absent). Missing at build → hard error (cannot copy).

### 3. Build copy step — `builder.rs` + `build_writer.rs`

New type and `BuildInputs` field:

```rust
#[derive(Debug, Clone)]
pub struct MountCopy {
    pub source: PathBuf,   // absolute
    pub path: String,      // normalized URL prefix / out subdir
}

// BuildInputs gains:
pub mounts: Vec<MountCopy>,   // default empty
```

`write_build_output` gains a step (after the media / derived-image copy, before the manifest write):

```rust
for mount in &inputs.mounts {
    let dst = out_dir.join(&mount.path);
    copy_dir_recursive(&mount.source, &dst)?;
}
```

Reuses the existing `copy_dir_recursive` helper. Mounts copy *after* core assets, so each mount is isolated under `out/{path}/`; the reserved-prefix validation in §2 guarantees no clobber of core paths.

**Three call sites thread `config.mounts` (resolved) → `inputs.mounts`**, each mapping `MountConfig` → `MountCopy { source, path }`:

- `crates/oxibuilder-cli/src/commands/build.rs` — CLI `oxibuilder build` (config already loaded there).
- `crates/oxibuilder-console/src/build/build_run.rs` — console background build (config in `AppState`).
- `crates/oxibuilder-core/src/http.rs` — on-demand build endpoint (config in scope).

### 4. Manifest — `manifest.rs`

```rust
#[derive(Serialize)]
pub struct ManifestMount {
    pub id: String,
    pub display_name: ManifestLocalized,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub open_in_new_tab: bool,
}

// Manifest gains:
pub mounts: Vec<ManifestMount>,
```

`assemble()` appends `config.mounts` → `ManifestMount` (no DB access needed). Both consumers — the live handler (`http.rs`) and the SSG `data/lobby.json` writer (`cli/commands/build.rs`) — inherit it from the single assembly.

### 5. SPA link card — `web/src`

The lobby renders `manifest.mounts` as generic link cards alongside extension cards. Each card:

- title (localized by `site.default_lang`), description, icon.
- A **plain `<a href="{path}/">`** (NOT `react-router`'s `<Link>`), so the browser performs a full page load to the standalone mount HTML. The existing extension cards use `<Link to>` for in-app routing; mounts deliberately bypass that.
- The href is **relative**, resolved against the SPA `<base href="{deployment_base}">` (e.g. `/blog/portfolio/`). The SPA need not know the deployment base explicitly.
- `open_in_new_tab` → `target="_blank" rel="noopener"`.

Rendered in both the grid and list lobby modes (`web/src/lobby/Lobby.tsx`), mirroring `LobbyCard` / `LobbyRow` markup but as anchors.

## Error handling

- **Config load:** structural validation (§2). Missing source → warn (non-fatal).
- **Build:** missing source → `Err` with context (mount id + path).
- **Manifest:** always reflects configured mounts; never panics on an empty list.

## Testing

Follow the `ssg_build.rs` / `build_writer_tags.rs` patterns:

- **`config.rs` tests:** parse `[[mounts]]`; reject reserved prefixes; reject duplicate `id`/`path`; `resolve_mount_sources` yields absolute paths.
- **`build_writer` test:** temp source dir with `index.html` + a nested asset; set `inputs.mounts`; assert `out/{path}/index.html` + the asset are present and that core `out/index.html` / `out/assets/` are untouched.
- **`manifest` test:** a `Config` with mounts → `assemble()` output contains the matching `ManifestMount` entries.

## Out of scope (v1)

- Multi-site per-mount scoping (v1 mounts are global to the default site).
- Build orchestration — running external build tools (`astro build`, etc.); copy-only.
- Ingest / snapshot mode (live reference only).
- Rewriting absolute paths inside source HTML; relative paths are required (documented). Absolute links like `/styles.css` break under a project deployment base (`/blog/`).
- Live-console mount serving (deemed unnecessary — see "Lobby serving boundary").
- Approach B managed surface (DB + `oxibuilder mount` CLI + console UI).

## Open questions for spec review

- Confirm the default: config-file-dir as the source resolution base.
- Is the reserved-prefix list complete? Proposed: `assets`, `data`, `media`, `api`, `search`, `s`, `admin`, `lobby`, `theme`.

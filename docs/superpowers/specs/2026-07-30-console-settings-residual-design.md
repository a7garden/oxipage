# Console Settings Residual — Design Spec

> **Date:** 2026-07-30
> **Sub-project:** 4 of the decomposed "remaining console work" (Phase 10 remainder).
> **Scope:** fix the `set_default` stub; decision on "Purge All Data".
> **Predecessor:** `2026-2026-30-console-data-foundation-design.md` (S1).

## 1. Goal

Close the two Settings items left dangling after S1: the `set_default` handler is a no-op stub, and the Danger Zone "Purge All Data" button is disabled with no decision. S4 fixes `set_default` and explicitly defers "Purge All Data".

## 2. Scope

### In scope
- **`set_default` stub fix:** `PUT /api/console/sites/default` actually persists the chosen default site to `sites.toml` + in-memory state.
- **Client wiring:** connect the existing `setDefaultSite` to a UI affordance (SitesPage row action / Settings).

### Out of scope (deferred)
- **"Purge All Data":** stays `disabled`. Rationale: truncating every content table is destructive and low-value; a safe version (snapshot-first via the existing `POST /backup/snapshot`, then truncate, with typed-slug confirmation) is a worthwhile but separate destructive-ops sub-project. Deferring keeps S4 at its documented "난이도 하".

## 3. Current State (grounding)

| Concern | Current state | File |
|---------|--------------|------|
| `set_default` handler | stub — `async fn set_default() -> Json({"data":{"ok":true}})`, **takes no body, persists nothing** | `router.rs:94-96` |
| Route | `PUT /sites/default` → stub | `router.rs:32` |
| `SitesFile::set_default` | **exists** — already used by `register_in_file` | `sites_runtime.rs:166` |
| sites.toml write pattern | established by `remove_site` (read → modify → `toml::to_string_pretty` → `fs::write` → sync in-memory) | `sites_runtime.rs:174-204` |
| `default_slug()` / `get_default` | read `default_site`; `all_sites` flags the active one | `sites_runtime.rs:80-130` |
| Client `setDefaultSite` | exists, fires `PUT /sites/default` | `api.ts:42-48` |
| `backup_snapshot` endpoint | exists (future purge safety net) | `oxipage-core/src/http.rs:63` |

## 4. Design

### 4.1 `set_default` fix

- Change the signature to accept a body:
```rust
#[derive(Deserialize)]
struct SetDefaultInput { default_site: String }

async fn set_default(
    State(registry): State<Arc<SiteRegistry>>,
    Json(input): Json<SetDefaultInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)>
```
- Add `SiteRegistry::set_default(&self, slug: &str) -> anyhow::Result<()>` mirroring `remove_site` (lines 174-204):
  1. Validate the slug is registered (`sites.read().await.contains_key(slug)`) → else `404 unknown_site`.
  2. Read `sites.toml` from `ProjectDirs config_dir`, parse `SitesFile`.
  3. `sf.set_default(slug)`.
  4. `toml::to_string_pretty(&sf)` → `fs::write` (create parent dir if missing).
  5. Sync in-memory: `*self.sites_file.write().await = sf`.
- Return `{ "data": { "default_site": "<slug>" } }`.

### 4.2 Client wiring

- `setDefaultSite` (api.ts) already exists and matches the new contract. No API change beyond the response shape it already expects.
- Add a "Set as default" action on the SitesPage (per-row) and/or a "Default site" selector in Settings, both calling `setDefaultSite` and invalidating `["sites"]` + the per-site `["site",slug,"config"]` if it surfaces default state.

### 4.3 Purge All Data (deferred)

- Keep the button `disabled`. Change its label/help to indicate it is planned, not broken. No handler added.
- Document the deferred plan (for a future sub-project): `POST /backup/snapshot` → confirm by typing the site slug → `DELETE FROM <enabled-ext content tables>` → confirmation. Out of scope here.

## 5. Constraints

- `set_default` validates the slug is a *registered* site (not just present in the file) before persisting.
- The sites.toml write reuses the exact `remove_site` pattern (no new I/O convention).
- Unknown slug → `404`, not a silent success.

## 6. Testing

- `PUT /sites/default {default_site:"selfhost"}` round-trips: `default_slug()` reflects it; `sites.toml` on disk contains the new `default_site`; `all_sites` flags the right one active.
- Unknown slug → 404.
- A previously-set default can be changed to another registered site.

## 7. File map

```
crates/oxipage-console/src/
├── router.rs            # set_default: accept body, delegate to registry, 404 unknown
└── sites_runtime.rs     # +SiteRegistry::set_default(slug) — mirror remove_site write path

web/src/admin/
├── shared/api.ts        # setDefaultSite (exists; confirm response shape)
├── sites/SitesPage.tsx  # + "Set as default" row action
└── settings/SettingsPage.tsx  # Purge button stays disabled + relabeled
```

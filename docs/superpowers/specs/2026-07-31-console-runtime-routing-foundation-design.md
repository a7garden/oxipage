# Console Runtime and Routing Foundation — Design Spec

> **Date:** 2026-07-31
> **Subproject:** 1 of 5
> **Predecessor:** none
> **Successor contracts:** Admin theme, preview/media, GitHub Pages

## 1. Goal

Make Admin SPA boot and all site filesystem paths deterministic. Direct navigation to `/sites` and every Admin deep link must serve the current `admin.html`, load matching hashed chunks, and either render React or show an actionable runtime error. Build, preview, upload, and deploy must share one absolute site path model.

## 2. Current state and diagnosis

The current source path for `/sites` is internally correct:

```text
GET /sites
→ oxibuilder-core build_app outer fallback
→ static_handler exact asset lookup misses "sites"
→ serve embedded admin.html
→ load /assets/admin-<hash>.js and global CSS
→ AdminApp BrowserRouter, no basename
→ <Route path="sites">
```

Evidence:

- `crates/oxibuilder-core/src/http.rs::Assets` embeds `crates/oxibuilder-core/embedded-spa`.
- `static_handler` serves an exact embedded file or falls back to `admin.html`.
- `web/src/admin/App.tsx` declares `path="sites"` under a root-hosted `BrowserRouter`.
- Admin API clients use absolute `/api/console/...` paths, so deep-link depth is irrelevant.
- Current `web/dist/admin.html` references assets that exist in the served core embed.

The source therefore does not justify a `/sites` server route or an Admin router basename. Likely runtime causes are stale embedded assets, a binary built without `admin.html`, a cached HTML document referencing deleted chunks, or an unhandled JS/lazy-chunk failure.

A major source of confusion is the duplicate embed:

- **served:** `crates/oxibuilder-core/embedded-spa`
- **unused:** `crates/oxibuilder-console/embedded-spa`, populated by `crates/oxibuilder-console/build.rs`

Nothing in `oxibuilder-console/src` embeds or reads the latter.

## 3. Scope

### In scope

- Reproduce `/sites` from a freshly built frontend and Rust binary before changing routing.
- Consolidate Admin embed ownership in `oxibuilder-core`.
- Fail the build when the production Admin entry is absent.
- Add cache and SPA revision policy.
- Add an Admin ErrorBoundary and stale-chunk recovery UI.
- Resolve `project_dir`, `data_dir`, `out_dir`, and `media_dir` once in `SiteContext`.
- Remove legacy top-level build/deploy route duplicates after site-scoped callers are confirmed.
- Remove the dead favicon reference or ship the referenced favicon.

### Out of scope

- Hosting the Admin SPA under a reverse-proxy subpath.
- Changing the Admin router to a data router.
- Changing the `oxibuilder open` root URL behavior.
- Public static-site base-path behavior beyond defining the `SiteContext` and build-manifest contracts consumed by later specs.

## 4. Embed and build design

### 4.1 Single embed

Keep only:

```text
crates/oxibuilder-core/embedded-spa/
```

Remove:

```text
crates/oxibuilder-console/embedded-spa/
crates/oxibuilder-console/build.rs
```

The current console build script has no other responsibility.

### 4.2 Development and packaged-crate inputs

The build script chooses one of two explicit input modes:

1. **Workspace development:** when `../../web/dist` and `../../web/dist-static` exist, require:
   ```text
   web/dist/admin.html
   web/dist/index.html
   web/dist-static/index.html
   ```
   Missing or partial workspace output fails with:
   ```text
   cd web && bun run build && bun run build:static
   ```
2. **Published crate build:** when workspace `web/` is absent, use the packaged, already-populated `embedded-spa/` and `embedded-spa-static/`; require their `admin.html`/`index.html` entries and fail if the crate package is incomplete.

The build script never writes an index-only placeholder. `oxibuilder-core/Cargo.toml` includes both `embedded-spa/**` and `embedded-spa-static/**` so `cargo install oxibuilder` works without the web source tree.

### 4.3 Revision marker

The core build script computes a deterministic SHA-256 over relative filenames plus bytes in `web/dist` and writes:

```text
embedded-spa/.build-revision
```

The same revision is compiled into the server. HTML responses expose:

```http
X-Oxibuilder-SPA-Revision: <sha256>
```

The ErrorBoundary displays this revision. It is diagnostic metadata, not a cache key or security token.

## 5. Static response policy

`serve_asset` classifies responses:

|Response|Cache-Control|
|---|---|
|`admin.html`, `index.html`, SPA fallback HTML|`no-cache`|
|hashed `/assets/*` files|`public, max-age=31536000, immutable`|
|non-hashed icons/manifest|`no-cache`|

HTML also receives an ETag derived from embedded bytes. Exact assets retain MIME detection. `HEAD` must return the same status and headers without a body.

The fallback remains:

```text
exact embedded file → that file
otherwise           → admin.html
```

API paths remain outside this fallback. A missing `admin.html` is treated as a server build defect and returns a structured 500 response with the compiled SPA revision, not a silent 404.

## 6. Admin runtime resilience

A root `AdminErrorBoundary` surrounds the routed Admin content and handles:

- render exceptions,
- lazy-import rejection,
- stale chunk `TypeError`/module fetch failures.

Its recovery view contains:

- concise failure category,
- SPA revision,
- `Reload console` button using `window.location.reload()`,
- `Clear cached console and reload` action that unregisters service workers if any exist, clears Cache Storage for the origin, then reloads.

It does not suppress errors or route around a broken component. Development builds still log the original error and component stack.

## 7. Site path model

`SiteLoader` canonicalizes and stores:

```rust
pub struct SiteContext {
    pub slug: String,
    pub project_dir: PathBuf,
    pub data_dir: PathBuf,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    pub startup_server: ServerConfig,
    pub settings: Arc<RwLock<MutableSiteSettings>>,
    pub config_write_lock: Arc<Mutex<()>>,
    // existing db/registry/builders/guard fields
}
```

Rules:

```text
project_dir = canonicalize(registered site path)
data_dir    = config.server.data_dir if absolute
              else project_dir.join(config.server.data_dir)
out_dir     = data_dir.join("out")
media_dir   = data_dir.join("media")
```

`startup_server` and all resolved paths are immutable for the process lifetime: changing host/port requires rebinding the listener and changing data_dir requires reopening the DB and recomputing output/media paths. They are excluded from `ConfigUpdate` and shown as read-only values in Settings.

`MutableSiteSettings` contains only live-reloadable site display/language fields, lobby settings, integrations, and deploy target. `config_put` takes `config_write_lock`, rereads the current TOML, applies an allowlisted patch to those mutable sections, preserves server/unknown sections, validates the complete document, writes a same-directory temporary file plus atomic rename, then replaces only `settings`. Atomic rename prevents torn writes; the mutex prevents two console mutations from losing each other. Build and deploy clone one settings snapshot at operation start.

The DB is `data_dir/oxibuilder.db`. Build output is always `out_dir`. Media is always `media_dir`. Handlers do not recompute any of these from `ctx.path`, config strings, or CWD.

The field `path` is renamed to `project_dir` and all internal callers migrate in the same cutover; no alias remains.

## 8. Route cleanup

The canonical operation routes are:

```text
POST /api/console/s/{slug}/build
GET  /api/console/s/{slug}/build/{run_id}/stream
POST /api/console/s/{slug}/deploy
GET  /api/console/s/{slug}/deploy/{run_id}/stream
```

Remove legacy duplicates:

```text
POST /api/console/build/{slug}
POST /api/console/deploy/{slug}
crates/oxibuilder-console/src/build/site_build.rs
crates/oxibuilder-console/src/deploy/site_deploy.rs
```

Preview remains top-level site-addressed because it serves a public static origin simulation:

```text
GET /api/console/preview/{slug}/*
```

Its behavior is completed in subproject 3.

## 9. File map

```text
crates/oxibuilder-core/
├── Cargo.toml                      # package both live + static embeds
├── build.rs                        # workspace/package modes, one live embed, revision
├── embedded-spa/                   # sole live Admin embed
├── embedded-spa-static/            # packaged public static embed
└── src/http.rs                     # cache headers, ETag, revision

crates/oxibuilder-console/
├── build.rs                         # remove
├── embedded-spa/                    # remove
└── src/
    ├── loader.rs                    # resolve paths + reloadable config snapshot
    ├── sites_runtime.rs             # explicit path/config fields
    ├── router.rs                    # remove legacy operation routes
    ├── build/site_build.rs          # remove
    └── deploy/site_deploy.rs        # remove

web/
├── admin.html                       # real favicon; theme boot retained
└── src/admin/
    ├── App.tsx                      # AdminErrorBoundary mount
    └── shared/ui/AdminErrorBoundary.tsx
```

## 10. Verification

### Reproduction before change

1. Build `web/dist` and `web/dist-static`.
2. Build the Rust binary.
3. Start the console.
4. Verify `/sites` returns 200 HTML with the current admin chunk.
5. Verify the referenced JS/CSS return 200.
6. Verify `/api/console/sites` returns JSON.
7. Inspect browser Console and Network before assigning a code root cause.

### Post-change acceptance

- `/sites`, `/sites/new`, `/setup`, and every `/s/{slug}/...` page direct-load and survive refresh.
- HTML is `no-cache`; hashed chunks are immutable; HTML exposes the compiled revision.
- Building without `web/dist/admin.html` fails instead of producing a partial binary.
- Only the core embed exists and changes to it are reflected in the built binary.
- A forced missing lazy chunk renders the recovery view instead of a blank screen.
- With console CWD outside the site repository, `SiteContext` still points to the correct DB/out/media directories.
- No caller uses the removed top-level build/deploy routes.

# Site-Picker Unified Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace v1's split :8787/:8788 console shape and inactive SiteSwitcher with one :8787 management console that owns all registered oxipage project directories. Each site = its own oxipage.toml + oxipage.db; the backend mounts `/s/<slug>/<ext>` for every site at startup and resolves per-request DB pool through middleware.

**Architecture:** SiteRegistry (startup-loaded, no swap), per-site `SiteScopedDb` injected via middleware into `Request::extensions()`, admin-web SPA absorbed into `web/src/admin/` so the wizard, the sites picker, and the per-site console share one shell. Setup state moves from per-site DB to `~/.config/oxipage/console.db`. Public preview is `out/<slug>/` served at `/preview/:slug/*`.

**Tech Stack:** Rust 1.96+, axum 0.8, sqlx (SqlitePool), serde, walkdir, directories; React 19 + Vite 7 + TanStack Query 5 (consumed by SPA, no new dependency); all extensions modified minimally via one-line state.db → SiteScopedDb swap.

**Spec:** `docs/superpowers/specs/2026-07-30-site-picker-console-design.md`

## Global Constraints

- Workspace remains: core / console / cli / wasm-demo / 9 extensions + starter.
- `cargo test --workspace` must pass after each task.
- `cargo clippy --workspace --all-targets -- -D warnings` clean after each task.
- `cd web && bun run build` clean after each task.
- All Korean CLI output messages unchanged unless directly modified by a task.
- Never delete `proxy.rs`-style behavior: it was already removed in v2, but no code added in this plan may reintroduce remote HTTP proxying.
- All new HTTP routes must enforce `:8787` loopback binding; reject `0.0.0.0` with a warning + bind failure (assertion in test).
- Token/session handling: do not introduce remote endpoints without authentication. Bearer tokens stay out of this plan.

---

## File Structure

### New files
- `crates/oxipage-console/src/sites_runtime.rs` — SiteRegistry + SiteLoader
- `crates/oxipage-console/src/console_state.rs` — console.db connection + setup_state migrations
- `crates/oxipage-console/src/router.rs` — site-prefixed route builder
- `crates/oxipage-console/src/middleware/site_db.rs` — db_for_middleware
- `crates/oxipage-console/src/middleware/setup_gate.rs` — (moved from core, opt-in re-export)
- `crates/oxipage-console/tests/sites_registry.rs`
- `crates/oxipage-console/tests/site_routes.rs`
- `crates/oxipage-console/tests/setup_site_create.rs`
- `crates/oxipage-console/tests/console_state.rs`
- `crates/oxipage-cli/src/commands/console.rs` — (refactored; replaces init_console)
- `crates/oxipage-cli/src/commands/site.rs` — path-based variant
- `web/src/admin/shell/SiteShell.tsx`
- `web/src/admin/shell/SiteSwitcher.tsx`
- `web/src/admin/shell/TopBar.tsx`
- `web/src/admin/shell/Sidebar.tsx`
- `web/src/admin/sites/SitesPage.tsx`
- `web/src/admin/sites/NewSiteWizardPage.tsx`
- `web/src/admin/sites/HomeRedirect.tsx`
- `web/src/admin/sites/PreviewFrame.tsx`
- `web/src/admin/main.tsx`
- `web/src/admin/App.tsx`
- `web/src/admin/shared/api.ts`
- `web/src/admin/shared/SiteContext.tsx`
- `web/src/admin/dashboard/DashboardPage.tsx`
- `web/src/admin/content/BlogListPage.tsx`
- `web/src/admin/content/BlogEditorPage.tsx`
- `web/src/admin/extensions/ExtensionsPage.tsx`
- `web/src/admin/themes/ThemesPage.tsx`
- `web/src/admin/build/BuildPage.tsx`
- `web/src/admin/deploy/DeployPage.tsx`
- `crates/oxipage-core/tests/per_site_handler_smoke.rs` (one-line swap integration)

### Modified files
- `Cargo.toml` (workspace deps)
- `crates/oxipage-console/Cargo.toml`
- `crates/oxipage-console/src/lib.rs` (entry; remove run_admin)
- `crates/oxipage-console/src/admin/mod.rs` (delete)
- `crates/oxipage-core/src/extension.rs` (no trait change; sites_runtime depends on existing trait surface)
- `crates/oxipage-core/src/http.rs::build_app` (move into oxipage-console/src/router.rs)
- `crates/oxipage-core/src/setup.rs` (read setup_state from console.db, not site DB)
- `crates/oxipage-cli/src/commands/mod.rs`
- `crates/oxipage-cli/src/commands/init_console.rs` → renamed to `console.rs`
- `crates/oxipage-cli/src/sites.rs` (SiteEntry renamed: `path: PathBuf`)
- `crates/oxipage-cli/src/main.rs`
- `crates/oxipage-ext-blog/src/http.rs` (state.db → SiteScopedDb)
- `web/src/setup/SetupWizard.tsx` (StepSite adds site-directory choice)
- `web/src/setup/api.ts` (create-site added)
- `web/src/App.tsx` (remove the public-shell route — it now lives in admin shell)

### Deleted files
- `crates/oxipage-console/src/admin/` directory
- `admin-web/` directory (only after web/src/admin/ is wired up)

---

## Task 1: sites.toml schema migration + SiteRegistry skeleton

**Files:**
- Modify: `crates/oxipage-cli/src/sites.rs`
- Create: `crates/oxipage-console/src/sites_runtime.rs`
- Create: `crates/oxipage-console/tests/sites_registry.rs`

**Interfaces:**
- `SitesFile::add(slug, path)`, `SitesFile::remove(slug)`, `SitesFile::set_default(slug)` (already exist for CRUD — adapt to new schema)
- `SiteRegistry::new(sites_file: SitesFile) -> Self` (loads all valid sites synchronously at startup)
- `SiteRegistry::db_for(slug: &str) -> Option<SqlitePool>`
- `SiteRegistry::ctx_for(slug: &str) -> Option<Arc<SiteContext>>`

- [ ] **Step 1: Write the failing test for SitesFile new schema**

```rust
// crates/oxipage-cli/src/sites.rs (test module, adjust if already exists)
#[test]
fn site_entry_round_trips_path_only() {
    let sf = SitesFile {
        default_site: Some("blog".into()),
        sites: BTreeMap::from([(
            "blog".into(),
            SiteEntry { path: PathBuf::from("/tmp/oxipage/test-blog") },
        )]),
    };
    let raw = toml::to_string(&sf).unwrap();
    assert!(raw.contains("path"));
    assert!(!raw.contains("endpoint"));
    let back: SitesFile = toml::from_str(&raw).unwrap();
    assert_eq!(back.sites["blog"].path, PathBuf::from("/tmp/oxipage/test-blog"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-cli --lib site_entry_round_trips_path_only -v`
Expected: FAIL — `endpoint`/`token` fields still in struct.

- [ ] **Step 3: Edit SiteEntry schema**

```rust
// crates/oxipage-cli/src/sites.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    pub path: PathBuf,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxipage-cli --lib site_entry_round_trips_path_only -v`
Expected: PASS.

- [ ] **Step 5: Write failing test for SiteRegistry load-and-lookup**

```rust
// crates/oxipage-console/tests/sites_registry.rs
#[tokio::test]
async fn registry_loads_each_valid_site_and_lookups_db() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    std::fs::write(dir_a.path().join("oxipage.toml"), "[site]\nname = \"A\"\nbase_url = \"http://a\"\n").unwrap();
    std::fs::write(dir_b.path().join("oxipage.toml"), "[site]\nname = \"B\"\nbase_url = \"http://b\"\n").unwrap();

    let mut sf = SitesFile::default();
    sf.add("a", dir_a.path().to_path_buf()).unwrap();
    sf.add("b", dir_b.path().to_path_buf()).unwrap();
    sf.set_default("a");

    let reg = SiteRegistry::new(sf).await.unwrap();
    assert!(reg.db_for("a").is_some());
    assert!(reg.db_for("b").is_some());
    assert!(reg.db_for("missing").is_none());
    assert_eq!(reg.default_slug(), Some("a".into()));
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test sites_registry registry_loads_each_valid_site_and_lookups_db -v`
Expected: FAIL — `oxipage-console` has no `SiteRegistry` yet.

- [ ] **Step 7: Define SiteContext and SiteRegistry**

```rust
// crates/oxipage-console/src/sites_runtime.rs
use crate::loader::SiteLoader;
use oxipage_cli::sites::{SitesFile, SiteEntry};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SiteContext {
    pub slug: String,
    pub path: PathBuf,
    pub config: Arc<oxipage_core::config::Config>,
    pub db: SqlitePool,
    pub registry: Arc<oxipage_core::registry::ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn oxipage_core::builder::BuildExt>>>,
    pub wasm_loader: Option<Arc<dyn oxipage_core::extension::WasmLoader>>,
}

pub struct SiteRegistry {
    sites: RwLock<HashMap<String, Arc<SiteContext>>>,
    sites_file: RwLock<SitesFile>,
}

impl SiteRegistry {
    pub async fn new(sites_file: SitesFile) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        for (slug, entry) in &sites_file.sites {
            if !entry.path.exists() {
                tracing::warn!(slug, path = %entry.path.display(), "site path missing; skipping");
                continue;
            }
            let ctx = SiteLoader::load(slug.clone(), entry.path.clone()).await?;
            map.insert(slug.clone(), Arc::new(ctx));
        }
        Ok(Self {
            sites: RwLock::new(map),
            sites_file: RwLock::new(sites_file),
        })
    }

    pub async fn db_for(&self, slug: &str) -> Option<SqlitePool> {
        self.sites.read().await.get(slug).map(|c| c.db.clone())
    }
    pub async fn ctx_for(&self, slug: &str) -> Option<Arc<SiteContext>> {
        self.sites.read().await.get(slug).cloned()
    }
    pub async fn default_slug(&self) -> Option<String> {
        let sf = self.sites_file.read().await;
        sf.default_site.clone().or_else(|| sf.sites.keys().next().cloned())
    }
}
```

- [ ] **Step 8: Implement SiteLoader**

```rust
// crates/oxipage-console/src/loader.rs
use crate::sites_runtime::SiteContext;
use oxipage_core::builder::BuildExt;
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SiteLoader;

impl SiteLoader {
    pub async fn load(slug: String, path: PathBuf) -> anyhow::Result<SiteContext> {
        let cfg = Config::load(&path.join("oxipage.toml"))?;
        let db_path = cfg.server.data_dir.join("oxipage.db");
        let db = oxipage_core::db::connect(&db_path).await?;
        let toml_enabled = cfg.extensions.enabled.clone();
        let extensions = oxipage_console::all_extensions();
        let registry = Arc::new(ExtensionRegistry::new(extensions));
        registry.run_migrations(&db, &toml_enabled).await?;
        let wasm_loader: Option<Arc<dyn oxipage_core::extension::WasmLoader>> = None;
        Ok(SiteContext {
            slug,
            path,
            config: Arc::new(cfg),
            db,
            registry,
            builders: Arc::new(oxipage_console::all_builders()),
            wasm_loader,
        })
    }
}
```

(Note: `all_extensions` and `all_builders` are moved out of `oxipage-console::lib` to a shared module so loader can call them — see also T3.)

- [ ] **Step 9: Run test to verify it passes**

Run: `cargo test -p oxipage-console --test sites_registry -v`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(console): SiteRegistry skeleton + SiteLoader (sites.toml path schema)"
```

---

## Task 2: console.db + setup_state migration

**Files:**
- Create: `crates/oxipage-console/src/console_state.rs`
- Create: `crates/oxipage-console/tests/console_state.rs`
- Modify: `crates/oxipage-core/src/setup.rs` (read setup_state from console.db)
- Modify: `crates/oxipage-console/src/lib.rs` (wire console.db connection at startup)

**Interfaces:**
- `ConsoleState::open(data_dir: &Path) -> Result<Self>` — opens or creates `console.db`, runs migrations
- `console_db_path(global_config_dir: &Path) -> PathBuf`
- `is_setup_needed_v2(db: &SqlitePool) -> bool` (replaces the per-site-DB variant internally)

- [ ] **Step 1: Write failing test for console.db migrations**

```rust
// crates/oxipage-console/tests/console_state.rs
use oxipage_console::console_state::ConsoleState;

#[tokio::test]
async fn console_state_migrates_setup_state_table() {
    let tmp = tempdir().unwrap();
    let state = ConsoleState::open(tmp.path()).await.unwrap();
    let conn = state.pool().acquire().await.unwrap();
    let row: (i64, Option<String>) = sqlx::query_as("SELECT id, setup_completed_at FROM setup_state")
        .fetch_one(conn).await.unwrap();
    assert_eq!(row.0, 1);
    assert!(row.1.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test console_state -v`
Expected: FAIL — module missing.

- [ ] **Step 3: Implement ConsoleState**

```rust
// crates/oxipage-console/src/console_state.rs
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::path::Path;

pub struct ConsoleState {
    pool: SqlitePool,
}

impl ConsoleState {
    pub async fn open(data_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("console.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS setup_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                setup_completed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );",
        ).execute(&pool).await?;
        sqlx::query("INSERT OR IGNORE INTO setup_state (id) VALUES (1);")
            .execute(&pool).await?;
        Ok(Self { pool })
    }
    pub fn pool(&self) -> &SqlitePool { &self.pool }
    pub async fn is_setup_needed(&self) -> bool {
        let row: (Option<String>,) = sqlx::query_as(
            "SELECT setup_completed_at FROM setup_state WHERE id = 1",
        ).fetch_one(&self.pool).await.unwrap_or((None,));
        row.0.is_none()
    }
    pub async fn mark_setup_complete(&self) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE setup_state SET setup_completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
        ).execute(&self.pool).await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxipage-console --test console_state -v`
Expected: PASS.

- [ ] **Step 5: Switch setup.rs to read from console.db**

In `crates/oxipage-core/src/setup.rs`:
- Replace `is_setup_needed(db: &SqlitePool)` to take `ConsoleState`-like source instead.
- During lib refactor (T2 includes this), mark the function `#[deprecated]` and route callers through the new one inside `oxipage-console`.

```rust
#[deprecated(note = "Use oxipage_console::console_state::ConsoleState::is_setup_needed")]
pub async fn is_setup_needed(db: &sqlx::SqlitePool) -> bool {
    let row: (Option<String>,) = sqlx::query_as(
        "SELECT setup_completed_at FROM setup_state WHERE id = 1",
    ).fetch_one(db).await.unwrap_or((None,));
    row.0.is_none()
}
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(console): console.db with setup_state table — replaces per-site setup_state lookups"
```

---

## Task 3: Site-prefixed route builder + SiteScopedDb middleware

**Files:**
- Create: `crates/oxipage-console/src/router.rs`
- Create: `crates/oxipage-console/src/middleware/mod.rs`
- Create: `crates/oxipage-console/src/middleware/site_db.rs`
- Create: `crates/oxipage-console/tests/site_routes.rs`
- Modify: `crates/oxipage-console/src/lib.rs`

**Interfaces:**
- `pub fn build_console_router(state: AppState) -> Router`
- `pub struct SiteScopedDb { pub db: SqlitePool }`
- `async fn db_for_middleware(...) -> Result<Response, ApiError>`

- [ ] **Step 1: Write failing test for site-prefixed routes**

```rust
// crates/oxipage-console/tests/site_routes.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn unknown_slug_returns_404() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/api/console/s/missing/blog/posts").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test site_routes -v`
Expected: FAIL — `build_test_app` helper missing.

- [ ] **Step 3: Implement SiteScopedDb middleware**

```rust
// crates/oxipage-console/src/middleware/site_db.rs
use crate::console_state::AppState;
use crate::sites_runtime::SiteScopedDb;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

pub async fn db_for_middleware(
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ctx = state.sites.ctx_for(&slug).await.ok_or(StatusCode::NOT_FOUND)?;
    req.extensions_mut().insert(SiteScopedDb { db: ctx.db.clone() });
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}
```

- [ ] **Step 4: Implement build_console_router**

```rust
// crates/oxipage-console/src/router.rs
use crate::console_state::AppState;
use crate::middleware::site_db::db_for_middleware;
use crate::sites_runtime::SiteScopedDb;
use axum::extract::Extension;
use axum::routing::{get, post, put, delete};
use axum::Router;

pub fn build_console_router(state: AppState) -> Router {
    let mut api = Router::new();
    api = api
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/:slug", put(update_site).delete(remove_site))
        .route("/sites/default", get(get_default).put(set_default))
        .route("/setup/*", crate::setup::router());

    for (slug, _ctx) in state.sites.iter_blocking() {
        let mut nested = Router::new()
            .route("/build", post(crate::build::site_build_handler))
            .route("/deploy", post(crate::deploy::site_deploy_handler));
        for ext in state.all_extensions() {
            if ext.route_dispatcher().is_some() { continue; }
            nested = nested.nest(&format!("/{}", ext.id()), ext.routes());
        }
        api = api.nest(
            &format!("/s/{slug}"),
            nested.layer(axum::middleware::from_fn_with_state(
                state.clone(),
                db_for_middleware,
            )),
        );
    }
    api.with_state(state)
}
```

(`state.sites.iter_blocking()` is a sync iterator over the loaded map, available at startup before the router is constructed. Add this method in T1 alongside the async accessors.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p oxipage-console --test site_routes -v`
Expected: PASS (one assertion only — additional coverage comes from T4).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(console): site-prefixed router + SiteScopedDb middleware"
```



## Task 4: Blog extension — first per-extension handler swap to SiteScopedDb

**Files:**
- Modify: `crates/oxipage-ext-blog/src/http.rs` (and any other handler modules in the extension)
- Create: `crates/oxipage-core/tests/per_site_handler_smoke.rs` (integration)
- Modify: `crates/oxipage-console/tests/site_routes.rs` (add blog post listing assertion)

**Interfaces:**
- All blog handlers: `state: State<AppState>` → `pool: Extension<SiteScopedDb>` (and add the `use oxipage_console::sites_runtime::SiteScopedDb;` import).

- [ ] **Step 1: Write failing integration test**

```rust
// crates/oxipage-console/tests/site_routes.rs
#[tokio::test]
async fn blog_list_returns_under_site_prefix() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/console/s/blog/blog/posts")
                .body(Body::empty()).unwrap(),
        ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test site_routes blog_list_returns_under_site_prefix -v`
Expected: FAIL — blog route still mounted at `/api/console/blog/posts` and reading from `state.db` (which exists only in test app, not the registered site).

- [ ] **Step 3: Bulk-swap blog handlers' `state.db` access**

For every async handler in `crates/oxipage-ext-blog/src/`:
- Change signature `state: State<AppState>` → `Extension(pool): Extension<SiteScopedDb>` (keep `State` if and only if the handler also touches non-db globals — for blog, only `db` is touched).
- Replace `&state.db` → `&pool.db`.

```rust
// before
pub async fn list_posts(State(state): State<AppState>) -> Result<Json<...>, ApiError> {
    let rows = sqlx::query_as::<_, BlogRow>("SELECT ...").fetch_all(&state.db).await?;
    Ok(Json(...))
}

// after
pub async fn list_posts(Extension(pool): Extension<SiteScopedDb>) -> Result<Json<...>, ApiError> {
    let rows = sqlx::query_as::<_, BlogRow>("SELECT ...").fetch_all(&pool.db).await?;
    Ok(Json(...))
}
```

- [ ] **Step 4: Run blog lib tests**

Run: `cargo test -p oxipage-ext-blog --lib -v`
Expected: PASS.

- [ ] **Step 5: Re-run failing site_routes test — now passes**

Run: `cargo test -p oxipage-console --test site_routes blog_list_returns_under_site_prefix -v`
Expected: PASS.

- [ ] **Step 6: Run full workspace check**

Run: `cargo test --workspace`
Expected: 139+ (slightly more) tests passing, 0 failures.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(blog): handlers consume SiteScopedDb; routes mountable under /s/<slug>/blog"
```

---

## Task 5: admin-web → web/src/admin/ migration + shell router rebuild

**Files:**
- Create: `web/src/admin/main.tsx`
- Create: `web/src/admin/App.tsx`
- Create: `web/src/admin/shared/api.ts`
- Create: `web/src/admin/shared/SiteContext.tsx`
- Create: `web/src/admin/shell/SiteShell.tsx`
- Create: `web/src/admin/shell/SiteSwitcher.tsx`
- Create: `web/src/admin/shell/TopBar.tsx`
- Create: `web/src/admin/shell/Sidebar.tsx`
- Create: `web/src/admin/sites/HomeRedirect.tsx`
- Create: `web/src/admin/sites/SitesPage.tsx`
- Create: `web/src/admin/sites/NewSiteWizardPage.tsx`
- Create: `web/src/admin/dashboard/DashboardPage.tsx` (moved from admin-web)
- Create: `web/src/admin/content/BlogListPage.tsx`
- Create: `web/src/admin/content/BlogEditorPage.tsx`
- Create: `web/src/admin/extensions/ExtensionsPage.tsx`
- Create: `web/src/admin/themes/ThemesPage.tsx`
- Create: `web/src/admin/build/BuildPage.tsx`
- Create: `web/src/admin/deploy/DeployPage.tsx`
- Modify: `web/vite.config.ts` (add admin entry; keep main public entry)
- Modify: `crates/oxipage-console/Cargo.toml` (rust-embed target: `web/dist-admin` + `web/dist-static` distinct)
- Modify: `crates/oxipage-console/build.rs` (compile both SPA bundles)

**Interfaces:**
- `web/src/admin/shared/api.ts`: `ADMIN_BASE = "/api/console"`; `siteScopedFetch(slug, path)` adds `Bearer` only if and when remote auth exists.
- `web/src/admin/shared/SiteContext.tsx`: provides current `:slug` from URL via react-router params.

- [ ] **Step 1: Add admin entry to vite.config.ts**

```ts
// web/vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";

export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        admin: resolve(__dirname, "admin.html"),
      },
    },
  },
});
```

- [ ] **Step 2: Create `web/admin.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>Oxipage Console</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/admin/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 3: Write failing shell router test**

```ts
// web/src/admin/__tests__/router.test.tsx (vitest if present; else skip)
import { render } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router";
import { AdminApp } from "../App";

test("s/<slug>/* mounts", () => {
  const { container } = render(
    <MemoryRouter initialEntries={["/s/blog"]}>
      <AdminApp />
    </MemoryRouter>,
  );
  expect(container.textContent).toMatch(/Sites/);
});
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cd web && bun run test`
Expected: FAIL — module missing.

- [ ] **Step 5: Implement SiteShell and SiteSwitcher**

```tsx
// web/src/admin/shell/SiteShell.tsx
import { Outlet, useParams } from "react-router";
import { SiteSwitcher } from "./SiteSwitcher";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function SiteShell() {
  const { slug } = useParams<{ slug?: string }>();
  return (
    <div className="admin-shell">
      <TopBar />
      <div className="admin-body">
        <Sidebar currentSlug={slug ?? null} />
        <main><Outlet /></main>
      </div>
    </div>
  );
}
```

```tsx
// web/src/admin/shell/SiteSwitcher.tsx
import { useQuery } from "@tanstack/react-query";
import { listSites, setDefaultSite } from "../shared/api";
import { Link, useParams } from "react-router";

export function SiteSwitcher() {
  const { slug } = useParams();
  const { data } = useQuery({ queryKey: ["sites"], queryFn: listSites });
  return (
    <div>
      {(data?.data ?? []).map((s) => (
        <Link key={s.name} to={`/s/${s.name}`}>
          {s.name}{slug === s.name ? " ●" : ""}
        </Link>
      ))}
      <Link to="/sites/new">+ 새 사이트</Link>
    </div>
  );
}
```

- [ ] **Step 6: Implement AdminApp router**

```tsx
// web/src/admin/App.tsx
import { BrowserRouter, Routes, Route } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SiteShell } from "./shell/SiteShell";
import { HomeRedirect } from "./sites/HomeRedirect";
import { SitesPage } from "./sites/SitesPage";
import { NewSiteWizardPage } from "./sites/NewSiteWizardPage";
import { DashboardPage } from "./dashboard/DashboardPage";
import { BlogListPage } from "./content/BlogListPage";
import { BlogEditorPage } from "./content/BlogEditorPage";
import { ExtensionsPage } from "./extensions/ExtensionsPage";
import { ThemesPage } from "./themes/ThemesPage";
import { BuildPage } from "./build/BuildPage";
import { DeployPage } from "./deploy/DeployPage";

const queryClient = new QueryClient();

export function AdminApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<SiteShell />}>
            <Route index element={<HomeRedirect />} />
            <Route path="sites" element={<SitesPage />} />
            <Route path="sites/new" element={<NewSiteWizardPage />} />
            <Route path="s/:slug" element={<SiteShell />}>
              <Route index element={<DashboardPage />} />
              <Route path="content/blog" element={<BlogListPage />} />
              <Route path="content/blog/new" element={<BlogEditorPage />} />
              <Route path="content/blog/:postslug" element={<BlogEditorPage />} />
              <Route path="themes" element={<ThemesPage />} />
              <Route path="extensions" element={<ExtensionsPage />} />
              <Route path="build" element={<BuildPage />} />
              <Route path="deploy" element={<DeployPage />} />
            </Route>
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
```

- [ ] **Step 7: Implement HomeRedirect + SitesPage + NewSiteWizardPage + dashboard/content/pages**

Copy and adapt the corresponding `admin-web/src/{sites,dashboard,content,extensions,themes,build,deploy}/*` pages into `web/src/admin/*`. Replace `"/api/admin/..."` calls with `"/api/console/..."` and `useSite().activeSite.name` → use `useParams().slug`. Remove the cog/settings button (D6).

- [ ] **Step 8: Compile both SPA bundles**

Run: `cd web && bun run build`
Expected: `dist/index.html` and `dist-admin/admin.html` written; no tsc errors.

- [ ] **Step 9: Update rust-embed in oxipage-console**

```rust
// crates/oxipage-console/src/admin_bundle.rs
#[derive(RustEmbed)]
#[folder = "../../web/dist-admin/"]
pub struct AdminAssets;

#[derive(RustEmbed)]
#[folder = "../../web/dist-static/"]
pub struct PublicAssets;
```

Add `pub mod admin_bundle;` to `crates/oxipage-console/src/lib.rs`.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(admin): migrate admin-web SPA into web/src/admin and serve from :8787"
```

---

## Task 6: Wizard Step 1 = site directory decision + create-site handler

**Files:**
- Modify: `web/src/setup/SetupWizard.tsx` (StepSite: add directory choice)
- Modify: `web/src/setup/api.ts` (createSite + completeSetup wired)
- Create: `crates/oxipage-console/src/setup/create_site.rs`
- Create: `crates/oxipage-console/src/setup/router.rs` (re-export from core)
- Modify: `crates/oxipage-core/src/setup.rs` (route handler exposes create-site)
- Create: `crates/oxipage-console/tests/setup_site_create.rs`

**Interfaces:**
- `POST /api/console/setup/create-site { path: "~/oxipage/blog" }` → `{ data: { slug, path } }`
- StepSite becomes the wizard's first step (already is, but now also accepts path).

- [ ] **Step 1: Write failing test**

```rust
// crates/oxipage-console/tests/setup_site_create.rs
#[tokio::test]
async fn create_site_handler_seeds_toml_and_registers_in_sites_file() {
    let tmp = tempdir().unwrap();
    let sites_file = tmp.path().join("sites.toml");
    let target = tmp.path().join("blog");
    let app = build_setup_app_with_dir(tmp.path()).await;
    let body = serde_json::json!({ "path": target.to_str().unwrap() });
    let resp = app.oneshot(
        Request::builder().method("POST").uri("/api/console/setup/create-site")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(target.join("oxipage.toml").exists());
    assert!(sites_file.exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test setup_site_create -v`
Expected: FAIL — endpoint missing.

- [ ] **Step 3: Implement create-site handler**

```rust
// crates/oxipage-console/src/setup/create_site.rs
use crate::console_state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use oxipage_cli::sites::{SiteEntry, SitesFile};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct CreateSiteInput {
    pub path: String,
}

pub async fn create_site_handler(
    State(state): State<AppState>,
    Json(input): Json<CreateSiteInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = PathBuf::from(input.path);
    fs::create_dir_all(&path).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    fs::write(
        path.join("oxipage.toml"),
        r#"[site]
name = "New Site"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "./data"
"#,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let slug = path.file_name().and_then(|s| s.to_str()).unwrap_or("site").to_string();
    let mut sf = state.sites_file().clone();
    sf.add(slug.clone(), path.clone()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if sf.default_site.is_none() { sf.set_default(&slug); }
    sf.save(&state.sites_file_path()).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "data": { "slug": slug, "path": path.to_string_lossy() } })))
}
```

(`state.sites_file()` and `state.sites_file_path()` are accessors added to `AppState` as part of T1.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxipage-console --test setup_site_create -v`
Expected: PASS.

- [ ] **Step 5: Update wizard StepSite UI**

```tsx
// web/src/setup/StepSite.tsx (delta)
const [sitePath, setSitePath] = useState("");
async function submit() {
  const result = await fetch("/api/console/setup/create-site", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path: sitePath }),
  }).then((r) => r.json());
  // store result.slug to use in later steps for which extension step data is keyed
  onComplete(result.data.slug);
}
```

- [ ] **Step 6: Update StepDone to redirect to /s/<slug>/**

```tsx
// web/src/setup/StepDone.tsx
useEffect(() => {
  if (!completed) {
    window.location.href = `/s/${slug}/`;
  }
}, [completed, slug]);
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(setup): Step 1 site-directory decision + create-site handler + console redirect"
```



## Task 7: Remove :8788 / run_admin / OXIPAGE_ADMIN_PORT

**Files:**
- Modify: `crates/oxipage-console/src/lib.rs` (delete `pub fn run_admin`, stop embedding `admin_bundle`)
- Modify: `crates/oxipage-console/src/admin/mod.rs` (delete file)
- Modify: `crates/oxipage-cli/src/commands/init_console.rs` (delete `--admin-port` arg)
- Modify: `crates/oxipage-cli/src/commands/mod.rs` (drop admin-bind branch)
- Delete: `admin-web/` directory (post T5)

- [ ] **Step 1: Verify no remaining callers of `run_admin`**

Run: `grep -rn "run_admin\|OXIPAGE_ADMIN_PORT\|admin_bundle" crates/ web/ 2>/dev/null`
Expected: only deletion targets left; if any live reference remains, refactor it to use the new router and update this step's grep.

- [ ] **Step 2: Write smoke test that 8788 binds are gone**

```rust
// crates/oxipage-console/tests/no_8788.rs
#[tokio::test]
async fn run_console_does_not_bind_8788() {
    let cfg = oxipage_core::config::Config::default();
    let bound = try_bind("127.0.0.1:8788").await;
    let _ = bound;
    // After this task, even if nothing is binding 8788 internally, no code path
    // should attempt to do so. Run cargo doc on crate and assert no `0.0.0.0:8788`
    // any longer — implementation: parse src/admin/*.rs (deleted) and src/lib.rs for "8788".
    let src = std::fs::read_to_string("src/lib.rs").unwrap();
    assert!(!src.contains("8788"), "stale 8788 reference: {}", src);
}
```

- [ ] **Step 3: Run test to verify it fails (before deletion)**

Run: `cargo test -p oxipage-console --test no_8788 -v`
Expected: FAIL — `src/lib.rs` still mentions 8788.

- [ ] **Step 4: Delete run_admin and admin/mod.rs**

```bash
git rm crates/oxipage-console/src/admin/mod.rs
git rm crates/oxipage-console/src/admin/sites_api.rs
git rm crates/oxipage-console/src/admin/themes.rs
```

Edit `crates/oxipage-console/src/lib.rs`: remove the `pub mod admin;`, the `pub async fn run_admin(port: u16)`, and the `pub fn run_admin` deprecated alias.

- [ ] **Step 5: Remove OXIPAGE_ADMIN_PORT usage**

Run: `grep -rn OXIPAGE_ADMIN_PORT crates/`
Expected: no matches. If CLI had `env::var("OXIPAGE_ADMIN_PORT")` plumbing, remove those lines.

- [ ] **Step 6: Delete admin-web/ after T5 landed**

Run: `git rm -r admin-web/`

- [ ] **Step 7: Re-run smoke test**

Run: `cargo test -p oxipage-console --test no_8788 -v`
Expected: PASS.

- [ ] **Step 8: Run full verification**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cd web && bun run build`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "chore(console): remove run_admin / :8788 / admin-web directory"
```

---

## Task 8: Remaining extensions — state.db → SiteScopedDb line swap

**Files (one PR per extension, all identical pattern):**
- `crates/oxipage-ext-projects/src/http.rs`
- `crates/oxipage-ext-links/src/http.rs`
- `crates/oxipage-ext-movies/src/http.rs`
- `crates/oxipage-ext-books/src/http.rs`
- `crates/oxipage-ext-scraps/src/http.rs`
- `crates/oxipage-ext-activity/src/http.rs`
- `crates/oxipage-ext-novels/src/http.rs`
- `crates/oxipage-ext-profile/src/http.rs`

**For each extension:**

- [ ] **Step 1: Write a regression test confirming the route returns 200 under `/s/<slug>/<ext>/...`**

```rust
// crates/oxipage-ext-<name>/tests/site_prefixed.rs
#[tokio::test]
async fn list_under_site_prefix_returns_200() {
    let app = build_test_app().await;
    let resp = app.oneshot(
        Request::builder().uri("/api/console/s/<slug>/<ext>/items").body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert!(resp.status().is_success(), "{:?}", resp.status());
}
```

- [ ] **Step 2: Run test, observe failure with the unmodified handler**

Run: `cargo test -p oxipage-ext-<name> --test site_prefixed -v`
Expected: FAIL — handler reads `state.db` which is not populated.

- [ ] **Step 3: Swap handlers' `state.db` → `Extension(SiteScopedDb).db`**

Identical pattern to T4 Step 3 (one-line replacement per handler).

- [ ] **Step 4: Re-run test, observe pass**

Run: `cargo test -p oxipage-ext-<name> --test site_prefixed -v`
Expected: PASS.

- [ ] **Step 5: Run extension lib tests**

Run: `cargo test -p oxipage-ext-<name> --lib -v`
Expected: PASS.

- [ ] **Step 6: One PR per extension**

Commit separately so reviews stay scoped:
- `refactor(projects): SiteScopedDb`
- `refactor(links): SiteScopedDb`
- ...continuing for each remaining extension.

After all eight are merged: run `cargo test --workspace && cargo clippy -- -D warnings` once before T9.

---

## Task 9: Build/deploy triggers + preview route

**Files:**
- Create: `crates/oxipage-console/src/build/site_build.rs`
- Create: `crates/oxipage-console/src/deploy/site_deploy.rs`
- Create: `crates/oxipage-console/src/preview/handler.rs`
- Modify: `crates/oxipage-console/src/router.rs` (wire new endpoints)
- Modify: `web/src/admin/build/BuildPage.tsx`
- Modify: `web/src/admin/deploy/DeployPage.tsx`

- [ ] **Step 1: Write failing test for build trigger**

```rust
// crates/oxipage-console/tests/site_build.rs
#[tokio::test]
async fn site_build_handler_writes_out_dir() {
    let app = build_test_app().await;
    let resp = app.oneshot(
        Request::builder().method("POST").uri("/api/console/s/blog/build").body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(PathBuf::from("./test-blog/out/data").exists());
}
```

- [ ] **Step 2: Run test, observe failure**

Run: `cargo test -p oxipage-console --test site_build -v`
Expected: FAIL — handler missing.

- [ ] **Step 3: Implement site_build_handler**

```rust
// crates/oxipage-console/src/build/site_build.rs
use crate::console_state::AppState;
use axum::extract::{Path, State};
use axum::Json;

pub async fn site_build_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let ctx = state.sites.ctx_for(&slug).await.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let out_dir = ctx.path.join("out");
    std::fs::create_dir_all(&out_dir).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    oxipage_core::build::build_site(&ctx.db, &ctx.builders)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::build::write_static_outputs(&out_dir, &ctx.builders).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "data": { "out_dir": out_dir.to_string_lossy() } })))
}
```

(`build_site` and `write_static_outputs` are existing utilities from the v2 SSG pipeline; if signatures differ, wrap them in a thin adapter.)

- [ ] **Step 4: Implement site_deploy_handler**

Analogous; dispatches `--target github-pages` (existing behavior) using `ctx.path`'s `out/` and `git worktree` flow.

- [ ] **Step 5: Implement /preview/:slug/* static handler**

```rust
// crates/oxipage-console/src/preview/handler.rs
pub async fn preview_handler(
    State(state): State<AppState>,
    Path((slug, rest)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let ctx = state.sites.ctx_for(&slug).await.ok_or(StatusCode::NOT_FOUND)?;
    let file_path = ctx.path.join("out").join(&rest);
    let bytes = std::fs::read(&file_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
    Ok(([(axum::http::header::CONTENT_TYPE, mime.to_string())], bytes).into_response())
}
```

Register in `router.rs`:
```rust
api = api.route("/preview/:slug/*", get(preview_handler));
```

- [ ] **Step 6: Wire BuildPage and DeployPage UIs**

```tsx
// web/src/admin/build/BuildPage.tsx
import { useParams } from "react-router";
export function BuildPage() {
  const { slug } = useParams();
  async function trigger() {
    await fetch(`/api/console/s/${slug}/build`, { method: "POST" });
  }
  return <button onClick={trigger}>Build now</button>;
}
```

Similar for `DeployPage`.

- [ ] **Step 7: Re-run integration test**

Run: `cargo test -p oxipage-console --test site_build -v`
Expected: PASS.

- [ ] **Step 8: Update CLI `--preview` flow**

In `crates/oxipage-cli/src/commands/init_console.rs` (or its renamed successor): when `--preview --site <slug>` is passed, the console still serves :8787 but `/preview/:slug/*` is enabled. Add a flag wiring test:

```rust
// crates/oxipage-cli/tests/preview_flag.rs
#[test]
fn preview_flag_parses_with_site() {
    let parsed = oxipage_cli::parse_preview_args(["--preview", "--site", "blog"]).unwrap();
    assert!(parsed.preview);
    assert_eq!(parsed.site.as_deref(), Some("blog"));
}
```

- [ ] **Step 9: Final workspace check**

Run:
```bash
cargo test --workspace &&
cargo clippy --workspace --all-targets -- -D warnings &&
cd web && bun run build
```
Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(console): site-scoped build/deploy triggers + /preview/:slug/* static route"
```

---

## Cleanup Pass (gated on smoke)

After T1–T9, do not declare "done" until the smoke checklist below passes. Then close scope-drift residue:

- [ ] **Cleanup step 1: README updates**

Edit `README.md`: remove `oxipage admin` references, remove ":8788 admin console" mentions, update "Open http://127.0.0.1:8787" to say "관리 콘솔 셸" (instead of "admin console and API"). Add a "여러 oxipage 사이트 관리" subsection citing `oxipage site add --path`.

- [ ] **Cleanup step 2: doc/09 + doc/12 stub updates**

In `doc/09-multi-site.md`: replace endpoint/token model with path-only schema; add a note that the **console** reads path while CLI can read sites.toml entries identically. In `doc/12-console.md`: append a §12.10 "v2.0 통합 콘솔" note pointing to the new spec.

- [ ] **Cleanup step 3: E2E harness update**

`crates/oxipage-cli/tests/e2e.rs`: update any test referencing `:8788` or `OXIPAGE_ADMIN_PORT`. Add an e2e that calls `oxipage site add blog --path /tmp/blog`, expects OK; then `oxipage console &`, expects `:8787` reachable; then `curl /api/console/s/blog/blog/posts` expects 200.

- [ ] **Cleanup step 4: Smoke checklist**

```bash
cargo build --release --workspace
./target/release/oxipage site add blog --path /tmp/blog
./target/release/oxipage console &
sleep 1
curl -fsS http://127.0.0.1:8787/api/console/s/blog/blog/posts | jq .
./target/release/oxipage build --site blog
./target/release/oxipage deploy --target github-pages --dry-run --site blog
kill %1
```

Expected: all commands exit 0; `curl` returns `{ "data": [...] }`; `build` creates `/tmp/blog/out/`; `deploy --dry-run` exits without pushing.

- [ ] **Cleanup step 5: One final cleanup commit**

```bash
git add -A
git commit -m "docs+tests: README and docs reflect site-picker console; e2e covers blog round-trip"
```

---

## Self-Review Checklist (run by reviewer before merge)

- [ ] **Spec coverage**: every D1–D7 + §0–§14 decision in `docs/superpowers/specs/2026-07-30-site-picker-console-design.md` is implemented by at least one task. Spot-check missing pieces:
  - D2 (single :8787, no :8788) → T7.
  - D3 (admin-web absorbed) → T5.
  - D4 (v2.0 = single instance) → T1's SiteRegistry (no multi-port code).
  - D5 (wizard Step 1 = site dir) → T6.
  - D6 (no cog button) → T5 + Cleanup step 1.
  - D7 (remote sites excluded) → T1's SiteRegistry keyed on local SiteEntry only.
- [ ] **No placeholder**: every step has real code; no "TBD"/"TODO"/"fill in details".
- [ ] **Type consistency**: `SiteScopedDb` defined once (T3), reused across T4/T8. `SiteRegistry::iter_blocking` used only at startup (T3), async `ctx_for` used at request time (T4/T8).
- [ ] **Workspace discipline**: each extension gets its own PR (T8). No single PR mixes blog/projects/links swaps.
- [ ] **Verification gates** green at end: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cd web && bun run build`.
- [ ] **Smoke**: end-to-end covered in Cleanup step 4.
- [ ] **Branch policy**: implementation is on a feature branch, not `main`. Main only carries the spec, this plan, and any fixes; T1 commits land on the feature branch per project memory (`구현은 별도 브랜치로 분기 후 진행`).

---

## Branch Discipline Note (project memory)

Per the standing project memory, all implementation work for this plan must be carried on a **feature branch** that diverges from this commit (`66cb3b8 docs(spec): site-picker unified console — v2 SSG-consistent design`). Suggested branch name: `feat/site-picker-console`. The main-branch commits accepted so far are:

- `docs(spec)` (spec itself)
- `docs(plan)` (this plan, committed separately)

Tasks T1–T9 + cleanup are NOT to land on main.


# Console Runtime and Routing Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/sites` and every Admin deep link deterministic by consolidating the embed, adding cache/error diagnostics, and giving every site one canonical path model.

**Architecture:** The Rust static handler chain is already correct; the real risks are stale binaries, a dead duplicate embed, browser cache, and unhandled JS failures. Fix those plus refactor `SiteContext` to hold resolved absolute paths and a reloadable mutable-settings snapshot, and remove legacy route duplicates.

**Tech Stack:** Rust (axum 0.8, rust-embed, sha2), React 19, TypeScript, Vite 7

## Global Constraints

- Single served embed: `crates/oxipage-core/embedded-spa` only.
- Admin SPA stays root-hosted; no `BrowserRouter` basename.
- `server.host`, `server.port`, `server.data_dir` are startup-immutable — excluded from `ConfigUpdate`.
- All site paths resolve through `SiteContext` fields, never from CWD or config strings at runtime.
- Hashed assets are immutable; HTML is `no-cache`.
- No placeholders, no shims, no aliases after cutover.

---

## File Structure

```text
crates/oxipage-core/
├── Cargo.toml                      # add embedded-spa-static to include
├── build.rs                        # dual-mode validation, revision marker
├── embedded-spa/                   # sole live Admin embed
├── embedded-spa-static/            # packaged public static embed
└── src/
    ├── http.rs                     # cache headers, ETag, revision, HEAD
    ├── lib.rs                      # pub mod site_paths
    ├── site_paths.rs               # NEW: resolved path + settings types
    └── config.rs                   # DeployConfig addition (consumed by subproject 4)

crates/oxipage-console/
├── Cargo.toml                      # remove build dep if any
├── build.rs                        # DELETE
├── embedded-spa/                   # DELETE
└── src/
    ├── loader.rs                   # resolve paths, MutableSiteSettings
    ├── sites_runtime.rs            # new SiteContext fields
    ├── router.rs                   # remove legacy top-level routes
    ├── build/site_build.rs         # DELETE
    ├── deploy/site_deploy.rs       # DELETE
    └── per_site.rs                 # config_put uses config_write_lock

web/
├── admin.html                      # fix favicon ref
└── src/admin/
    ├── App.tsx                     # mount AdminErrorBoundary
    └── shared/ui/AdminErrorBoundary.tsx   # NEW
```

---

### Task 1: Delete dead console embed and build script

**Files:**
- Delete: `crates/oxipage-console/build.rs`
- Delete: `crates/oxipage-console/embedded-spa/` (entire directory)
- Modify: `crates/oxipage-console/Cargo.toml`

**Interfaces:**
- Consumes: nothing
- Produces: a console crate with no embed responsibility; core embed is sole source

- [ ] **Step 1: Verify nothing in console src references the embed**

Run: `grep -r "embedded-spa\|RustEmbed\|Assets::get" crates/oxipage-console/src/`
Expected: no matches (the console crate never embeds; core does)

- [ ] **Step 2: Delete the dead files**

```bash
rm crates/oxipage-console/build.rs
rm -rf crates/oxipage-console/embedded-spa/
```

- [ ] **Step 3: Verify the workspace still builds**

Run: `cargo build --workspace`
Expected: success — the console binary compiles without its build script

- [ ] **Step 4: Verify the console binary still serves admin.html**

Run: `cargo test -p oxipage-console --test site_routes`
Expected: existing tests pass (they use the core embed via `build_console_router`)

- [ ] **Step 5: Commit**

```bash
git add -A crates/oxipage-console/
git commit -m "refactor(console): remove dead duplicate embed and build script"
```

---

### Task 2: Core build.rs — dual-mode validation and revision marker

**Files:**
- Modify: `crates/oxipage-core/build.rs`
- Modify: `crates/oxipage-core/Cargo.toml`

**Interfaces:**
- Consumes: `../../web/dist/`, `../../web/dist-static/` (workspace mode) or packaged embeds (crate mode)
- Produces: `embedded-spa/.build-revision` file; build fails if `admin.html` is missing in workspace mode

- [ ] **Step 1: Add `embedded-spa-static` to the Cargo.toml include list**

In `crates/oxipage-core/Cargo.toml`, change the `include` array:

```toml
include = [
    "src/**",
    "migrations/**",
    "embedded-spa/**",
    "embedded-spa-static/**",
    "_registry.json",
    "_wasm-demo.wasm",
    "build.rs",
    "Cargo.toml",
]
```

- [ ] **Step 2: Rewrite build.rs with dual-mode validation**

Replace the entire contents of `crates/oxipage-core/build.rs`:

```rust
fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=../../web/dist-static");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = std::path::Path::new(&manifest_dir);
    let web_dist = root.join("../../web/dist");
    let web_dist_static = root.join("../../web/dist-static");

    // Mode 1: Workspace development — web/dist exists, require complete output.
    if web_dist.exists() || web_dist_static.exists() {
        validate_and_copy(&web_dist, root.join("embedded-spa"), "admin.html", "web/dist");
        validate_and_copy(
            &web_dist_static,
            root.join("embedded-spa-static"),
            "index.html",
            "web/dist-static",
        );
    } else if root.join("embedded-spa").exists() || root.join("embedded-spa-static").exists() {
        // Mode 2: Published crate — packaged embeds must already be populated.
        require_packaged(root.join("embedded-spa"), "admin.html");
        require_packaged(root.join("embedded-spa-static"), "index.html");
    } else {
        // Fresh development clone: no web build and no packaged embeds yet.
        // Fail with the exact command that produces the required output.
        panic!(
            "no SPA bundle found. Run first: cd web && bun run build && bun run build:static"
        );
    }

    // Registry + WASM (unchanged from existing logic).
    copy_or_stub(root.join("_registry.json"), root.join("../../../registry/index.json"), "[]");
    copy_or_stub(
        root.join("_wasm-demo.wasm"),
        root.join("../../../crates/oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm"),
        b"",
    );
}

fn validate_and_copy(src: &std::path::Path, dst: &std::path::Path, required: &str, label: &str) {
    if !src.exists() {
        // If the sibling dist exists but this one doesn't, that's a partial workspace build.
        panic!(
            "{label} not found at {}. Run: cd web && bun run build && bun run build:static",
            src.display()
        );
    }
    if !src.join(required).exists() {
        panic!(
            "{label}/{} is missing. Run: cd web && bun run build && bun run build:static",
            required,
        );
    }
    if dst.exists() {
        let _ = std::fs::remove_dir_all(dst);
    }
    copy_dir(src, dst).unwrap_or_else(|e| panic!("failed to copy {label} to {}: {e}", dst.display()));
}

fn require_packaged(dir: &std::path::Path, required: &str) {
    if !dir.join(required).exists() {
        panic!(
            "packaged embed at {} is missing {}. The crate package is incomplete.",
            dir.display(),
            required
        );
    }
}

fn copy_or_stub(dst: &std::path::Path, src: &std::path::Path, empty: &[u8]) {
    if src.exists() {
        std::fs::copy(src, dst).unwrap_or_else(|e| panic!("failed to copy {}: {e}", src.display()));
    } else if !dst.exists() {
        std::fs::write(dst, empty).unwrap();
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p oxipage-core`
Expected: success

- [ ] **Step 4: Verify failure when web/dist is missing admin.html**

```bash
# Temporarily rename admin.html to simulate a stale build
mv web/dist/admin.html web/dist/admin.html.bak
cargo build -p oxipage-core 2>&1 | grep "admin.html is missing"
mv web/dist/admin.html.bak web/dist/admin.html
```
Expected: the panic message appears

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-core/build.rs crates/oxipage-core/Cargo.toml
git commit -m "build(core): dual-mode embed validation — fail on missing admin.html"
```

---

### Task 3: Static response policy — cache headers, ETag, revision

**Files:**
- Modify: `crates/oxipage-core/src/http.rs`

**Interfaces:**
- Consumes: `Assets` (rust-embed)
- Produces: `serve_asset` with `Cache-Control`, `ETag`, `X-Oxipage-SPA-Revision` headers; proper `HEAD` support

- [ ] **Step 1: Write a test for cache headers**

Create `crates/oxipage-core/tests/cache_headers.rs`:

```rust
//! Tests for static asset cache policy.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_core::http::build_app;
use oxipage_core::state::AppState;
use tower::util::ServiceExt;

async fn build_test_app() -> axum::Router {
    use oxipage_core::config::Config;
    use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
    use oxipage_core::registry::ExtensionRegistry;

    struct DummyExt;
    #[async_trait::async_trait]
    impl Extension for DummyExt {
        fn id(&self) -> &'static str { "dummy" }
        fn display_name(&self, l: Lang) -> String { "Dummy".into() }
        fn migrations(&self) -> Vec<Migration> {
            vec![Migration { version: 1, name: "init",
                sql: "CREATE TABLE IF NOT EXISTS dummy_t (id INTEGER PRIMARY KEY)" }]
        }
        fn table_names(&self) -> Vec<&'static str> { vec!["dummy_t"] }
        fn routes(&self) -> axum::Router { axum::Router::new() }
        async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> { None }
    }

    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry,
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };
    oxipage_core::http::build_app(state)
}

#[tokio::test]
async fn admin_html_has_no_cache_header() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/sites").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp.headers().get("cache-control").unwrap().to_str().unwrap();
    assert!(cc.contains("no-cache"), "cache-control was: {cc}");
}

#[tokio::test]
async fn hashed_asset_has_immutable_cache() {
    let app = build_test_app().await;
    // Extract the hashed JS asset URI from the embedded admin.html so the
    // test is robust to hash changes across builds.
    let html = oxipage_core::http::spa_index_html().unwrap_or_default();
    let asset = html
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('\"').next())
        .expect("admin.html must reference a script");
    let resp = app
        .oneshot(
            Request::builder()
                .uri(asset)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "asset {asset} not found");
    let cc = resp.headers().get("cache-control").unwrap().to_str().unwrap();
    assert!(cc.contains("immutable"), "cache-control was: {cc}");
}

#[tokio::test]
async fn admin_html_has_revision_meta_and_header() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/sites").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        body.contains("oxipage-spa-revision"),
        "admin.html must carry the revision meta tag"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-core --test cache_headers`
Expected: FAIL — `serve_asset` currently sets no cache headers

- [ ] **Step 3: Implement cache classification in `serve_asset`**

In `crates/oxipage-core/src/http.rs`, replace `serve_asset`:

```rust
fn serve_asset(path: &str) -> Option<Response> {
    Assets::get(path).map(|content| {
        let mut bytes = content.data.into_owned();

        // Expose the compiled SPA revision to the browser on the console entry
        // HTML (both the exact match and the fallback path). The ErrorBoundary
        // reads this meta tag; the header is for debugging.
        if path == "admin.html" {
            let meta = format!(
                "<meta name=\"oxipage-spa-revision\" content=\"{}\">",
                spa_revision()
            );
            let html = String::from_utf8_lossy(&bytes);
            bytes = html.replace("</head>", &format!("{meta}</head>")).into_bytes();
        }

        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache_control = cache_policy_for(path);
        let etag = format!("\"{:x}\"", content_hash(&bytes));

        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::ETAG, etag);

        if is_html_entry(path) {
            builder = builder.header("X-Oxipage-SPA-Revision", spa_revision());
        }
        builder.body(Body::from(bytes)).unwrap()
    })
}

fn cache_policy_for(path: &str) -> &'static str {
    if is_html_entry(path) {
        "no-cache"
    } else if path.starts_with("assets/") && has_hash_suffix(path) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn is_html_entry(path: &str) -> bool {
    path == "admin.html" || path == "index.html" || path.ends_with(".html")
}

fn has_hash_suffix(path: &str) -> bool {
    // Vite emits assets/<name>-<hash>.<ext>. Check for the dash-hash pattern.
    let stem = path.strip_prefix("assets/").unwrap_or(path);
    let dot = stem.rfind('.').unwrap_or(stem.len());
    let name = &stem[..dot];
    name.contains('-') && name.rfind('-').map(|i| &name[i + 1..]).map_or(false, |h| h.len() >= 6)
}

fn spa_revision() -> &'static str {
    // Compiled-in from build.rs; read .build-revision at build time.
    option_env!("OXIPAGE_SPA_REVISION").unwrap_or("unknown")
}

fn content_hash(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
```

Also add to `build.rs` (in the `validate_and_copy` for the live embed):

```rust
// Compute revision and set it as an env var for compilation.
let revision = compute_revision(&root.join("embedded-spa"));
println!("cargo:rustc-env=OXIPAGE_SPA_REVISION={revision}");
```

```rust
fn compute_revision(dir: &std::path::Path) -> String {
    use std::collections::BTreeMap;
    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    collect_files(dir, "", &mut entries);
    let mut hasher = sha2::Sha256::new();
    for (name, data) in &entries {
        hasher.update(name.as_bytes());
        hasher.update(data);
    }
    format!("{:x}", hasher.finalize())
}

fn collect_files(base: &std::path::Path, rel: &str, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    let dir = if rel.is_empty() { base } else { &base.join(rel) };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                collect_files(base, &rel_path, out);
            } else if let Ok(data) = std::fs::read(entry.path()) {
                out.insert(rel_path, data);
            }
        }
    }
}
```

Add `sha2` to `crates/oxipage-core/Cargo.toml` `[dependencies]`:
```toml
sha2.workspace = true
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxipage-core --test cache_headers`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-core/src/http.rs crates/oxipage-core/build.rs crates/oxipage-core/Cargo.toml crates/oxipage-core/tests/cache_headers.rs
git commit -m "feat(core): cache headers, ETag, SPA revision for static assets"
```

---

### Task 4: AdminErrorBoundary component

**Files:**
- Create: `web/src/admin/shared/ui/AdminErrorBoundary.tsx`
- Modify: `web/src/admin/App.tsx`

**Interfaces:**
- Consumes: nothing
- Produces: `AdminErrorBoundary` React component wrapping all routed Admin content

- [ ] **Step 1: Create the ErrorBoundary component**

Create `web/src/admin/shared/ui/AdminErrorBoundary.tsx`:

```tsx
import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  category: "render" | "chunk-load" | "unknown";
}

/** Compiled SPA revision, injected into admin.html by serve_asset as a meta tag. */
function getSpaRevision(): string {
  return (
    document.querySelector('meta[name="oxipage-spa-revision"]')?.getAttribute("content") ??
    "unknown"
  );
}

export class AdminErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null, category: "unknown" };

  static getDerivedStateFromError(error: Error): State {
    const category: State["category"] = error.message.includes("Failed to fetch dynamically imported module")
      || error.message.includes("error loading dynamically imported module")
      ? "chunk-load"
      : "render";
    return { hasError: true, error, category };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("AdminErrorBoundary caught:", error, info);
  }

  handleReload = () => {
    window.location.reload();
  };

  handleClearCache = async () => {
    try {
      if ("serviceWorker" in navigator) {
        const regs = await navigator.serviceWorker.getRegistrations();
        for (const r of regs) await r.unregister();
      }
      if ("caches" in window) {
        const names = await caches.keys();
        for (const n of names) await caches.delete(n);
      }
    } catch {
      // Best-effort; reload regardless.
    }
    window.location.reload();
  };

  render() {
    if (!this.state.hasError) return this.props.children;

    const isChunk = this.state.category === "chunk-load";

    return (
      <div className="min-h-screen flex items-center justify-center bg-canvas p-8">
        <div className="max-w-md space-y-4 text-center">
          <h1 className="text-xl font-bold text-foreground">
            {isChunk ? "Console needs to reload" : "Console encountered an error"}
          </h1>
          <p className="text-sm text-muted">
            {isChunk
              ? "A cached version of the console is out of date. Reloading will fetch the latest build."
              : "An unexpected error occurred while rendering the console."}
          </p>
          {this.state.error && (
            <pre className="text-xs text-left bg-surface p-3 rounded border border-line overflow-auto max-h-32">
              {this.state.error.message}
            </pre>
          )}
          <div className="flex gap-2 justify-center">
            <button
              onClick={this.handleReload}
              className="px-4 py-2 text-sm font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90"
            >
              Reload console
            </button>
            <button
              onClick={this.handleClearCache}
              className="px-4 py-2 text-sm font-medium rounded-md border border-line text-foreground hover:bg-surface"
            >
              Clear cache and reload
            </button>
          </div>
          <p className="text-xs text-muted">
            SPA revision: <code className="font-mono">{getSpaRevision().slice(0, 12)}</code>
          </p>
        </div>
      </div>
    );
  }
}
```

- [ ] **Step 2: Mount it in AdminApp**

In `web/src/admin/App.tsx`, wrap the routed content:

```tsx
import { AdminErrorBoundary } from "./shared/ui/AdminErrorBoundary";

// Inside AdminApp return:
<QueryClientProvider client={queryClient}>
  <AdminErrorBoundary>
    <BrowserRouter>
      <ScrollToTop />
      <Routes>
        {/* ... existing routes ... */}
      </Routes>
    </BrowserRouter>
  </AdminErrorBoundary>
</QueryClientProvider>
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd web && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Build and smoke test**

Run: `cd web && bun run build`
Expected: build succeeds, `AdminErrorBoundary` chunk appears in dist

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/shared/ui/AdminErrorBoundary.tsx web/src/admin/App.tsx
git commit -m "feat(admin): ErrorBoundary with stale-chunk recovery UI"
```

---

### Task 5: SiteContext path model — resolved absolute paths

**Files:**
- Modify: `crates/oxipage-console/src/sites_runtime.rs`
- Modify: `crates/oxipage-console/src/loader.rs`
- Modify: all files that reference `ctx.path` for DB/out/media resolution

**Interfaces:**
- Consumes: `Config::load` result
- Produces: `SiteContext { project_dir, data_dir, out_dir, media_dir }` replacing `ctx.path` usage

- [ ] **Step 1: Write a test for resolved paths**

Add to `crates/oxipage-console/tests/site_paths.rs`:

```rust
//! Tests for SiteContext resolved paths.

use oxipage_console::sites_runtime::{SiteContext, SiteRegistry};
use oxipage_core::sites::SitesFile;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn site_context_resolves_absolute_data_dir() {
    let dir = TempDir::with_prefix("oxipage-paths-").unwrap();
    let toml = format!(
        r#"[site]
name = "Test"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"
"#,
    );
    std::fs::write(dir.path().join("oxipage.toml"), toml).unwrap();

    let mut sf = SitesFile::default();
    sf.add("test".into(), dir.path().to_path_buf());
    sf.set_default("test");

    let registry = Arc::new(SiteRegistry::new(sf, Default::default(), Default::default()).await.unwrap());
    let ctx = registry.ctx_for("test").await.unwrap();

    // data_dir should be project_dir/data (relative resolved against project_dir).
    assert_eq!(ctx.data_dir, dir.path().canonicalize().unwrap().join("data"));
    assert_eq!(ctx.out_dir, ctx.data_dir.join("out"));
    assert_eq!(ctx.media_dir, ctx.data_dir.join("media"));
    assert!(ctx.data_dir.is_absolute());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxipage-console --test site_paths`
Expected: FAIL — `data_dir`, `out_dir`, `media_dir` fields don't exist yet

- [ ] **Step 3: Add path fields to SiteContext and resolve in SiteLoader**

In `crates/oxipage-console/src/sites_runtime.rs`, replace the `path: PathBuf` field:

```rust
pub struct SiteContext {
    pub slug: String,
    pub project_dir: PathBuf,
    pub data_dir: PathBuf,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    pub startup_server: oxipage_core::config::ServerConfig,
    // config: Arc<Config> is KEPT for now. Task 6 adds `settings`,
    // migrates all readers, THEN removes this field.
    pub config: Arc<oxipage_core::config::Config>,
    pub db: SqlitePool,
    pub registry: Arc<ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub build_guard: Arc<BuildGuard>,
    pub deploy_guard: Arc<DeployGuard>,
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}
```

In `crates/oxipage-console/src/loader.rs`, resolve paths:

```rust
pub async fn load(slug: String, path: PathBuf, build_guard: Arc<BuildGuard>, deploy_guard: Arc<DeployGuard>) -> anyhow::Result<SiteContext> {
    let toml_path = path.join("oxipage.toml");
    let cfg = Config::load(&toml_path)?;

    let project_dir = path.canonicalize().unwrap_or(path);
    let data_dir = if cfg.server.data_dir.is_absolute() {
        cfg.server.data_dir.clone()
    } else {
        project_dir.join(&cfg.server.data_dir)
    };
    tokio::fs::create_dir_all(&data_dir).await?;
    let out_dir = data_dir.join("out");
    let media_dir = data_dir.join("media");

    let db_path = data_dir.join("oxipage.db");
    let db = oxipage_core::db::connect(&db_path).await?;
    let toml_enabled = cfg.extensions.enabled.clone();
    let extensions = crate::all_extensions();
    let registry = Arc::new(ExtensionRegistry::new(extensions));
    registry.run_migrations(&db, &toml_enabled).await?;
    let wasm_loader: Option<Arc<dyn WasmLoader>> = None;

    Ok(SiteContext {
        slug,
        project_dir,
        data_dir,
        out_dir,
        media_dir,
        startup_server: cfg.server.clone(),
        config: Arc::new(cfg),
        db,
        registry,
        builders: Arc::new(crate::all_builders()),
        build_guard,
        deploy_guard,
        wasm_loader,
    })
}
```

- [ ] **Step 4: Migrate all `ctx.path` references to `ctx.project_dir`/`ctx.data_dir`/`ctx.out_dir`/`ctx.media_dir`**

Search and replace across `crates/oxipage-console/src/`:

Run: `grep -rn "ctx\.path" crates/oxipage-console/src/`
Replace each:
- `ctx.path.join("out")` → `ctx.out_dir.clone()`
- `ctx.path.join("oxipage.toml")` → `ctx.project_dir.join("oxipage.toml")`
- `ctx.path.join("data")` → `ctx.data_dir.clone()`
- bare `ctx.path` → `ctx.project_dir.clone()`

Do NOT migrate `ctx.config` references yet — `settings` does not exist until Task 6.
The `config: Arc<Config>` field stays so the crate compiles after this task.
- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p oxipage-console --test site_paths`
Expected: PASS

- [ ] **Step 6: Run full console test suite**

Run: `cargo test -p oxipage-console`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add -A crates/oxipage-console/src/ crates/oxipage-console/tests/site_paths.rs
git commit -m "refactor(console): SiteContext resolved absolute paths (project_dir/data_dir/out_dir/media_dir)"
```

---

### Task 6: MutableSiteSettings and atomic config_write_lock

**Files:**
- Create: `crates/oxipage-core/src/site_paths.rs`
- Modify: `crates/oxipage-core/src/lib.rs`
- Modify: `crates/oxipage-console/src/sites_runtime.rs`
- Modify: `crates/oxipage-console/src/loader.rs`
- Modify: `crates/oxipage-console/src/per_site.rs`

**Interfaces:**
- Consumes: `Config` from `oxipage-core::config`
- Produces: `MutableSiteSettings`, `config_write_lock` pattern, updated `config_put` handler

- [ ] **Step 1: Define MutableSiteSettings**

Create `crates/oxipage-core/src/site_paths.rs`:

```rust
//! Runtime-mutable site settings (display, languages, lobby, integrations, deploy).
//! Server host/port/data_dir are intentionally excluded — they are startup-immutable.

use serde::{Deserialize, Serialize};

/// Live-reloadable subset of site configuration. Excludes `[server]` fields
/// (host/port/data_dir) which are captured once at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableSiteSettings {
    pub site: MutableSiteConfig,
    pub lobby: MutableLobbyConfig,
    pub integrations: MutableIntegrationsConfig,
    pub extensions: MutableExtensionsConfig,
    #[serde(default)]
    pub deploy: DeployConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableSiteConfig {
    pub name: String,
    pub base_url: String,
    pub default_lang: String,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutableLobbyConfig {
    pub default_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MutableIntegrationsConfig {
    #[serde(default)]
    pub github_username: Option<String>,
    #[serde(default)]
    pub tmdb_api_key_env: Option<String>,
    #[serde(default)]
    pub aladin_ttbkey_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MutableExtensionsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// Deploy target configuration. Consumed by subproject 4.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeployConfig {
    #[serde(default)]
    pub github_pages: Option<GitHubPagesTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubPagesTarget {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

impl MutableSiteSettings {
    /// Extract mutable settings from a full Config.
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        MutableSiteSettings {
            site: MutableSiteConfig {
                name: cfg.site.name.clone(),
                base_url: cfg.site.base_url.clone(),
                default_lang: cfg.site.default_lang.clone(),
                languages: cfg.site.languages.clone(),
            },
            lobby: MutableLobbyConfig {
                default_mode: cfg.lobby.default_mode.clone(),
            },
            integrations: MutableIntegrationsConfig {
                github_username: cfg.integrations.github_username.clone(),
                aladin_ttbkey_env: cfg.integrations.aladin_ttbkey_env.clone(),
            },
            extensions: MutableExtensionsConfig {
                enabled: cfg.extensions.enabled.clone(),
            },
            deploy: DeployConfig::default(),
        }
    }
}
```

In `crates/oxipage-console/src/sites_runtime.rs`, the final `SiteContext` struct (combining Task 5 + Task 6) is:

```rust
pub struct SiteContext {
    pub slug: String,
    pub project_dir: PathBuf,
    pub data_dir: PathBuf,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    pub startup_server: oxipage_core::config::ServerConfig,
    pub settings: Arc<RwLock<MutableSiteSettings>>,
    pub config_write_lock: Arc<Mutex<()>>,
    pub db: SqlitePool,
    pub registry: Arc<ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub build_guard: Arc<BuildGuard>,
    pub deploy_guard: Arc<DeployGuard>,
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}
```

There is NO `config: Arc<Config>` field. Server fields are in `startup_server`; mutable fields are in `settings`.

Add to `crates/oxipage-core/src/lib.rs`:

```rust
pub mod site_paths;
```

- [ ] **Step 2: Wire settings into SiteContext**

In `crates/oxipage-console/src/sites_runtime.rs`, add:

```rust
use oxipage_core::site_paths::MutableSiteSettings;
use tokio::sync::RwLock;
use std::sync::Mutex;

pub struct SiteContext {
    // ... existing path/db/registry/guard fields from Task 5 ...
    // config: Arc<Config> still present alongside settings during migration.
    pub config: Arc<oxipage_core::config::Config>,
    pub settings: Arc<RwLock<MutableSiteSettings>>,
    pub config_write_lock: Arc<Mutex<()>>,
}
```

In `crates/oxipage-console/src/loader.rs`, construct:

```rust
let settings = Arc::new(RwLock::new(MutableSiteSettings::from_config(&cfg)));
let config_write_lock = Arc::new(Mutex::new(()));

Ok(SiteContext {
    // ... existing fields from Task 5 (including config: Arc::new(cfg.clone())) ...
    settings,
    config_write_lock,
})
```

- [ ] **Step 2b: Migrate all `ctx.config` readers to `ctx.settings` or `ctx.startup_server`**

Now that `settings` exists alongside `config`, migrate every reader. After this step, `config` has zero consumers and can be removed.

Run: `grep -rn "ctx\.config" crates/oxipage-console/src/`
Replace each:
- `ctx.config.server.host` → `ctx.startup_server.host`
- `ctx.config.server.port` → `ctx.startup_server.port`
- `ctx.config.server.data_dir` → `ctx.data_dir` (already resolved)
- `ctx.config.site.name` → `ctx.settings.read().await.site.name`
- `ctx.config.site.base_url` → `ctx.settings.read().await.site.base_url`
- `ctx.config.site.default_lang` → `ctx.settings.read().await.site.default_lang`
- `ctx.config.site.languages` → `ctx.settings.read().await.site.languages.clone()`
- `ctx.config.lobby.default_mode` → `ctx.settings.read().await.lobby.default_mode.clone()`
- `ctx.config.integrations.*` → `ctx.settings.read().await.integrations.*.clone()`
- `ctx.config.extensions.enabled` → `ctx.settings.read().await.extensions.enabled.clone()`

For handlers in a non-async context or build/deploy snapshots, read once and hold the guard:

```rust
let settings = ctx.settings.read().await;
let base_url = &settings.site.base_url;
```

Run: `cargo build --workspace`
Expected: success — all `ctx.config` references are migrated

- [ ] **Step 2c: Remove the now-unused `config` field**

After all readers migrate, remove `config: Arc<Config>` from `SiteContext` and its construction in `loader.rs`.

Run: `cargo build --workspace`
Expected: success — `config` field is gone, `settings` + `startup_server` cover all needs

- [ ] **Step 3: Update config_put to use lock + reread + allowlisted patch**

In `crates/oxipage-console/src/per_site.rs`, update `config_put`:

```rust
pub async fn config_put(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let toml_path = ctx.project_dir.join("oxipage.toml");

    // Lock to serialize concurrent config writes for this site.
    let _guard = ctx.config_write_lock.lock().unwrap();

    // Reread current TOML (preserves server + unknown sections).
    let raw = tokio::fs::read_to_string(&toml_path).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read toml: {e}")))?;
    let mut doc: toml::Value = toml::from_str(&raw)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse toml: {e}")))?;

    // Apply allowlisted patches (site, lobby, integrations — NOT server).
    // These three helpers are NEW functions defined in per_site.rs that
    // preserve the existing inline TOML-table edit logic at per_site.rs
    // lines 104-183 (site_tbl/lobby_tbl/int_tbl insertion). Extract that
    // logic verbatim into apply_site_patch, apply_lobby_patch, and
    // apply_integrations_patch — each takes &mut toml::Value and the
    // corresponding Option<Update> struct, returns Result<(), (StatusCode,
    // String)> on parse/validation failure. Do not add new fields to these
    // helpers; Task 6 only refactors the existing mutation path.
    apply_site_patch(&mut doc, &update.site)?;
    apply_lobby_patch(&mut doc, &update.lobby)?;
    apply_integrations_patch(&mut doc, &update.integrations)?;

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize toml: {e}")))?;

    // Atomic write: temp file in same directory, then rename.
    let tmp = toml_path.with_extension("toml.tmp");
    tokio::fs::write(&tmp, &serialized).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write tmp: {e}")))?;
    tokio::fs::rename(&tmp, &toml_path).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")))?;

    // Reload and replace settings snapshot.
    let new_cfg = Config::load(&toml_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("reload: {e}")))?;
    *ctx.settings.write().await = MutableSiteSettings::from_config(&new_cfg);

    // Response from the reloaded settings.
    let s = ctx.settings.read().await;
    Ok(Json(ConfigResponse {
        data: serde_json::json!({
            "site": { "name": s.site.name, "base_url": s.site.base_url,
                      "default_lang": s.site.default_lang, "languages": s.site.languages },
            "lobby": { "default_mode": s.lobby.default_mode },
            "integrations": {
                "github_username": s.integrations.github_username,
                "tmdb_api_key_env": s.integrations.tmdb_api_key_env,
                "aladin_ttbkey_env": s.integrations.aladin_ttbkey_env,
            },
            "extensions": { "enabled": s.extensions.enabled },
        }),
    }))
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build --workspace`
Expected: success

Run: `cargo test -p oxipage-console`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add -A crates/oxipage-core/src/site_paths.rs crates/oxipage-core/src/lib.rs crates/oxipage-console/src/
git commit -m "feat(console): MutableSiteSettings with atomic config_write_lock"
```

---

### Task 7: Remove legacy top-level build/deploy routes

**Files:**
- Delete: `crates/oxipage-console/src/build/site_build.rs`
- Delete: `crates/oxipage-console/src/deploy/site_deploy.rs`
- Modify: `crates/oxipage-console/src/router.rs`
- Modify: `crates/oxipage-console/src/build/mod.rs`
- Modify: `crates/oxipage-console/src/deploy/mod.rs`
- Modify: `crates/oxipage-console/tests/build_deploy_preview.rs`

**Interfaces:**
- Consumes: site-scoped routes at `/api/console/s/{slug}/build|deploy` (already mounted via `per_site_router`)
- Produces: no legacy top-level routes; only per-site operation routes remain

- [ ] **Step 1: Update tests to remove references to top-level routes**

In `crates/oxipage-console/tests/build_deploy_preview.rs`, remove or rewrite tests that hit `/build/{slug}` and `/deploy/{slug}`:

```rust
// Remove build_endpoint_rejects_unknown_slug and deploy_endpoint_returns_stub_response
// if they test the top-level routes. The per-site routes are tested separately.
// Keep preview_endpoint_returns_404_for_missing_out_dir (preview is top-level by design).
```

- [ ] **Step 2: Remove the top-level routes from router.rs**

In `crates/oxipage-console/src/router.rs`, remove from `build_top_level_router`:

```rust
// DELETE these two lines:
// .route("/build/{slug}", post(site_build::build_handler))
// .route("/deploy/{slug}", post(site_deploy::deploy_handler))
```

The resulting `build_top_level_router` keeps only:

```rust
pub fn build_top_level_router() -> Router<Arc<SiteRegistry>> {
    Router::new()
        .route("/sites", get(list_sites))
        .route("/sites/default", get(get_default).put(set_default))
        .route("/sites/{slug}", delete(delete_site_handler))
        .route("/preview/{slug}/{*rest}", get(preview_handler))
        .route("/setup/create-site", post(create_site_handler))
}
```

Remove the `use` statements for `site_build` and `site_deploy`.

- [ ] **Step 3: Delete the dead handler files**

```bash
rm crates/oxipage-console/src/build/site_build.rs
rm crates/oxipage-console/src/deploy/site_deploy.rs
```

Update `crates/oxipage-console/src/build/mod.rs` to remove `pub mod site_build;`.
Update `crates/oxipage-console/src/deploy/mod.rs` to remove `pub mod site_deploy;`.

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace`
Expected: success

Run: `cargo test -p oxipage-console`
Expected: remaining tests pass (preview test, site routes test)

- [ ] **Step 5: Verify the SPA still builds and deploys work via per-site routes**

Run: `cd web && bun run build && cd .. && cargo test -p oxipage-console --test site_routes`
Expected: per-site route tests pass

- [ ] **Step 6: Commit**

```bash
git add -A crates/oxipage-console/
git commit -m "refactor(console): remove legacy top-level build/deploy routes"
```

---

### Task 8: Fix favicon reference and end-to-end verification

**Files:**
- Modify: `web/admin.html`

- [ ] **Step 1: Fix the dead favicon reference**

In `web/admin.html`, replace the `/vite.svg` favicon link with an inline SVG data URI or remove it:

```html
<!-- Replace: <link rel="icon" type="image/svg+xml" href="/vite.svg" /> -->
<!-- With: -->
<link rel="icon" href="data:," />
```

- [ ] **Step 2: Build frontend and backend**

```bash
cd web && bun run build && cd .. && cargo build --workspace
```

- [ ] **Step 3: Start console and verify deep links**

```bash
cargo run -p oxipage-cli -- console &
sleep 2
curl -s -D - http://127.0.0.1:8787/sites | head -20
curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8787/api/console/sites
kill %1
```
Expected: `/sites` returns 200 with `Content-Type: text/html` and `Cache-Control: no-cache`; API returns 200 JSON

- [ ] **Step 4: Verify cache headers on a hashed asset**

```bash
cargo run -p oxipage-cli -- console &
sleep 2
ASSET=$(curl -s http://127.0.0.1:8787/sites | grep -o '/assets/admin-[^"]*\.js' | head -1)
curl -s -D - "http://127.0.0.1:8787$ASSET" | grep -i cache-control
kill %1
```
Expected: `Cache-Control: public, max-age=31536000, immutable`

- [ ] **Step 5: Commit**

```bash
git add web/admin.html
git commit -m "fix(admin): favicon reference and end-to-end deep-link verification"
```

---

## Self-Review

**Spec coverage:**
- §3 reproduce `/sites`: Task 8 Step 3
- §3 consolidate embed: Task 1
- §3 fail build on missing admin.html: Task 2 Step 4
- §3 cache/revision: Task 3
- §3 ErrorBoundary: Task 4
- §3 resolve project_dir/data_dir/out_dir/media_dir: Task 5
- §3 MutableSiteSettings + config_write_lock: Task 6
- §3 remove legacy routes: Task 7
- §3 favicon fix: Task 8

**Placeholder scan:** No TBD/TODO/placeholders found.

**Type consistency:** `MutableSiteSettings`, `GitHubPagesTarget`, `DeployConfig` are defined in Task 6 and consumed by subprojects 3–5. `SiteContext` fields match across Tasks 5–7.

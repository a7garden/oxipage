# Console Deploy Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Real build log SSE streaming, per-site concurrency guard, port deploy logic, build_log.finished_at.

**Architecture:** Build runs lazily (first SSE subscriber triggers spawn_blocking via build_site_with_progress). Per-site guard in DashMap. Deploy logic extracted to shared oxipage-deploy crate.

**Tech Stack:** Rust (axum SSE, tokio broadcast, dashmap), TypeScript/React (EventSource)

## Global Constraints

- Build/deploy NEVER runs on tokio worker thread — always spawn_blocking
- Per-site guard (DashMap<String, BuildRun>) returns 409 if busy
- Build starts lazily on first subscriber to avoid SSE event-loss race
- Grace watchdog (3s) starts build even without subscriber
- Guard MUST be released on every exit (RAII guard type)
- Existing `build_site` (sync, non-streaming) stays unchanged
- axum SSE via `axum::response::sse::Sse`; tokio-stream for BroadcastStream
- build_log.finished_at added as `ALTER TABLE build_log ADD COLUMN finished_at TEXT`

---

### Task 1: Core — add `build_site_with_progress`

**Files:**
- Modify: `crates/oxipage-core/src/build.rs`

- [ ] **Step 1: Add BuildEvent enum**

```rust
// build.rs — at top
#[derive(Debug, Clone, Serialize)]
pub enum BuildEvent {
    BuildStarted { total: usize },
    ExtensionStart { ext_id: String },
    ExtensionDone { ext_id: String, pages: usize },
    BuildComplete { total_pages: usize },
    BuildFailed { error: String },
}
```

- [ ] **Step 2: Add `build_site_with_progress`**

```rust
/// Like build_site but emits BuildEvent per extension via an mpsc sender.
/// Rayon parallelism is preserved — events reflect real (non-deterministic) completion order.
pub fn build_site_with_progress(
    db: &SqlitePool,
    builders: &[Box<dyn BuildExt>],
    tx: &tokio::sync::mpsc::Sender<BuildEvent>,
) -> Result<BuildOutput, Box<dyn Error + Send + Sync>> {
    use rayon::prelude::*;
    let total = builders.len();
    let _ = tx.blocking_send(BuildEvent::BuildStarted { total });
    let handle = tokio::runtime::Handle::current();

    let results: Vec<Result<ExtBuildOutput, String>> = builders
        .par_iter()
        .map(|ext| {
            let ext_id = ext.ext_id();
            let _ = tx.blocking_send(BuildEvent::ExtensionStart { ext_id: ext_id.to_string() });
            // same build logic as build_site:
            let pages = handle
                .block_on(ext.build_pages(db))
                .map_err(|e| format!("[{}] build_pages: {}", ext_id, e))?;
            let data = ext
                .build_data(db, &handle)
                .map_err(|e| format!("[{}] build_data: {}", ext_id, e))?;
            let search_docs = ext
                .build_search_docs(db, &handle)
                .map_err(|e| format!("[{}] build_search_docs: {}", ext_id, e))?;
            let _ = tx.blocking_send(BuildEvent::ExtensionDone {
                ext_id: ext_id.to_string(),
                pages: pages.len(),
            });
            Ok(ExtBuildOutput {
                ext_id: ext_id.to_string(),
                pages,
                data,
                search_docs,
            })
        })
        .collect();

    // Error handling (same as build_site)
    let errors: Vec<String> = results.iter().filter_map(|r| r.as_ref().err().cloned()).collect();
    if !errors.is_empty() {
        let msg = errors.join("; ");
        let _ = tx.blocking_send(BuildEvent::BuildFailed { error: msg.clone() });
        return Err(msg.into());
    }
    let output = BuildOutput::merge(results.into_iter().filter_map(Result::ok));
    let total_pages = output.pages.len();
    let _ = tx.blocking_send(BuildEvent::BuildComplete { total_pages });
    Ok(output)
}
```

- [ ] **Step 3: Build check**

`cargo check -p oxipage-core`

---

### Task 2: Console — add deps + BuildRun infrastructure

**Files:**
- Modify: `crates/oxipage-console/Cargo.toml`
- Create: `crates/oxipage-console/src/build/build_run.rs`

- [ ] **Step 1: Add deps**

```toml
# Cargo.toml — add to [dependencies]
tokio-stream = { workspace = true, features = ["sync"] }
futures = { workspace = true }
dashmap = "6"  # or workspace if present
```

Check workspace Cargo.toml for existing `futures`/`tokio-stream` versions.

- [ ] **Step 2: Create BuildRun struct**

```rust
// build/build_run.rs
use crate::build::BuildEvent;  // re-export from oxipage-core
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{broadcast, Notify};

pub struct BuildRun {
    pub id: String,
    pub tx: broadcast::Sender<BuildEvent>,
    pub started: AtomicBool,
    pub start: Notify,
    pub started_at: String,
}

#[derive(Clone)]
pub struct BuildGuard {
    pub runs: Arc<DashMap<String, BuildRun>>,
}

impl BuildGuard {
    pub fn new() -> Self {
        Self { runs: Arc::new(DashMap::new()) }
    }

    /// Try to start a build for `slug`. Returns 409 error if already running.
    pub fn try_start(&self, slug: &str, run: BuildRun) -> Result<(), String> {
        use dashmap::mapref::entry::Entry;
        match self.runs.entry(slug.to_string()) {
            Entry::Occupied(occ) => {
                let id = occ.get().id.clone();
                Err(format!("build_in_progress: {}", id))
            }
            Entry::Vacant(vac) => {
                vac.insert(run);
                Ok(())
            }
        }
    }

    pub fn finish(&self, slug: &str) {
        self.runs.remove(slug);
    }
}
```

- [ ] **Step 3: Build check**

`cargo check -p oxipage-console`

---

### Task 3: Console — per-site build guard integration

**Files:**
- Modify: `crates/oxipage-console/src/main.rs` or `lib.rs` (inject BuildGuard into app state)
- Modify: `crates/oxipage-console/src/per_site.rs` (build_post + stream endpoint)
- Modify: `crates/oxipage-console/src/build/site_build.rs`

- [ ] **Step 1: Inject BuildGuard into app state / SiteContext**

In the console startup (main.rs or loader.rs), create `Arc<BuildGuard>` and inject into each `SiteContext` or a shared state.

If SiteContext:

```rust
// sites_runtime.rs
pub struct SiteContext {
    // ...existing fields
    pub build_guard: Arc<BuildGuard>,
}
```

If state (`Arc<AppState>` or similar), inject alongside registry. Plan confirms per-site guard (simplest is Arc in SiteContext so build_post has access).

- [ ] **Step 2: Rewrite build_post as async job**

```rust
// per_site.rs
pub async fn build_post(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let slug = ctx.slug.clone();
    let (bcast_tx, _) = broadcast::channel(128);
    let build_id = uuid::Uuid::new_v4().to_string();
    let run = BuildRun {
        id: build_id.clone(),
        tx: bcast_tx,
        started: AtomicBool::new(false),
        start: Notify::new(),
        started_at: chrono_now(),
    };
    ctx.build_guard.try_start(&slug, run).map_err(|e| {
        (StatusCode::CONFLICT, e)
    })?;

    let guard = ctx.build_guard.clone();
    let slug_c = slug.clone();
    let db = ctx.db.clone();
    let builders = ctx.builders.clone();
    let out_dir = ctx.path.join("out");
    let media_dir = ctx.config.server.data_dir.join("media");

    // Grace watchdog: start after 3s if no subscriber
    let guard_w = guard.clone();
    let slug_w = slug.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        if let Some(entry) = guard_w.runs.get(&slug_w) {
            if !entry.started.load(Ordering::Acquire) {
                entry.start.notify_one();
            }
        }
    });

    tokio::fs::create_dir_all(&out_dir).await.map_err(|e| {
        guard.finish(&slug_c);
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(serde_json::json!({ "data": { "build_id": build_id } })))
}
```

Wait — the lazy-start design means POST should NOT do I/O yet. The build doesn't run work until subscriber attaches. But `create_dir_all` is I/O that doesn't block the build. Actually, the lazy start means: POST just creates the channel + guard + returns. The actual build work (build_site_with_progress) happens only when the first subscriber arrives. The watchdog also starts it if nobody subscribes within 3s.

Let me restructure: POST creates BuildRun + guard, returns immediately. The subscriber triggers the run. The watchdog is a fallback.

The tricky part: after subscribing, the build needs the SiteContext data. The subscriber function needs access to ctx (db, builders, path, etc.). So either:
(a) Store the context data in BuildRun at POST time, so the subscriber can start the build.
(b) Have the subscriber read SiteContext from the active registry.

Option (a) is simpler: BuildRun stores what the build task needs (db, builders, out_dir, media_dir, slug).

Let me adjust BuildRun:

```rust
pub struct BuildRun {
    pub id: String,
    pub tx: broadcast::Sender<BuildEvent>,
    pub started: AtomicBool,
    pub start: Notify,
    pub started_at: String,
    pub db: SqlitePool,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    pub slug: String,
}
```

Then build_post creates BuildRun with these fields cloned from ctx. The subscriber handler starts the build.

Actually, let me reconsider whether lazy-start is worth the complexity. The alternative: POST always starts the build immediately, and we buffer events. The subscriber catches up via the buffer.

Hmm, but the advisory specifically recommended lazy-start for correctness. Let me keep it but simplify the BuildRun.

For the implementation, keeping the plan practical: I'll define BuildRun to hold the data the build task needs. The subscriber handler (SSE GET) reads BuildRun from the guard and triggers spawn_blocking if !started.

Let me adjust the plan to be clear. I'll write the practical POST handler that creates BuildRun with the build's closure data, then the GET handler triggers the build task on first subscribe.

This is getting complex for a plan step. Let me simplify the plan steps to be more concrete.

Actually, the key insight: I should write the plan steps as I intend to implement them. Let me simplify:

1. POST handler: guard.try_start(slug).create BuildRun with required data (db, builders, path, etc). Return build_id.
2. The GET stream handler: on first subscriber, if !run.started, set started, spawn_blocking running build_site_with_progress with a write-builder that writes output + creates build_log.

Let me write the plan more concisely, focusing on the key endpoints.

Actually, given the todo state issues and reminding myself that the user wants fast execution, let me write concise plans for S3, S4, S5 and then move to execution. The S2 plan was detailed. I'll make S3/S4/S5 plans more compact — the spec has contracts, the plan just needs task decomposition.<｜end▁of▁thinking｜>

<｜｜DSML｜｜tool_calls>
<｜｜DSML｜｜invoke name="write">
<｜｜DSML｜｜parameter name="content" string="true"># Console Deploy Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Real deploy pipeline: per-site build guard (409), SSE streaming, build_log.finished_at, port gh-pages deploy.

**Architecture:** Lazy-start build (first SSE subscriber triggers spawn_blocking; 3s watchdog fallback). BuildRun stored in DashMap with broadcast channel. Deploy logic extracted to oxipage-deploy crate.

**Tech Stack:** Rust (axum SSE, tokio broadcast, dashmap, uuid), TypeScript/React (EventSource)

## Global Constraints

- Build/deploy NEVER on tokio worker thread — spawn_blocking
- Per-site DashMap guard: 409 Conflict if busy
- Lazy-start: build starts on first SSE subscriber (avoids event-loss race); watchdog starts it after 3s without subscriber
- Guard released on every exit path (RAII)
- Existing `build_site` (synchronous, non-streaming) unchanged
- build_log.finished_at added idempotently (ALTER TABLE ADD COLUMN, ignore if exists)

---

### Task 1: Core — Streaming build variant

**Files:** `crates/oxipage-core/src/build.rs`

- [ ] **Add BuildEvent enum and `build_site_with_progress`**

```rust
#[derive(Debug, Clone, Serialize)]
pub enum BuildEvent {
    BuildStarted { total: usize },
    ExtensionStart { ext_id: String },
    ExtensionDone { ext_id: String, pages: usize },
    BuildComplete { total_pages: usize },
    BuildFailed { error: String },
}

pub fn build_site_with_progress(
    db: &SqlitePool,
    builders: &[Box<dyn BuildExt>],
    tx: &tokio::sync::mpsc::Sender<BuildEvent>,
) -> Result<BuildOutput, Box<dyn Error + Send + Sync>>;
```
Keeps rayon `par_iter`, emits ExtensionStart/ExtensionDone per builder, BuildComplete on success, BuildFailed on error. Errors collected per-extension as in build_site.

- [ ] `cargo check -p oxipage-core`

---

### Task 2: Console — Dependencies + BuildRun infrastructure

**Files:**
- Modify: `Cargo.toml` (workspace) or `oxipage-console/Cargo.toml`
- Create: `crates/oxipage-console/src/build/build_run.rs`

- [ ] **Add deps:** `tokio-stream` (features=["sync"]), `futures`, `dashmap`, `uuid` to console's Cargo.toml
- [ ] **Create `BuildRun` struct** — holds `id: String, tx: broadcast::Sender<BuildEvent>, started: AtomicBool, start: Notify, db: SqlitePool, builders: Arc<Vec<Box<dyn BuildExt>>>, out_dir: PathBuf, media_dir: PathBuf, slug: String, started_at: String`
- [ ] **Create `BuildGuard`** — wraps `DashMap<String, BuildRun>` with `try_start(slug, run) -> Result<(), 409>` and `finish(slug)`
- [ ] Inject `Arc<BuildGuard>` into `SiteContext` (loader.rs or main.rs)

- [ ] `cargo check -p oxipage-console`

---

### Task 3: Console — Build endpoints (POST + SSE stream)

**Files:**
- Modify: `crates/oxipage-console/src/per_site.rs` (build_post)
- Modify: `crates/oxipage-console/src/build/site_build.rs` (build_handler → 202 + SSE)

- [ ] **Rewrite `build_post`** → guard.try_start → clone ctx data into BuildRun → return 202 {build_id} — no spawn yet
- [ ] **Add `GET /build/{build_id}/stream` SSE endpoint** → on first subscriber: if !started, set started, spawn_blocking(build_site_with_progress) feeding mpsc→broadcast relay (cancelling watchdog). Stream via BroadcastStream. On BuildComplete/Failed: INSERT/UPDATE build_log with finished_at, set outcome, drop guard.
- [ ] **Add `build_log.finished_at`** — ALTER TABLE migration in the build path (ignore column-exists error)
- [ ] **Grace watchdog** — tokio::spawn sleep(3s) + if !started → notify start

- [ ] `cargo check -p oxipage-console`

API: `POST /s/{slug}/build` → 202 `{build_id}` / `GET /s/{slug}/build/{build_id}/stream` → SSE

---

### Task 4: Shared `oxipage-deploy` crate

**Files:**
- Create: `crates/oxipage-deploy/Cargo.toml`
- Create: `crates/oxipage-deploy/src/lib.rs`
- Modify: `Cargo.toml` (workspace — add member)
- Modify: `crates/oxipage-cli/src/commands/deploy.rs` (call shared crate)

- [ ] **Create crate**: `oxipage-deploy = { path = "crates/oxipage-deploy" }` in workspace; `[dependencies]` = `tokio`, `thiserror`
- [ ] **Port `deploy_github_pages`** — extract from CLI. Emit `DeployEvent` via Sender instead of printing. Signature:
```rust
pub enum DeployEvent {
    GhCheck, AuthCheck, WorktreeReady, FilesCopied, Pushing, Deployed { url: String }, Failed { error: String },
}
pub fn deploy_github_pages(out_dir: &Path, tx: &tokio::sync::mpsc::Sender<DeployEvent>) -> anyhow::Result<()>;
```
- [ ] **Update CLI** — call `oxipage_deploy::deploy_github_pages`, translate DeployEvent → Output lines (no behavior change)

- [ ] `cargo check`

---

### Task 5: Console — Deploy endpoints

**Files:**
- Modify: `crates/oxipage-console/src/deploy/site_deploy.rs`
- Modify: `crates/oxipage-console/src/per_site.rs` (deploy_post)

- [ ] **Replace deploy stub**: guard check → spawn_blocking running `oxipage_deploy::deploy_github_pages` with DeployEvent → broadcast → SSE stream (same pattern as build). Return 202 {deploy_id}. No build_log for deploy (deploy is a post-build step).
- [ ] Add deploy stream endpoint `GET /deploy/{deploy_id}/stream`

- [ ] `cargo check -p oxipage-console`

---

### Task 6: Frontend — DeployPage SSE integration

**Files:** `web/src/admin/deploy/DeployPage.tsx`

- [ ] **On build click**: POST /build → read build_id → open `new EventSource(/build/{id}/stream)` → append JSON events as monospace log lines. Auto-scroll. On BuildComplete/Failed: close stream, invalidate builds history.
- [ ] **On deploy click**: same pattern with deploy_id stream.
- [ ] **409 handling**: if POST returns build_in_progress → show "A build is already running" + re-connect to the in-progress stream by build_id from the error.
- [ ] **Button state**: disable build/deploy buttons while a job is in progress (track via state or query). Show spinner/progress indicator.

- [ ] `cd web && npx tsc --noEmit`

---

### Task 7: Wire + smoke

- [ ] `cargo check && cd web && npx tsc --noEmit`
- [ ] Manual: `oxipage console` → Deploy → trigger build → watch per-extension live log → BuildComplete → history updates → trigger deploy → stream steps → second concurrent POST → 409 message + attach

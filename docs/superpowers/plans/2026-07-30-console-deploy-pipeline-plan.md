# Console Deploy Pipeline — Implementation Plan

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
- axum SSE via `axum::response::sse::Sse`; tokio-stream for BroadcastStream

---

### Task 1: Core — Streaming build variant

**Files:** `crates/oxipage-core/src/build.rs`

- [ ] **Add BuildEvent enum and `build_site_with_progress`**

Keeps rayon `par_iter`, emits ExtensionStart/ExtensionDone to `tx` per builder, BuildComplete on success, BuildFailed on error. Errors collected per-extension as in build_site.

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

- [ ] `cargo check -p oxipage-core`

---

### Task 2: Console — Dependencies + BuildRun infrastructure

**Files:**
- Modify: `crates/oxipage-console/Cargo.toml` (+tokio-stream, +futures, +dashmap, +uuid)
- Create: `crates/oxipage-console/src/build/build_run.rs`

`BuildRun` holds ALL data the lazy-started build task needs (db, builders, out_dir, media_dir, slug) so the subscriber can run `build_site_with_progress` without the original request context:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{broadcast, Notify};
use dashmap::DashMap;
use sqlx::SqlitePool;
use oxipage_core::builder::BuildExt;

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

pub struct BuildGuard {
    pub runs: DashMap<String, BuildRun>,
}

impl BuildGuard {
    pub fn new() -> Self { Self { runs: DashMap::new() } }
    pub fn try_start(&self, slug: &str, run: BuildRun) -> Result<(), String> { /* Occupied → 409; Vacant → insert */ }
    pub fn finish(&self, slug: &str) { self.runs.remove(slug); }
}
```

Inject `Arc<BuildGuard>` into `SiteContext`. Add to `sites_runtime.rs`:
```rust
pub struct SiteContext {
    // ...existing
    pub build_guard: Arc<BuildGuard>,
}
```

- [ ] `cargo check -p oxipage-console`

---

### Task 3: Console — Build endpoints (POST + SSE stream)

**Files:**
- Modify: `crates/oxipage-console/src/per_site.rs` (build_post → 202; add stream handler)
- Modify: `crates/oxipage-console/src/build/site_build.rs` (add stream route)

- [ ] **`POST /s/{slug}/build`** → guard.try_start(slug). If busy → 409 `{error:"build_in_progress", build_id}`. Create BuildRun from ctx, clone db/builders/out_dir/media_dir/slug into it. **No spawn yet.** Schedule 3s watchdog (tokio::spawn sleep + notify start if !started). Return 202 `{build_id}`.

- [ ] **`GET /s/{slug}/build/{build_id}/stream`** → Look up BuildRun from guard. On first subscriber: if !started, set started (AtomicBool CAS), spawn_blocking running `build_site_with_progress` feeding an mpsc → relayed to the broadcast. This is the event stream. (First subscriber sees BuildStarted first — zero event loss.) Stream BuildEvents as SSE JSON. On BuildComplete/Failed: INSERT/UPDATE build_log (status, page_count, out_dir, started_at=created_at, finished_at=TEXT), set outcome, drop guard by calling guard.finish(slug).

- [ ] **build_log.finished_at** — ALTER TABLE ADD COLUMN idempotent migration in the build_log path. Stats#last_build (from S1) already reads build_log — finished_at appears automatically.

- [ ] `cargo check -p oxipage-console`

---

### Task 4: Shared `oxipage-deploy` crate

**Files:**
- Create: `crates/oxipage-deploy/Cargo.toml`
- Create: `crates/oxipage-deploy/src/lib.rs`
- Modify: `Cargo.toml` (workspace — add member)
- Modify: `crates/oxipage-cli/src/commands/deploy.rs` (call shared crate)

- [ ] **Create crate**: `oxipage-deploy = { path = "crates/oxipage-deploy" }` in workspace members; deps = `tokio`, `thiserror`
- [ ] **Port `deploy_github_pages`** from `oxipage-cli/src/commands/deploy.rs:32-152`: extract the gh/auth check, worktree create, cp, commit+push logic into `oxipage_deploy::deploy_github_pages(out_dir: &Path, tx: &Sender<DeployEvent>)`. Emit events instead of printing via `Output`. Return Result.
```rust
pub enum DeployEvent {
    GhCheck, AuthCheck, WorktreeReady, FilesCopied { count: usize }, Pushing,
    Deployed { url: String }, Failed { error: String },
}
```
- [ ] **Update CLI deploy.rs** — call the shared fn, translate DeployEvent to `out.ok("...")` lines (no behavior change).

- [ ] `cargo check`

---

### Task 5: Console — Deploy endpoints

**Files:**
- Modify: `crates/oxipage-console/src/deploy/site_deploy.rs` (replace stub)
- Modify: `crates/oxipage-console/src/per_site.rs` (deploy_post)

- [ ] **`POST /s/{slug}/deploy`** → guard check (reuse the same guard or a separate deploy guard), clone ctx, create DeployRun with broadcast, return 202. Lazy-start: on first subscriber, spawn_blocking(deploy_github_pages). Stream DeployEvents as SSE.
- [ ] **`GET /s/{slug}/deploy/{deploy_id}/stream`** → SSE of DeployEvent JSON.

- [ ] `cargo check -p oxipage-console`

---

### Task 6: Frontend — DeployPage SSE integration

**Files:** `web/src/admin/deploy/DeployPage.tsx`

- [ ] **Build click**: `POST /build` → read `build_id` → `new EventSource(/build/{build_id}/stream)` → append each JSON event as a monospace log line in a scrollable dark box. On BuildComplete/Failed: close stream, invalidate `["site",slug,"builds"]`.
- [ ] **Deploy click**: same, with deploy_id stream.
- [ ] **409**: if POST 409s, show "A build is already running for this site" + attach to the in-progress stream via the returned build_id.
- [ ] **Button state**: disable build/deploy while in-progress. Show spinner.

- [ ] `cd web && npx tsc --noEmit`

---

### Task 7: Wire + smoke

- [ ] `cargo check && cd web && npx tsc --noEmit`
- [ ] Manual: `oxipage console` → Deploy → trigger build → watch per-extension log → BuildComplete → history refreshes → trigger deploy → stream steps → second concurrent POST → 409 + attach

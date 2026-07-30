# Console Deploy Pipeline — Design Spec

> **Date:** 2026-07-30
> **Sub-project:** 3 of the decomposed "remaining console work" (Phase 9).
> **Scope:** Build log SSE streaming, real deploy action, build concurrency safety, `build_log.finished_at`.
> **Predecessor:** `2026-07-30-console-data-foundation-design.md` (S1).

## 1. Goal

Turn the Deploy page from a fire-and-forget button over a stub into a live, observable pipeline: stream per-extension build progress over SSE, run a real `gh-pages` deploy, and — critically — prevent the concurrent-build race that the current `build_post` has no defense against.

## 2. Scope

### In scope
- **Per-site build guard:** reject concurrent builds for the same site (409), preventing `write_build_output` interleaving + `build_log` INSERT races.
- **Build streaming:** SSE endpoint emitting `BuildEvent`s; a core `build_site_with_progress` variant that emits per-extension events while keeping rayon parallelism.
- **`spawn_blocking` build execution:** moves the rayon `par_iter` off the tokio worker thread (current `build_post` blocks a worker).
- **`build_log.finished_at`:** track build duration.
- **Deploy action:** replace the stub with the real `gh-pages` pipeline (ported from the CLI into a shared module).
- **DeployPage UX:** live log panel + in-progress button disabling + 409 handling.

### Out of scope (flagged, deferred)
- **Non-gh-pages targets** (cloudflare, netlify): the CLI `deploy()` routes on `target`; only `github-pages` has a real implementation today. Other targets remain dry-run/stub.
- **Scheduled/CI deploy:** manual trigger only.
- **Deploy rollback:** separate concern.

## 3. Current State (grounding)

| Concern | Current state | File |
|---------|--------------|------|
| Build core | `build_site(db, builders)` — rayon `par_iter`, sync, **no progress callback**; returns `BuildOutput` | `oxipage-core/src/build.rs:15-64` |
| `build_post` | runs `build_site` **directly in the async handler** (blocks tokio worker); no concurrency guard | `per_site.rs:410-449` |
| `build_log` schema | `id, status, created_at, page_count, out_dir` — **no `finished_at`** | `per_site.rs:424-431` |
| `deploy_post` | pure **stub** — returns `status:"queued"` + a note pointing to the CLI | `per_site.rs:451-462`, `deploy/site_deploy.rs` |
| CLI deploy | `deploy_github_pages(out_dir, dry_run, out)` — gh check, git worktree/orphan branch, `cp`, commit+push; ~120 lines of subprocess orchestration; prints via `Output` (CLI-specific) | `oxipage-cli/src/commands/deploy.rs:32-152` |
| DeployPage | Build/Deploy mutations fire POSTs; no streaming, no in-progress guard | `web/src/admin/deploy/DeployPage.tsx` |
| Console deps | no `futures` / `tokio-stream` / `dashmap` | `oxipage-console/Cargo.toml` |

### Why the build guard is load-bearing
`build_post` writes to the shared `ctx.path/out` directory and inserts into `build_log`. Two concurrent `POST /build` for the same site interleave `write_build_output` (corrupted output) and race `build_log` rows. **SSE streaming lengthens the build window**, so without a guard collisions become *more* likely, not less. The guard is therefore a prerequisite of streaming, not a nicety.

## 4. Architecture

```
Client                          Console                         Core
─────                           ────────                        ────
POST /build  ──────►  build guard check ──409──►  {error:"build_in_progress"}
                 │            │
                 │     spawn_blocking ──►  build_site_with_progress(db, builders, tx)
                 │            │                      │ per-extension events
                 │     broadcast fan-out ◄──────────┘
                 │            │
GET /build/{id}/stream  ──►  SSE (BroadcastStream) ──►  client log panel
                 │            │
                 │     on BuildComplete/Failed: UPDATE build_log.finished_at; drop guard
                 ▼
```

Build runs as a detached background task keyed by `build_id` in a shared `BuildRuns` map. The `mpsc`→`broadcast` split lets the producing task fan events to zero or more SSE subscribers (a client that connects after completion replays the buffered tail + terminal event).

## 5. Backend — contracts

### 5.1 Per-site build guard

- `BuildRuns` shared state: `DashMap<String /*slug*/, BuildRun>` where `BuildRun { id, tx: broadcast::Sender<BuildEvent>, started_at }`. Injected alongside the registry (a small `Arc` in `AppState` or the console app state).
- `POST /build`: `try_insert(slug)`. If present → `409 { "error": "build_in_progress", "build_id": "<id>" }`. On the path, insert, run, remove on completion (success or error — use a guard/`Drop` to guarantee removal even on panic).
- Deploy reuses the same guard: a deploy while a build runs for that site → 409 (and vice versa, or a separate deploy guard — the plan decides; recommendation: one "site busy" guard covering both).

### 5.2 Core streaming build

Add to `oxipage-core/src/build.rs`:
```rust
pub enum BuildEvent {
    BuildStarted   { total: usize },
    ExtensionStart { ext_id: String },
    ExtensionDone  { ext_id: String, pages: usize },
    BuildComplete  { total_pages: usize },
    BuildFailed    { error: String },
}

pub fn build_site_with_progress(
    db: &SqlitePool,
    builders: &[Box<dyn BuildExt>],
    tx: &tokio::sync::mpsc::Sender<BuildEvent>,
) -> Result<BuildOutput, Box<dyn Error + Send + Sync>>;
```
- Keeps the existing `build_site` (unchanged, for non-console callers). The streaming variant runs the same rayon `par_iter` but, instead of collecting silently, emits `ExtensionDone` per builder as each completes. Rayon completion order is non-deterministic; events reflect real completion order.
- Errors collected per-extension (existing behavior); a fatal collection error sends `BuildFailed`. The Tokio handle capture stays valid because the call still originates from a tokio context (now via `spawn_blocking`).

### 5.3 Build endpoints

**`POST /api/console/s/{slug}/build`** → `202 Accepted { "build_id": "<uuid>" }`
- Guard check (409 if busy). Spawn `build_site_with_progress` in `spawn_blocking`, feeding an `mpsc` relayed to a `broadcast`. Insert into `BuildRuns`. On completion: `INSERT build_log (status, page_count, out_dir, finished_at)` / on failure: `status='failed'`. Remove guard.

**`GET /api/console/s/{slug}/build/{build_id}/stream`** → `text/event-stream` (SSE)
- Subscribes to the run's `broadcast` via `tokio_stream::wrappers::BroadcastStream`. Each `BuildEvent` → `data: <json>\n\n`. Terminates after `BuildComplete`/`BuildFailed`. Unknown `build_id` (completed/evicted) → a single terminal event with the last known status (or 404).

### 5.4 `build_log.finished_at`

Migration-style (idempotent, like the existing `CREATE TABLE IF NOT EXISTS`):
```sql
ALTER TABLE build_log ADD COLUMN finished_at TEXT;  -- guarded: column already exists → ignore error
```
- `created_at` is the start; `finished_at` set on completion. `stats` `last_build` (S1) gains `finished_at`.

### 5.5 Deploy — port gh-pages into a shared module

- Extract `deploy_github_pages` from `oxipage-cli` into a shared module that emits events instead of printing. **New crate `oxipage-deploy`** (or `oxipage-core::deploy`) — recommended a thin crate to keep core free of git/gh subprocess concerns:
```rust
pub enum DeployEvent { GhCheck, AuthCheck, WorktreeReady, FilesCopied, Pushing, Deployed { url }, Failed { error } }
pub fn deploy_github_pages(out_dir: &Path, tx: &Sender<DeployEvent>) -> Result<()>;
```
- CLI's `deploy.rs` calls the shared fn and translates `DeployEvent` → `Output` lines (no behavior change).
- Console `deploy_post` (`deploy/site_deploy.rs`): reuse the site busy-guard; run in `spawn_blocking`; stream `DeployEvent` via SSE (`GET /deploy/{deploy_id}/stream`) mirroring the build pattern; `POST /deploy` returns `202 { deploy_id }`.
- Deploy writes nothing to `build_log` (it is a publish step, not a build). A `deploy_log` table is optional; recommendation: reuse `build_log` with `status='deployed'` row, or skip logging in S3.

## 6. Frontend — DeployPage

- **Trigger buttons** disabled while a build/deploy is in progress for this site (track via a `["site",slug,"build","status"]` query or local SSE state).
- **Live log panel:** on `POST /build` → read `build_id` → open `EventSource` on `/build/{build_id}/stream`; append lines to a monospace dark box (`✓ blog — 12 pages`, etc.). Auto-scroll. On terminal event → close, invalidate `["site",slug,"builds"]`, update last-build card.
- **409 handling:** if `POST /build` returns `build_in_progress`, surface "A build is already running" + offer to **attach** to the in-progress stream by its `build_id`.
- Deploy mirrors: `POST /deploy` → stream deploy steps.

New client fns in `api.ts`: `startBuild(slug): Promise<{build_id}>`, `startDeploy(slug): Promise<{deploy_id}>`. (SSE is opened directly via `EventSource`, not `contentClient`.)

## 7. Dependencies

`oxipage-console/Cargo.toml`:
- `tokio-stream` (BroadcastStream → SSE)
- `futures` (if axum's `Sse` body needs it)
- `dashmap` (build guard map) — check workspace; add if absent.
- axum SSE (`axum::response::sse::Sse`) is available on the workspace `axum` version; confirm no feature flag needed.

New crate `oxipage-deploy` (or module) added to the workspace.

## 8. Constraints

- Build/deploy run in `spawn_blocking` — never on a tokio worker thread.
- The busy-guard **must** be released on every exit path (success, error, panic) — wrap in a RAII guard.
- `build_site` (the non-streaming original) stays unchanged for non-console callers.
- SSE events are JSON-serialized `BuildEvent`/`DeployEvent`; the client maps them to log lines.
- A client connecting after completion still gets the terminal event (broadcast buffer + per-run terminal snapshot).

## 9. Testing

- **Server:** concurrent `POST /build` for one site → first 202, second 409; `finished_at` populated on success and on failure; `build_site_with_progress` emits one `ExtensionDone` per builder and a `BuildComplete` with the correct total; SSE stream delivers events in order and closes on terminal.
- **Deploy:** shared `deploy_github_pages` emits the expected `DeployEvent` sequence in dry-run (mock gh/git) and surfaces `Failed` cleanly when gh is missing.
- **Manual smoke:** `oxipage console` → Deploy page: trigger build, watch live per-extension log, disable buttons during build, trigger deploy, watch step stream; second build tab → "already running" + attach.

## 10. File map

```
crates/oxipage-core/src/
└── build.rs        # +BuildEvent, +build_site_with_progress (build_site unchanged)

crates/oxipage-deploy/            # NEW shared deploy crate
├── Cargo.toml
└── src/lib.rs      # DeployEvent + deploy_github_pages(out_dir, tx)  [ported from CLI]

crates/oxipage-cli/src/commands/
└── deploy.rs       # call oxipage-deploy; translate DeployEvent→Output (behavior unchanged)

crates/oxipage-console/src/
├── build/site_build.rs   # +build guard, +spawn_blocking, +SSE stream endpoint, +finished_at
├── deploy/site_deploy.rs # replace stub: real deploy via oxipage-deploy + SSE
├── per_site.rs           # build_post/deploy_post → async job + stream wiring
└── Cargo.toml            # +tokio-stream, +futures, +dashmap, +oxipage-deploy

web/src/admin/
├── shared/api.ts                 # +startBuild, +startDeploy
└── deploy/DeployPage.tsx         # live SSE log panel; in-progress guard; 409 attach
```

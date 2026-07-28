# Console Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task.

**Goal:** Rename `oxipage-server` → `oxipage-console` (crate, CLI command, HTTP route prefix) and remove `oxipage-admin` to align the codebase with v2 SSG model terminology.

**Architecture:** Single Cargo workspace move. Update imports, CLI subcommand, HTTP routes, and all docs. Provide 301 redirect for `/api/v1/*` → `/api/console/*` for backward compat.

**Tech Stack:** Rust workspace, axum, clap, redis (existing)

**Spec:** `docs/superpowers/specs/2026-07-28-console-rename-design.md`

## Global Constraints

- Edition 2024, clippy `-D warnings` clean
- `cargo test --workspace` must pass
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `/api/v1/*` → `/api/console/*` redirect must work

---

### Task 1: Rename `oxipage-server` crate directory to `oxipage-console`

**Files:**
- Move: `crates/oxipage-server/` → `crates/oxipage-console/`
- Modify: `Cargo.toml` (workspace members)
- Modify: `crates/oxipage-console/Cargo.toml` (name field)

- [ ] **Step 1: Move the directory**

```bash
git mv crates/oxipage-server crates/oxipage-console
```

- [ ] **Step 2: Update workspace Cargo.toml**

Edit `Cargo.toml`:
```diff
 members = [
     "crates/oxipage-core",
-    "crates/oxipage-server",
+    "crates/oxipage-console",
     "crates/oxipage-cli",
```

- [ ] **Step 3: Update the crate's Cargo.toml name**

Edit `crates/oxipage-console/Cargo.toml`:
```diff
 [package]
-name = "oxipage-server"
+name = "oxipage-console"
 version = "0.1.0"
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p oxipage-console
```

- [ ] **Step 5: Commit**

```
git add -A
git commit -m "refactor: rename oxipage-server crate → oxipage-console"
```

---

### Task 2: Rename public functions in `oxipage-console` (server → console)

**Files:**
- Modify: `crates/oxipage-console/src/lib.rs`

- [ ] **Step 1: Rename functions**

```diff
-pub async fn run_server() -> anyhow::Result<()> {
-    run_server_with_extensions(all_extensions()).await
+pub async fn run_console() -> anyhow::Result<()> {
+    run_console_with_extensions(all_extensions()).await
 }

-pub async fn run_server_with_extensions(all: Vec<Arc<dyn Extension>>) -> anyhow::Result<()> {
+pub async fn run_console_with_extensions(all: Vec<Arc<dyn Extension>>) -> anyhow::Result<()> {
```

- [ ] **Step 2: Add deprecated alias for `run_server`**

```rust
#[deprecated(note = "Use run_console() — v2 renames server to console")]
pub async fn run_server() -> anyhow::Result<()> {
    run_console().await
}

#[deprecated(note = "Use run_console_with_extensions()")]
pub async fn run_server_with_extensions(all: Vec<Arc<dyn Extension>>) -> anyhow::Result<()> {
    run_console_with_extensions(all).await
}
```

- [ ] **Step 3: Update internal callers in same file**

Find any other references to `run_server_with_extensions` and update them to `run_console_with_extensions`.

- [ ] **Step 4: Verify**

```bash
cargo check -p oxipage-console 2>&1 | head -20
```

Expect warnings about deprecated `run_server` calls if any remain.

- [ ] **Step 5: Commit**

```
git add crates/oxipage-console/src/lib.rs
git commit -m "refactor(console): rename run_server → run_console (with deprecated alias)"
```

---

### Task 3: Update all `oxipage_server::*` imports to `oxipage_console::*`

**Files:**
- Multiple (grep first to find)

- [ ] **Step 1: Find all references**

```bash
grep -rln "oxipage_server" --include="*.rs" --include="*.toml" crates/
```

- [ ] **Step 2: Update each file**

```rust
// Replace:
use oxipage_server::all_builders;
// With:
use oxipage_console::all_builders;
```

- [ ] **Step 3: Verify with check**

```bash
cargo check --workspace 2>&1 | head -30
```

- [ ] **Step 4: Commit**

```
git add -A
git commit -m "refactor: update all imports oxipage_server → oxipage_console"
```

---

### Task 4: Integrate `oxipage-admin` into `oxipage-console`

**Files:**
- Read: `crates/oxipage-admin/` (all files)
- Modify: `crates/oxipage-console/Cargo.toml`
- Modify: `crates/oxipage-console/src/lib.rs`
- Delete: `crates/oxipage-admin/`
- Modify: `Cargo.toml` (workspace members)
- Modify: `crates/oxipage-cli/Cargo.toml`

- [ ] **Step 1: Read oxipage-admin to understand what to integrate**

```bash
ls crates/oxipage-admin/src/
cat crates/oxipage-admin/src/main.rs
cat crates/oxipage-admin/Cargo.toml
```

- [ ] **Step 2: Find where admin-web SPA bundle lives**

```bash
ls admin-web/dist/ 2>/dev/null || echo "no admin-web/dist"
```

- [ ] **Step 3: Add admin-web to oxipage-console via rust-embed**

In `crates/oxipage-console/Cargo.toml`:
```toml
[dependencies]
oxipage-core = { path = "../oxipage-core" }
# ... existing deps
rust-embed.workspace = true
```

If the admin-web React SPA was previously embedded in `oxipage-server`, the rust-embed setup should already exist. If it was only in `oxipage-admin`, copy the embedding pattern.

- [ ] **Step 4: Move admin's `run_admin()` logic into console**

In `crates/oxipage-console/src/lib.rs`, add a module that integrates the admin SPA serving. Or have console serve admin-web at the `/` route alongside the API.

- [ ] **Step 5: Update Cargo.toml workspace members**

```diff
 members = [
     "crates/oxipage-core",
     "crates/oxipage-console",
     "crates/oxipage-cli",
     "crates/oxipage-wasm",
-    "crates/oxipage-admin",
     "crates/oxipage-ext-activity",
     # ...
```

- [ ] **Step 6: Remove oxipage-admin dependency from CLI**

In `crates/oxipage-cli/Cargo.toml`:
```diff
-oxipage-admin = { version = "0.1.0", path = "../oxipage-admin" }
```

- [ ] **Step 7: Remove oxipage-admin calls from CLI**

```bash
grep -rln "oxipage_admin" crates/oxipage-cli/
```

Replace any `oxipage_admin::run_admin(...)` calls with the equivalent in `oxipage_console`.

- [ ] **Step 8: Delete the oxipage-admin directory**

```bash
git rm -r crates/oxipage-admin/
```

- [ ] **Step 9: Verify**

```bash
cargo check --workspace 2>&1 | head -30
```

- [ ] **Step 10: Commit**

```
git add -A
git commit -m "refactor: integrate oxipage-admin into oxipage-console; remove oxipage-admin crate"
```

---

### Task 5: Rename CLI `serve` command to `console`

**Files:**
- Modify: `crates/oxipage-cli/src/main.rs`
- Modify: `crates/oxipage-cli/src/commands/init_status_serve.rs` (rename file)
- Modify: `crates/oxipage-cli/src/commands/mod.rs`

- [ ] **Step 1: Rename the module file**

```bash
git mv crates/oxipage-cli/src/commands/init_status_serve.rs \
        crates/oxipage-cli/src/commands/init_console.rs
```

- [ ] **Step 2: Update the function `serve` → `console` in init_console.rs**

In `crates/oxipage-cli/src/commands/init_console.rs`:
```diff
-pub(crate) async fn serve(
+pub(crate) async fn console(
     port: Option<u16>,
     preview: bool,
     _config_path: Option<&Path>,
 ) -> anyhow::Result<()> {
```

- [ ] **Step 3: Update doc comments to use "console" terminology**

Change comments like "로컬 개발 서버" to "관리 콘솔".

- [ ] **Step 4: Update commands/mod.rs**

```diff
mod init_status_serve;
+mod init_console;
...
-use init_status_serve::serve;
+use init_console::console;
...
-        Command::Serve { port, preview } => init_status_serve::console(port, preview, ...).await,
+        Command::Console { port, preview } => init_console::console(port, preview, ...).await,
```

Also update `Command::Init { wizard }` arm that called `init_status_serve::serve()`.

- [ ] **Step 5: Update main.rs Command enum**

```diff
-    /// 로컬 개발 서버 기동 (유일하게 HTTP를 거치지 않는 예외)
-    Serve {
+    /// 로컬 관리 콘솔 기동
+    Console {
         #[arg(long)]
         port: Option<u16>,
         /// Preview mode: serve out/ directory as static files
         #[arg(long)]
         preview: bool,
     },
```

- [ ] **Step 6: Verify**

```bash
cargo check -p oxipage --bin oxipage 2>&1 | head -20
```

- [ ] **Step 7: Commit**

```
git add -A
git commit -m "feat(cli): rename 'oxipage serve' to 'oxipage console'"
```

---

### Task 6: Change HTTP route prefix `/api/v1` to `/api/console`

**Files:**
- Modify: `crates/oxipage-core/src/http.rs`

- [ ] **Step 1: Find the route prefix**

```bash
grep -n '/api/v1' crates/oxipage-core/src/http.rs
```

- [ ] **Step 2: Update the route prefix**

```diff
     Router::new()
         .route("/healthz", get(healthz))
-        .nest("/api/v1", api)
+        .nest("/api/console", api)
         .fallback(static_handler)
         .with_state(state)
```

- [ ] **Step 3: Add 301 redirect for `/api/v1/*` → `/api/console/*`**

Add a new handler:
```rust
async fn api_v1_redirect(req: Request) -> Response {
    let new_path = req
        .uri()
        .path()
        .replacen("/api/v1", "/api/console", 1);
    Redirect::permanent(&new_path).into_response()
}
```

Add the route in `build_app`:
```rust
.route("/api/v1/{*path}", axum::routing::any(api_v1_redirect))
.route("/api/v1", axum::routing::any(api_v1_redirect))
```

- [ ] **Step 4: Verify**

```bash
cargo check -p oxipage-core
```

- [ ] **Step 5: Commit**

```
git add crates/oxipage-core/src/http.rs
git commit -m "refactor(http): change API prefix /api/v1 → /api/console with 301 redirect"
```

---

### Task 7: Update admin-web and extensions to use new prefix

**Files:**
- Modify: `admin-web/src/**`
- Modify: extension web code (if any)

- [ ] **Step 1: Find all `/api/v1` references in web code**

```bash
grep -rln "/api/v1" admin-web/ web/ crates/oxipage-ext-*/web/
```

- [ ] **Step 2: Replace with `/api/console`**

For each file, change the fetch path. Use sed:

```bash
# In JS/TS files
find . -type f \( -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) \
  -exec sed -i '' 's|/api/v1|/api/console|g' {} +
```

- [ ] **Step 3: Verify with build**

```bash
cd admin-web && bun run build && cd ..
cd web && bun run build && cd ..
```

- [ ] **Step 4: Commit**

```
git add -A
git commit -m "refactor(web): update fetch paths /api/v1 → /api/console"
```

---

### Task 8: Update all docs (README, doc/, SKILL.md)

**Files:**
- Modify: `README.md`
- Modify: `doc/00-overview.md`, `doc/01-architecture.md`, etc.
- Modify: `.agent/skills/oxipage-cli/SKILL.md`
- Modify: `doc/12-admin-console.md` (rename → `doc/12-console.md`)

- [ ] **Step 1: Update SKILL.md**

Replace:
- `oxipage serve` → `oxipage console`
- `http://127.0.0.1:8787` description "서버" → "콘솔"
- Any example paths `/api/v1/...` → `/api/console/...`

- [ ] **Step 2: Update README.md**

Replace "서버", "server", "oxipage serve" terminology. The v2 section was already updated but check for v1 leftovers.

- [ ] **Step 3: Rename doc/12-admin-console.md → doc/12-console.md**

```bash
git mv doc/12-admin-console.md doc/12-console.md
```

- [ ] **Step 4: Update doc/12-console.md content**

Rewrite or update to reflect that "console" is the new unified name (no separate admin).

- [ ] **Step 5: Update other doc files**

For each file in `doc/`:
- Replace "서버" → "콘솔" where context is local management tool
- Replace "POST /api/v1/..." → "POST /api/console/..."
- Replace "oxipage serve" → "oxipage console"

```bash
# Use sed carefully; manually verify after
find doc/ -type f -name "*.md" -exec sed -i '' 's|oxipage serve|oxipage console|g' {} +
```

- [ ] **Step 6: Commit**

```
git add -A
git commit -m "docs: rename 'server' terminology to 'console' across README, doc/, SKILL.md"
```

---

### Task 9: Update `doc/12-admin-console.md` → `doc/12-console.md` content

**Files:**
- Modify: `doc/12-console.md`

- [ ] **Step 1: Read the current file**

```bash
cat doc/12-console.md
```

- [ ] **Step 2: Update to reflect unified console model**

The doc was written for the separate `oxipage-admin` admin UI. Now that it's unified into `oxipage-console`, rewrite the introduction:

- Remove references to "admin UI" (now part of console)
- Add note about `oxipage console` being the single binary for both API and admin UI
- Update any usage instructions

- [ ] **Step 3: Commit**

```
git add doc/12-console.md
git commit -m "docs(12): rewrite for unified console (server + admin merged)"
```

---

### Task 10: Run full test suite and fix any remaining issues

- [ ] **Step 1: Run tests**

```bash
cargo test --workspace 2>&1 | tail -5
```

If failures, fix:
- Likely some test still calls `run_server()` deprecated function
- Likely some import wasn't updated

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
```

- [ ] **Step 3: Build release**

```bash
cargo build --release
```

- [ ] **Step 4: Verify CLI works**

```bash
./target/release/oxipage console --help
./target/release/oxipage --help
```

Confirm `Console` appears, no `Serve`.

- [ ] **Step 5: Test API redirect**

Start the console locally and curl:
```bash
./target/release/oxipage console &
sleep 2
curl -I http://127.0.0.1:8787/api/v1/healthz 2>&1 | head -3
# Expect: HTTP/1.1 301 ... Location: /api/console/healthz
kill %1
```

- [ ] **Step 6: Commit any fixes**

```
git add -A
git commit -m "fix: address post-rename test/build issues"
```

---

### Task 11: Final verification

- [ ] **Step 1: Confirm no `oxipage-server` references remain (except in deprecation warnings)**

```bash
grep -rln "oxipage-server" --include="*.rs" --include="*.toml" --include="*.md" .
# Should only show:
# - Deprecation aliases in oxipage-console/src/lib.rs
# - Spec doc explaining the rename
```

- [ ] **Step 2: Confirm no `oxipage-admin` directory exists**

```bash
ls crates/oxipage-admin/ 2>&1
# Should error
```

- [ ] **Step 3: Confirm no `oxipage serve` references in active code**

```bash
grep -rn "oxipage serve" --include="*.rs" --include="*.md" .
# Should only show in:
# - doc/05 deployment history
# - Possibly deprecation notes
```

- [ ] **Step 4: Confirm `/api/v1` only in redirect handler**

```bash
grep -rn "/api/v1" --include="*.rs" .
# Should only show redirect handler + tests
```

- [ ] **Step 5: Final commit**

```
git add -A
git commit -m "chore: final console rename verification — no stale server references"
```

---

## Self-Review

1. **Spec coverage:** All 11 design sections mapped to tasks. ✓
2. **No placeholders:** Each step has concrete commands. ✓
3. **Type consistency:** `oxipage_console` crate name consistent throughout. ✓
4. **Test coverage:** Task 10 includes test suite verification. ✓

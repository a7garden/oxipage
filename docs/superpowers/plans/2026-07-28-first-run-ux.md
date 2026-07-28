# First-Run UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `cargo install oxipage` → `oxipage serve` → 브라우저로 6-step 마법사 완료

**Architecture:** Main server embeds `/setup` route + loopback-gated unauthenticated setup API. Admin console stays separate process. First boot detected via `setup_state.setup_completed_at` DB column.

**Tech Stack:** Rust (axum 0.8, sqlx, argon2), React 19 + Vite 7 + React Router

## Global Constraints

- Design doc: `doc/13-first-run-ux.md` (spec for ALL API shapes, field names, step definitions)
- All setup API endpoints: prefix `/api/v1/setup/`, loopback-only gate, return 410 after setup complete
- No new external crate dependencies unless explicitly noted
- Follow existing code patterns (route registration in http.rs, migrations in migrations/core/)

---

### Task P1: setup_state table + setup API 8 endpoints + loopback gate

**Files:**
- Create: `crates/oxipage-core/migrations/core/0006_setup_state.sql`
- Create: `crates/oxipage-core/src/setup.rs` (setup module with all handlers)
- Modify: `crates/oxipage-core/src/migrate.rs` (add Migration to CORE_MIGRATIONS)
- Modify: `crates/oxipage-core/src/http.rs` (add setup routes + loopback gate + api_fallback update)
- Modify: `crates/oxipage-core/src/state.rs` (add site_override field to AppState, add setup_completed helper)
- Modify: `crates/oxipage-server/src/lib.rs` (seed setup_state singleton on startup)

**Interfaces:**
- Consumes: `crates/oxipage-core/src/state.rs` AppState, `crates/oxipage-core/src/auth.rs` create_pat
- Produces: `/api/v1/setup/*` 8 endpoints, `AppState.site_override`, `AppState.is_setup_mode()`

**API endpoints (spec: doc/13-first-run-ux.md §13.5.2):**
- GET /api/v1/setup/status → `{setup_mode, completed_steps[], available_extensions[], available_themes[]}`
- POST /api/v1/setup/site → `{name, base_url}` → writes TOML + updates site_override
- POST /api/v1/setup/admin → `{password}` → argon2id hash → setup_state.admin_password_hash
- POST /api/v1/setup/extensions → `{enabled: ["blog", ...]}` → extension_state.enabled
- POST /api/v1/setup/profile → `{display_name, tagline_ko, ...}` → profile UPDATE
- POST /api/v1/setup/theme → `{theme_id, lobby_mode}` → theme_config + lobby_config
- POST /api/v1/setup/content → `{sample_post, tmdb_key, aladin_key}` → blog_post + extension_state.config
- POST /api/v1/setup/complete → `{}` → setup_completed_at + PAT 생성 + credentials 저장 → `{token, token_label}`

**Migration SQL (0006_setup_state.sql):**
```sql
CREATE TABLE IF NOT EXISTS setup_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    setup_completed_at TEXT,
    admin_password_hash TEXT,
    site_name TEXT,
    base_url TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT OR IGNORE INTO setup_state (id) VALUES (1);
```

- [ ] **Step 1: Add migration 0006_setup_state.sql**
- [ ] **Step 2: Register migration in CORE_MIGRATIONS** (crates/oxipage-core/src/migrate.rs)
- [ ] **Step 3: Add `site_override` field to AppState** (state.rs): `pub site_override: Arc<tokio::sync::RwLock<Option<SiteOverride>>>` where `SiteOverride { name: String, base_url: String }`
- [ ] **Step 4: Add `setup_completed()` helper to AppState**: checks `setup_state.setup_completed_at IS NOT NULL`
- [ ] **Step 5: Create setup.rs module** with 8 handler functions
  - `setup_status_handler` — reads setup_state + registry + themes
  - `setup_site_handler` — writes TOML + updates site_override + profile.display_name
  - `setup_admin_handler` — validates password ≥8 chars, argon2id hash, stores in setup_state
  - `setup_extensions_handler` — batch update extension_state.enabled
  - `setup_profile_handler` — writes to profile table via existing profile repo
  - `setup_theme_handler` — updates theme_config + batch lobby_config
  - `setup_content_handler` — creates sample blog post + stores API keys in extension_state.config
  - `setup_complete_handler` — writes setup_completed_at, creates PAT, writes credentials, returns token
- [ ] **Step 6: Add loopback gate middleware** `setup_gate` — returns 403 for non-loopback
- [ ] **Step 7: Register setup routes in http.rs** — `.route("/setup/status", get(...))` etc. BEFORE extension nest, with loopback gate layer
- [ ] **Step 8: Wire start-up in oxipage-server/src/lib.rs** — after migrations, ensure setup_state singleton exists
- [ ] **Step 9: Wire lobby_manifest to check site_override** — reads `state.site_override.read().await` first, falls back to `config.site`
- [ ] **Step 10: Unit test** — test setup API flow (loopback mock, 403, 410, happy path)
- [ ] **Step 11: Commit**

### Task P2: Wizard UI 6-step (web/ SPA)

**Files:**
- Create: `web/src/setup/SetupWizard.tsx` (wizard container)
- Create: `web/src/setup/StepSite.tsx`
- Create: `web/src/setup/StepAdmin.tsx`
- Create: `web/src/setup/StepExtensions.tsx`
- Create: `web/src/setup/StepProfile.tsx`
- Create: `web/src/setup/StepTheme.tsx`
- Create: `web/src/setup/StepContent.tsx`
- Create: `web/src/setup/StepDone.tsx`
- Create: `web/src/setup/api.ts` (setup API client)
- Create: `web/src/setup/SetupGuard.tsx` (redirect logic)
- Modify: `web/src/App.tsx` (add /setup/* route + SetupGuard)
- Modify: `web/src/shared/api.ts` (add fetchSetupStatus)

**Interfaces:**
- Consumes: `/api/v1/setup/*` endpoints (from P1)
- Produces: React components for 6-step wizard

**Step details (spec: doc/13-first-run-ux.md §13.7):**
- Step 1: site name + default language (beautiful centered card, step progress bar)
- Step 2: password + confirm (≥8 char validation)
- Step 3: extension cards (toggle on/off, preset buttons)
- Step 4: profile form (pre-filled display_name, optional fields)
- Step 5: theme cards + layout cards (visual selection with preview colors)
- Step 6: sample post checkbox + API key inputs (all optional)
- Done: token display + clipboard + "사이트 보기" / "관리 콘솔" buttons

- [ ] **Step 1: Create setup API client** (web/src/setup/api.ts) — 8 functions matching 8 endpoints
- [ ] **Step 2: Create SetupGuard** — fetches setup status on mount, redirects accordingly
- [ ] **Step 3: Create step progress bar component**
- [ ] **Step 4: Create StepSite** — site name input + language selector
- [ ] **Step 5: Create StepAdmin** — password + confirm with validation
- [ ] **Step 6: Create StepExtensions** — toggle cards with presets
- [ ] **Step 7: Create StepProfile** — form with pre-fill + skip
- [ ] **Step 8: Create StepTheme** — theme cards + layout cards
- [ ] **Step 9: Create StepContent** — sample post checkbox + API key inputs
- [ ] **Step 10: Create StepDone** — token display + clipboard + navigation buttons
- [ ] **Step 11: Create SetupWizard** — orchestrates 6 steps with next/back navigation
- [ ] **Step 12: Wire into App.tsx** — add `/setup/*` route, add SetupGuard to MainApp
- [ ] **Step 13: Add fetchSetupStatus to shared/api.ts**
- [ ] **Step 14: Build verify** — `cd web && bun run build` passes
- [ ] **Step 15: Commit**

### Task P3: serve first boot detection + browser auto-open + open command

**Files:**
- Modify: `crates/oxipage-server/src/lib.rs` (first boot detection + browser open)
- Modify: `crates/oxipage-cli/src/main.rs` (add `open` subcommand)
- Modify: `crates/oxipage-cli/src/commands/mod.rs` (register Open command)
- Create: `crates/oxipage-cli/src/commands/open.rs` (open implementation)
- Modify: `crates/oxipage-cli/src/commands/init_status_serve.rs` (serve: auto-open on first boot, init --wizard)

**Interfaces:**
- Consumes: `setup_state.setup_completed_at` (from P1)
- Produces: `oxipage open` CLI command, `oxipage serve` auto-open, `oxipage init --wizard`

- [ ] **Step 1: Add open_browser() helper** in oxipage-server (platform-specific `open`/`start`/`xdg-open`)
- [ ] **Step 2: First boot detection in run_server** — after setup_state migration, check `setup_completed_at`. If NULL → print URL + call open_browser
- [ ] **Step 3: Create open.rs** command implementation — reads TOML/sites.toml for URL, calls open_browser
- [ ] **Step 4: Register `open` subcommand** in main.rs and commands/mod.rs
- [ ] **Step 5: Add `init --wizard`** — calls init + serve with auto-open
- [ ] **Step 6: Commit**
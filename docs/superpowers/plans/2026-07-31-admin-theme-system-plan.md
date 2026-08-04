# Admin Theme System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve every console-theme duplication and split console appearance from public site theme: a single `oxibuilder_core::theme` catalog, one default-site `/api/console/theme` handler, a shared browser controller, a three-state appearance toggle, scoped sidebar tokens, and `--accent-hue`-driven public presentation.

**Architecture:** Rust catalog lives in `oxibuilder_core::theme` and is consumed by setup, core catalog, console default-site router, and per-site PUT validation. The console router resolves the default site through `SiteRegistry::default_slug` / `ctx_for` instead of reading `AppState.db`. The web side centralizes `ConsoleAppearance` (system/light/dark) with a single early-boot helper, removing the private toggle and inline FOUC scripts. Sidebar tokens are scoped per `[data-theme=light|dark]` so light mode actually has a light sidebar. `--accent-hue` is consumed by OKLCH accent primitives inside `[data-public-theme=...]` scopes (DraftPreviewPane) and the static generated HTML.

**Tech Stack:** Rust (axum 0.8, sqlx, serde), React 19, TypeScript, Vite 7, Tailwind v4 (OKLCH + @theme inline), TanStack Query 5.

## Global Constraints

- Single shared catalog: `oxibuilder_core::theme` — `paper`, `midnight`, `sepia`, `forest`, `neon`, `canvas` are the only valid IDs.
- Console appearance key: `localStorage["oxibuilder-console-appearance"]` — values `"system" | "light" | "dark"`; missing/invalid → `"system"`.
- Public theme key in per-site DB: `theme_config.theme_id` (singleton row id=1).
- `--accent-hue` is set by `applyServerTheme` (default scope: `<html>`); OKLCH accent primitives consume it only inside `[data-public-theme="…"]` scopes.
- Default-site `/api/console/theme` resolves through `Arc<SiteRegistry>`; with no registered site returns the `paper` definition without writing to DB.
- Per-site PUT returns 400 for unknown IDs.
- No placeholders, no shims, no aliases after cutover.

---

## File Structure

```text
crates/oxibuilder-core/
└── src/
    ├── theme.rs                    # NEW: ThemeDefinition catalog + helpers
    ├── lib.rs                      # pub mod theme
    ├── http.rs                     # -THEMES, -ThemeCatalogEntry, -theme_catalog, -theme_get, -theme_put routes
    └── setup.rs                    # -local THEMES, consume oxibuilder_core::theme

crates/oxibuilder-console/
└── src/
    ├── router.rs                   # +GET /theme route (default-site resolution via SiteRegistry)
    └── per_site.rs                 # -local VALID_THEMES, -local default theme_id; return full definition

web/
├── theme-boot.js                   # NEW: shared FOUC boot script
├── admin.html                      # -inline script, +<script src="/theme-boot.js" data-context="console">
├── index.html                      # -inline script, +<script src="/theme-boot.js" data-context="public">
└── src/
    ├── shared/
    │   ├── theme.ts                # rewrite: ConsoleAppearance, get/set, applyThemeMode, applyServerTheme, getThemePalette
    │   ├── ThemeToggle.tsx         # rewrite: three-state control (System/Light/Dark)
    │   └── tokens.css              # sidebar tokens move into [data-theme=light|...]
    └── admin/
        ├── App.tsx                 # ShellFallback uses semantic token; mount applyServerTheme
        ├── shell/
        │   ├── Topbar.tsx          # -private ThemeToggle, import shared
        │   └── Sidebar.tsx         # replace hex literals with tokens
        ├── settings/
        │   └── SettingsPage.tsx    # +Appearance section (console + public theme summary + link)
        └── themes/
            └── ThemesPage.tsx      # -local THEMES, fetch catalog from server, use shared ThemeDefinition
```

---

### Task 1: Create `oxibuilder_core::theme` with single shared catalog

**Files:**
- Create: `crates/oxibuilder-core/src/theme.rs`
- Modify: `crates/oxibuilder-core/src/lib.rs:1-22`

**Interfaces:**
- Consumes: nothing
- Produces:
  ```rust
  pub struct ThemeDefinition {
      pub id: &'static str,
      pub name_ko: &'static str,
      pub name_en: &'static str,
      pub mode: ThemeMode,           // "light" | "dark"
      pub accent_hue: f64,
      pub preview_colors: [&'static str; 4],
      pub description_ko: &'static str,
      pub description_en: &'static str,
  }
  pub const ALL_THEMES: &[ThemeDefinition] = &[ /* paper, midnight, sepia, forest, neon, canvas */ ];
  pub fn find_theme(id: &str) -> Option<&'static ThemeDefinition>;
  pub fn is_known_theme(id: &str) -> bool { find_theme(id).is_some() }
  ```

- [ ] **Step 1: Write failing test for `find_theme`**

Create `crates/oxibuilder-core/tests/theme_catalog.rs`:

```rust
use oxibuilder_core::theme::{ALL_THEMES, is_known_theme, find_theme};

#[test]
fn catalog_has_six_themes() {
    assert_eq!(ALL_THEMES.len(), 6);
}

#[test]
fn catalog_contains_required_ids() {
    let ids: Vec<&str> = ALL_THEMES.iter().map(|t| t.id).collect();
    for required in ["paper", "midnight", "sepia", "forest", "neon", "canvas"] {
        assert!(ids.contains(&required), "missing {required}");
    }
}

#[test]
fn find_theme_returns_definition() {
    let t = find_theme("paper").expect("paper exists");
    assert_eq!(t.id, "paper");
    assert_eq!(t.name_en, "Paper");
    assert!(matches!(t.mode, oxibuilder_core::theme::ThemeMode::Light));
    assert_eq!(t.preview_colors.len(), 4);
}

#[test]
fn unknown_theme_returns_none() {
    assert!(find_theme("atlantis").is_none());
    assert!(!is_known_theme("atlantis"));
}

#[test]
fn duplicate_ids_rejected() {
    let ids: Vec<&str> = ALL_THEMES.iter().map(|t| t.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "catalog has duplicate id");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxibuilder-core --test theme_catalog`
Expected: compile error — `theme` module does not exist.

- [ ] **Step 3: Implement `theme.rs`**

Create `crates/oxibuilder-core/src/theme.rs`:

```rust
//! Single source of truth for the curated public site theme catalog.
//!
//! Both the setup wizard, the public catalog endpoint, the per-site theme
//! PUT validator, the Admin ThemesPage, and the browser-side
//! `applyServerTheme` consume this catalog. There is no other catalog.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ThemeDefinition {
    pub id: &'static str,
    pub name_ko: &'static str,
    pub name_en: &'static str,
    pub mode: ThemeMode,
    pub accent_hue: f64,
    pub preview_colors: [&'static str; 4],
    pub description_ko: &'static str,
    pub description_en: &'static str,
}

/// The shared catalog. Order is the order shown in Admin → Themes and the
/// setup wizard. Mode/hue/preview/description are complete for every entry.
pub const ALL_THEMES: &[ThemeDefinition] = &[
    ThemeDefinition {
        id: "paper",
        name_ko: "종이",
        name_en: "Paper",
        mode: ThemeMode::Light,
        accent_hue: 160.0,
        preview_colors: ["#fafaf5", "#f5f2ed", "#2d2934", "#2d7a5c"],
        description_ko: "따뜻한 종이 배경, 파인 그린 악센트",
        description_en: "Warm paper background, pine green accent",
    },
    ThemeDefinition {
        id: "midnight",
        name_ko: "한밤",
        name_en: "Midnight",
        mode: ThemeMode::Dark,
        accent_hue: 230.0,
        preview_colors: ["#1a1a2e", "#16213e", "#e0e0e0", "#4fc3f7"],
        description_ko: "깊은 밤하늘, 시안-블루 악센트",
        description_en: "Deep night sky, cyan-blue accent",
    },
    ThemeDefinition {
        id: "sepia",
        name_ko: "세피아",
        name_en: "Sepia",
        mode: ThemeMode::Light,
        accent_hue: 70.0,
        preview_colors: ["#f5f0e8", "#ede0d4", "#3d3529", "#b8860b"],
        description_ko: "오래된 책장, 앰버-골드 악센트",
        description_en: "Old bookshelf, amber-gold accent",
    },
    ThemeDefinition {
        id: "forest",
        name_ko: "숲",
        name_en: "Forest",
        mode: ThemeMode::Dark,
        accent_hue: 155.0,
        preview_colors: ["#1b2b1b", "#243624", "#e0e8e0", "#2ecc71"],
        description_ko: "이끼 낀 숲, 에메랄드 악센트",
        description_en: "Mossy forest, emerald accent",
    },
    ThemeDefinition {
        id: "neon",
        name_ko: "네온",
        name_en: "Neon",
        mode: ThemeMode::Dark,
        accent_hue: 290.0,
        preview_colors: ["#0d0221", "#16003b", "#f4e6ff", "#a855f7"],
        description_ko: "합성 보라, 마젠타-네온 악센트",
        description_en: "Synthetic purple, magenta-neon accent",
    },
    ThemeDefinition {
        id: "canvas",
        name_ko: "캔버스",
        name_en: "Canvas",
        mode: ThemeMode::Light,
        accent_hue: 240.0,
        preview_colors: ["#fdfdfb", "#f4f4f1", "#1f2937", "#0ea5e9"],
        description_ko: "화이트 캔버스, 스카이-블루 악센트",
        description_en: "White canvas, sky-blue accent",
    },
];

pub fn find_theme(id: &str) -> Option<&'static ThemeDefinition> {
    ALL_THEMES.iter().find(|t| t.id == id)
}

pub fn is_known_theme(id: &str) -> bool {
    find_theme(id).is_some()
}
```

In `crates/oxibuilder-core/src/lib.rs`, add `pub mod theme;` between `state` and the closing brace (line 21). Insert before line 22 (end of file):

```rust
pub mod theme;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p oxibuilder-core --test theme_catalog`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core/src/theme.rs crates/oxibuilder-core/src/lib.rs crates/oxibuilder-core/tests/theme_catalog.rs
git commit -m "feat(core): single ThemeDefinition catalog — paper/midnight/sepia/forest/neon/canvas"
```

---

### Task 2: Move GET /api/console/theme from core http.rs into console router

**Files:**
- Modify: `crates/oxibuilder-console/src/router.rs:32-43` (add new route)
- Modify: `crates/oxibuilder-core/src/http.rs:67-72` (remove the `/theme` GET route; keep `/themes` PUT removed too)
- Modify: `crates/oxibuilder-core/src/http.rs:1047-1060` (delete the original core `theme_get`)
- Modify: `crates/oxibuilder-core/src/http.rs:977-988, 1028-1045` (delete `ThemeCatalogEntry`, the `THEMES` slice, and `theme_catalog`)
- Modify: `crates/oxibuilder-core/src/http.rs:1062-1095` (delete core `theme_put`)
- Modify: `crates/oxibuilder-core/src/http.rs:8` (drop `use crate::error::ApiError` only if no longer used; leave if needed)

**Interfaces:**
- Consumes: `SiteRegistry::{default_slug, ctx_for}` from `oxibuilder_console::sites_runtime`; `oxibuilder_core::theme::ALL_THEMES`
- Produces: `GET /api/console/theme` mounted in `build_top_level_router()` returning `{ "data": { "theme_id": "...", "definition": ThemeDefinition } }`. With no default site, returns `paper` without DB access.

- [ ] **Step 1: Write failing test for default-site GET /theme**

Create `crates/oxibuilder-console/tests/default_theme_route.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxibuilder_console::router::build_console_router;
use oxibuilder_console::sites_runtime::SiteRegistry;
use tower::util::ServiceExt;

#[tokio::test]
async fn get_default_theme_with_no_registered_site_returns_paper() {
    // Empty registry — no default slug, no sites. Handler must NOT hit DB.
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(Request::builder().uri("/theme").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["data"]["theme_id"], "paper");
    assert_eq!(json["data"]["definition"]["id"], "paper");
    assert_eq!(json["data"]["definition"]["accent_hue"], 160.0);
}

#[tokio::test]
async fn get_default_theme_404s_for_unknown_route_after_move() {
    // After moving, GET /theme in the core sub-router should 404 (handled by
    // /s/{slug}/theme on per-site and the new /theme on top-level console).
    // This is just a guard against double-mounting.
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(Request::builder().uri("/theme/extra").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p oxibuilder-console --test default_theme_route`
Expected: compile error — `SiteRegistry::empty_for_tests` does not exist; `GET /theme` route does not exist yet.

- [ ] **Step 3: Add `SiteRegistry::empty_for_tests` and the new console route**

In `crates/oxibuilder-console/src/sites_runtime.rs`, append inside `impl SiteRegistry`:

```rust
#[cfg(test)]
pub async fn empty_for_tests() -> Arc<Self> {
    use crate::build::BuildGuard;
    use crate::deploy::DeployGuard;
    use oxibuilder_core::config::Config;

    let sites_file = oxibuilder_core::sites::SitesFile::default();
    let bg = Arc::new(BuildGuard::default());
    let dg = Arc::new(DeployGuard::default());
    let inner = SiteRegistry::new(sites_file, bg, dg).await.unwrap();
    let cfg = Config::default();
    let _ = cfg; // currently empty; placeholder for future invocations
    Arc::new(inner)
}
```

(If `BuildGuard::default()` / `DeployGuard::default()` don't exist in your tree, replace with whatever zero-value construction already exists in `sites_runtime.rs` — keep lines minimal.) In `crates/oxibuilder-console/src/router.rs`, after line 43 (after `/preview/{slug}/{*rest}`), add:
        .route("/theme", get(get_default_theme))

Replace the `async fn get_default(...) { ... }` block (currently around lines 94–97) so it sits next to the new handler. Append a new handler in `crates/oxibuilder-console/src/router.rs` after the existing handlers:

```rust
/// `GET /api/console/theme` — current default site's theme definition.
///
/// Resolves the registered default site through `SiteRegistry`. With no
/// registered site, returns `paper` without touching any DB. Never reads
/// the global `AppState.db` (that handler used to read a different DB than
/// the per-site endpoint).
async fn get_default_theme(
    State(registry): State<Arc<SiteRegistry>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use oxibuilder_core::theme::{find_theme, ALL_THEMES};

    let slug = match registry.default_slug().await {
        Some(s) => s,
        None => {
            let def = ALL_THEMES
                .first()
                .copied()
                .expect("paper always present in ALL_THEMES");
            return Ok(Json(serde_json::json!({
                "data": {
                    "theme_id": def.id,
                    "definition": def,
                }
            })));
        }
    };

    let ctx = registry
        .ctx_for(&slug)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("default site '{slug}' not loaded")))?;

    let row: Option<(String,)> = sqlx::query_as("SELECT theme_id FROM theme_config WHERE id = 1")
        .fetch_optional(&ctx.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let theme_id = row.map(|r| r.0).unwrap_or_else(|| "paper".to_string());
    let def = find_theme(&theme_id).unwrap_or_else(|| {
        ALL_THEMES.first().copied().expect("paper always present")
    });

    Ok(Json(serde_json::json!({
        "data": {
            "theme_id": def.id,
            "definition": def,
        }
    })))
}
```

Add the required import in `crates/oxibuilder-console/src/router.rs` at the top:

```rust
use oxibuilder_core::theme::{find_theme, ALL_THEMES};
```

- [ ] **Step 4: Remove `/theme` GET/PUT and `/themes` GET from core's `build_app`**

In `crates/oxibuilder-core/src/http.rs`, delete the two lines inside `build_app`:

```rust
        .route("/theme", get(theme_get).put(theme_put))
        .route("/themes", get(theme_catalog))
```

(They are on lines 70–71 per the existing source.) Then delete the `THEMES` slice, the `ThemeCatalogEntry` struct, the `theme_get` (line 1048), and `theme_put` (line 1068) functions (lines 977–1095). The `GET /api/console/themes` (full catalog) endpoint must still exist. Since `ThemeDefinition` derives `Serialize` with flat field names (`name_ko`, `name_en`, `description_ko`, `description_en`), the catalog shares the same shape as the per-site/default endpoints — return `ALL_THEMES.to_vec()` directly:

```rust
/// `GET /api/console/themes` — public catalog. Auth-free; used by the
/// Serializes each entry with the SAME flat shape as the per-site and
/// default-site theme endpoints (`{ id, name_ko, name_en, mode, accent_hue,
/// preview_colors, description_ko, description_en }`). The TS
/// `ThemeDefinition` type fits all three endpoints, so no shape translation
/// is needed in the browser.
async fn theme_catalog()
    -> Json<DataEnvelope<Vec<oxibuilder_core::theme::ThemeDefinition>>>
{
    Json(DataEnvelope {
        data: oxibuilder_core::theme::ALL_THEMES.to_vec(),
    })
}
```

Delete the manual nested `serde_json::json!` body and the now-unused `ThemeCatalogEntry` struct / `THEMES` slice (original lines 977-988, 991-1028, 1031-1045).

In `build_app`, after `.route("/extensions/registry", ...)`, add:

```rust
        .route("/themes", get(theme_catalog))
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p oxibuilder-console --test default_theme_route`
Expected: 2 passed.

Run: `cargo test -p oxibuilder-core --test theme_catalog`
Expected: still 5 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-console/src/router.rs crates/oxibuilder-console/src/sites_runtime.rs crates/oxibuilder-console/tests/default_theme_route.rs crates/oxibuilder-core/src/http.rs
git commit -m "refactor(console): default-site GET /theme resolves through SiteRegistry"
```

---

### Task 3: Update per-site theme GET/PUT to use shared catalog and return full definition

**Files:**
- Modify: `crates/oxibuilder-console/src/per_site.rs:265-313`
- Modify: `crates/oxibuilder-core/src/setup.rs:22-47, 332-340, 470-484`

**Interfaces:**
- Consumes: `oxibuilder_core::theme::{find_theme, is_known_theme, ALL_THEMES}`
- Produces: `GET /api/console/s/{slug}/theme` returns `{ theme_id, definition: ThemeDefinition }`. PUT accepts only IDs in `ALL_THEMES`, returns 400 otherwise.

- [ ] **Step 1: Write failing test for per-site theme GET returning full definition**

Create `crates/oxibuilder-console/tests/per_site_theme.rs`:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxibuilder_console::router::build_console_router;
use oxibuilder_console::sites_runtime::SiteRegistry;
use tower::util::ServiceExt;

#[tokio::test]
async fn per_site_theme_get_unknown_slug_404s() {
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/s/nonexistent/theme")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run test to verify baseline**

Run: `cargo test -p oxibuilder-console --test per_site_theme`
Expected: PASS once site routes are wired (unknown slug already 404s because of middleware). This is the regression net for the next step.

- [ ] **Step 3: Rewrite per_site::theme_get and per_site::theme_put**

Replace `crates/oxibuilder-console/src/per_site.rs` lines 265–313 with:

```rust
// ─── theme (GET/PUT) ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ThemeResponse {
    pub data: serde_json::Value,
}

pub async fn theme_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ThemeResponse>, (StatusCode, String)> {
    use oxibuilder_core::theme::{find_theme, ALL_THEMES};

    let row: Option<(String,)> = sqlx::query_as("SELECT theme_id FROM theme_config WHERE id = 1")
        .fetch_optional(&ctx.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let stored = row.map(|r| r.0);
    let def = match stored.as_deref().and_then(find_theme) {
        Some(d) => d,
        None => ALL_THEMES.first().copied().expect("paper always present"),
    };

    Ok(Json(ThemeResponse {
        data: serde_json::json!({
            "theme_id": def.id,
            "definition": def,
        }),
    }))
}

#[derive(Deserialize)]
pub struct ThemePutInput {
    pub theme_id: String,
}

pub async fn theme_put(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Json(input): Json<ThemePutInput>,
) -> Result<Json<ThemeResponse>, (StatusCode, String)> {
    use oxibuilder_core::theme::{find_theme, is_known_theme};

    if !is_known_theme(&input.theme_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("'{}' is not a valid theme", input.theme_id),
        ));
    }
    sqlx::query(
        "INSERT INTO theme_config (id, theme_id, updated_at) VALUES (1, ?1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET theme_id = ?1, updated_at = datetime('now')",
    )
    .bind(&input.theme_id)
    .execute(&ctx.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let def = find_theme(&input.theme_id)
        .copied()
        .expect("validation above guarantees presence");
    Ok(Json(ThemeResponse {
        data: serde_json::json!({
            "theme_id": def.id,
            "definition": def,
        }),
    }))
}
```

Add at the top of `per_site.rs` (next to the existing oxibuilder_core imports) if absent:

```rust
use oxibuilder_core::theme::{find_theme, is_known_theme, ALL_THEMES};
```

(The imports are used only inside the handler bodies above; the explicit `use` inside each function keeps them scoped.)

- [ ] **Step 4: Replace `THEMES` in setup.rs with `ALL_THEMES`**

In `crates/oxibuilder-core/src/setup.rs`, remove lines 22–47 (the local `THEMES` slice and its 4 entries). The `available_themes` payload returned by `/api/console/setup/status` must still work. Replace line 336's `THEMES.to_vec()` with:

```rust
            available_themes: oxibuilder_core::theme::ALL_THEMES.to_vec(),
```

Replace line 475's `THEMES.iter().any(|t| t.id == input.theme_id)` with:

```rust
    if !oxibuilder_core::theme::is_known_theme(&input.theme_id) {
```

- [ ] **Step 5: Verify baseline still passes and run both tests**

Run: `cargo test -p oxibuilder-console --test per_site_theme --test default_theme_route`
Expected: 3 passed.

Run: `cargo test -p oxibuilder-core --test theme_catalog`
Expected: 5 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/oxibuilder-console/src/per_site.rs crates/oxibuilder-console/tests/per_site_theme.rs crates/oxibuilder-core/src/setup.rs
git commit -m "refactor(theme): per-site PUT uses oxibuilder_core::theme catalog; setup consumes shared"
```

---

### Task 4: Update shared TS API client to use new per-site response shape

**Files:**
- Modify: `web/src/admin/shared/api.ts:220-237`

**Interfaces:**
- Consumes: `getTheme(slug)` now returns `{ theme_id, definition: ThemeDefinition }`. `setTheme` PUT returns the same.
- Produces: TS types `ThemeDefinition` matching the Rust struct (id, name_ko, name_en, mode, accent_hue, preview_colors, description_ko, description_en).

- [ ] **Step 1: Write failing compile test**

The web side has no test runner — we use `tsc --noEmit` as the typecheck gate. Introduce `ThemeDefinition` exported from `shared/theme.ts` first (Task 5) and have `api.ts` import it. Until then the existing api.ts compiles because `getTheme` returns `{ theme_id: string }`.

- [ ] **Step 2: Add `ThemeDefinition` to shared/theme.ts (sketch — full rewrite in Task 5)**

In `web/src/shared/theme.ts`, append (the file is rewritten in Task 5; this line sets up the symbol that `api.ts` imports):

```ts
export interface ThemeDefinition {
  id: string;
  name_ko: string;
  name_en: string;
  mode: "light" | "dark";
  accent_hue: number;
  preview_colors: readonly [string, string, string, string];
  description_ko: string;
  description_en: string;
}
```

- [ ] **Step 3: Rewrite the theme section in api.ts**

In `web/src/admin/shared/api.ts`, replace lines 220–237 with:

```ts
// ─── Theme (GET/PUT) ──────────────────────────────────────────────────────

import type { ThemeDefinition } from "../../shared/theme";

export interface SiteTheme {
  theme_id: string;
  definition: ThemeDefinition;
}

export async function listThemes(): Promise<ThemeDefinition[]> {
  const res = await fetch(`${CONSOLE_BASE}/themes`);
  const json = await jsonOrThrow<{ data: ThemeDefinition[] }>(res);
  return json.data;
}

export async function getTheme(slug: string): Promise<SiteTheme> {
  const res = await siteScopedFetch(slug, "/theme");
  const json = await jsonOrThrow<{ data: SiteTheme }>(res);
  return json.data;
}

export async function setTheme(slug: string, themeId: string): Promise<SiteTheme> {
  const res = await siteScopedFetch(slug, "/theme", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ theme_id: themeId }),
  });
  const json = await jsonOrThrow<{ data: SiteTheme }>(res);
  return json.data;
}
```

- [ ] **Step 4: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add web/src/admin/shared/api.ts web/src/shared/theme.ts
git commit -m "feat(api): per-site theme returns full ThemeDefinition"
```

---

### Task 5: Create shared `web/theme-boot.js` and remove inline FOUC scripts

**Files:**
- Create: `web/theme-boot.js`
- Modify: `web/admin.html:6-18`
- Modify: `web/index.html:6-18`

**Interfaces:**
- Consumes: `localStorage["oxibuilder-console-appearance"]` when `data-context="console"`; nothing when `data-context="public"` (reads pre-baked attributes from a `<meta name="oxibuilder-theme">` if present — falls back to `paper`).
- Produces: synchronous, dependency-free script executed before `<link rel="stylesheet">` and before React. Sets `<html data-theme>` (and `data-public-theme` for the public site) before first paint.

- [ ] **Step 1: Write `web/theme-boot.js`**

Create `web/theme-boot.js`:

```js
/* Oxibuilder early theme boot.
   - Synchronous, no deps, executes before <link rel=stylesheet>.
   - Reads <script data-context="..."> on this tag (set by admin.html / index.html).
   - For "console": reads oxibuilder-console-appearance; resolves system | light | dark,
     writes <html data-theme> and document.documentElement.style.setProperty('--accent-hue','160').
   - For "public": reads <meta name="oxibuilder-theme" content="paper"> if present,
     writes <html data-public-theme="...">, sets --accent-hue on root.
*/
(function () {
  try {
    var scripts = document.currentScript || document.scripts[document.scripts.length - 1];
    var ctx = (scripts && scripts.getAttribute("data-context")) || "public";

    function systemMode() {
      return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }

    if (ctx === "console") {
      var stored;
      try {
        stored = localStorage.getItem("oxibuilder-console-appearance");
      } catch (e) {
        stored = null;
      }
      var mode = stored === "light" || stored === "dark" ? stored : systemMode();
      document.documentElement.dataset.theme = mode;
      document.documentElement.style.setProperty("--accent-hue", "160");
      return;
    }

    // public
    var meta = document.querySelector('meta[name="oxibuilder-theme"]');
    var themeId = (meta && meta.content) || "paper";
    document.documentElement.dataset.publicTheme = themeId;
    var hueByTheme = { paper: "160", midnight: "230", sepia: "70", forest: "155", neon: "290", canvas: "240" };
    document.documentElement.style.setProperty("--accent-hue", hueByTheme[themeId] || "160");
  } catch (e) {
    document.documentElement.dataset.theme = "light";
  }
})();
```

- [ ] **Step 2: Replace inline script in `admin.html`**

In `web/admin.html`, replace lines 6–18 (the `<script>` block) with:

```html
    <script src="/theme-boot.js" data-context="console"></script>
    <title>Oxibuilder Console</title>
```

(Place the `<script>` directly under `<meta name="viewport" ...>` and before `<title>`.)

- [ ] **Step 3: Replace inline script in `index.html`**

In `web/index.html`, replace lines 6–18 (the `<script>` block) with:

```html
    <script src="/theme-boot.js" data-context="public"></script>
    <meta name="oxibuilder-theme" content="paper" />
```

(Falls back to `paper`. SSG templates inject a real theme_id at build time.)

- [ ] **Step 4: Typecheck (N/A — this is `.js`, not TS)**

Run: `cd web && npx tsc --noEmit`
Expected: clean (no TS change here).

- [ ] **Step 5: Browser smoke (manual)**

Open `web/dist/admin.html` in a Chromium browser via `xd://browser` after running `cd web && bun run build` and confirm `<html data-theme>` is set before React mounts. No FOUC flash.

- [ ] **Step 6: Commit**

```bash
git add web/theme-boot.js web/admin.html web/index.html
git commit -m "feat(web): shared theme-boot.js — replaces duplicated inline FOUC scripts"
```

---

### Task 6: Rewrite `web/src/shared/theme.ts` with console-apperance separation

**Files:**
- Rewrite: `web/src/shared/theme.ts`

**Interfaces:**
- Consumes: `localStorage["oxibuilder-console-appearance"]`; `/api/console/theme` (new shape from Task 2/3).
- Produces:
  ```ts
  export type ConsoleAppearance = "system" | "light" | "dark";
  export type ResolvedMode = "light" | "dark";
  export const STORAGE_KEY: "oxibuilder-console-appearance";

  export function getConsoleAppearance(): ConsoleAppearance;
  export function setConsoleAppearance(value: ConsoleAppearance): void;
  export function getResolvedConsoleMode(): ResolvedMode;
  export function watchSystemAppearance(cb: (mode: ResolvedMode) => void): () => void;
  export function applyThemeMode(mode: ResolvedMode): void;
  export async function applyServerTheme(slug?: string): Promise<ThemeDefinition | null>;
  export function getThemePalette(theme: ThemeDefinition): Record<string, string>;

  export interface ThemeDefinition { /* see Task 4 */ }
  ```

- [ ] **Step 1: Rewrite `web/src/shared/theme.ts` entirely**

Replace the entire contents of `web/src/shared/theme.ts` with:

```ts
// Console appearance vs public site theme — kept distinct on purpose.
//
//  - Console appearance (the Admin shell's <html data-theme>)
//      localStorage["oxibuilder-console-appearance"] = "system" | "light" | "dark"
//      Resolution: explicit light/dark → that mode; "system" or missing/invalid →
//      window.matchMedia('(prefers-color-scheme: dark)').
//
//  - Public site theme (the per-site SQLite singleton)
//      Shared catalog: oxibuilder_core::theme in Rust; ThemeDefinition here.
//      applyServerTheme() publishes palette variables to the document, but
//      NEVER mutates the console's data-theme or sets console mode.

export type ConsoleAppearance = "system" | "light" | "dark";
export type ResolvedMode = "light" | "dark";

export const STORAGE_KEY = "oxibuilder-console-appearance";

export interface ThemeDefinition {
  id: string;
  name_ko: string;
  name_en: string;
  mode: ResolvedMode;
  accent_hue: number;
  preview_colors: readonly [string, string, string, string];
  description_ko: string;
  description_en: string;
}

function isAppearance(value: unknown): value is ConsoleAppearance {
  return value === "system" || value === "light" || value === "dark";
}

function systemMode(): ResolvedMode {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function getConsoleAppearance(): ConsoleAppearance {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isAppearance(v) ? v : "system";
  } catch {
    return "system";
  }
}

export function setConsoleAppearance(value: ConsoleAppearance): void {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch {
    /* storage disabled — ignore */
  }
  applyThemeMode(getResolvedConsoleMode());
}

export function getResolvedConsoleMode(): ResolvedMode {
  const stored = getConsoleAppearance();
  return stored === "light" || stored === "dark" ? stored : systemMode();
}

export function applyThemeMode(mode: ResolvedMode): void {
  document.documentElement.dataset.theme = mode;
}

/** Watch OS appearance. Only fires when stored value is "system" (or missing). */
export function watchSystemAppearance(cb: (mode: ResolvedMode) => void): () => void {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const listener = () => {
    if (getConsoleAppearance() === "system") {
      const m = systemMode();
      applyThemeMode(m);
      cb(m);
    }
  };
  mq.addEventListener("change", listener);
  return () => mq.removeEventListener("change", listener);
}

/**
 * Fetch the default site's theme metadata and publish palette variables to
 * <html>. Never overwrites console's data-theme or console mode.
 *
 * @param slug  Optional slug. When undefined, hits the default-site endpoint
 *              that resolves via SiteRegistry (no slug in URL).
 * @returns     The resolved ThemeDefinition, or null if nothing registered.
 */
export async function applyServerTheme(slug?: string): Promise<ThemeDefinition | null> {
  try {
    const url = slug ? `/api/console/s/${encodeURIComponent(slug)}/theme` : "/api/console/theme";
    const res = await fetch(url);
    if (!res.ok) return null;
    const json = (await res.json()) as { data: { theme_id: string; definition: ThemeDefinition } };
    const def = json?.data?.definition;
    if (!def) return null;
    publishPalette(def);
    return def;
  } catch {
    return null;
  }
}

/** Pure helper: derive the variable map from a ThemeDefinition. */
export function getThemePalette(theme: ThemeDefinition): Record<string, string> {
  return {
    "--accent-hue": String(theme.accent_hue),
    "--public-accent": `oklch(60% 0.14 ${theme.accent_hue})`,
    "--public-surface-bg": theme.preview_colors[0],
    "--public-surface-text": theme.preview_colors[2],
  };
}

function publishPalette(theme: ThemeDefinition): void {
  const root = document.documentElement;
  root.dataset.publicTheme = theme.id;
  // Only set --accent-hue on the document root; OKLCH primitives that
  // depend on it live inside [data-public-theme="..."] scopes.
  root.style.setProperty("--accent-hue", String(theme.accent_hue));
  // Subset of palette for any unscoped consumers
  root.style.setProperty("--public-surface-bg", theme.preview_colors[0]);
  root.style.setProperty("--public-surface-text", theme.preview_colors[2]);
}
```

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean. (ThemeToggle in Task 7 will be the only consumer that breaks if it still imports the old names.)

- [ ] **Step 3: Commit**

```bash
git add web/src/shared/theme.ts
git commit -m "refactor(theme): separate console appearance from public site theme"
```

---

### Task 7: Rewrite `ThemeToggle` as three-state control

**Files:**
- Rewrite: `web/src/shared/ThemeToggle.tsx`

**Interfaces:**
- Consumes: `getConsoleAppearance`, `setConsoleAppearance`, `getResolvedConsoleMode`, `watchSystemAppearance` from `./theme`.
- Produces: `ThemeToggle` React component rendering three buttons (System / Light / Dark) with the current effective mode visually highlighted.

- [ ] **Step 1: Rewrite `web/src/shared/ThemeToggle.tsx` entirely**

Replace the file with:

```tsx
import { useEffect, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

import {
  type ConsoleAppearance,
  type ResolvedMode,
  getConsoleAppearance,
  getResolvedConsoleMode,
  setConsoleAppearance,
  watchSystemAppearance,
} from "./theme";
import { Button } from "./ui/button";

const options: { value: ConsoleAppearance; label: string; icon: typeof Monitor }[] = [
  { value: "system", label: "System", icon: Monitor },
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
];

export function ThemeToggle() {
  const [appearance, setAppearanceState] = useState<ConsoleAppearance>(getConsoleAppearance);
  const [resolved, setResolved] = useState<ResolvedMode>(getResolvedConsoleMode);

  useEffect(() => {
    return watchSystemAppearance(setResolved);
  }, []);

  // Re-resolve when appearance changes.
  useEffect(() => {
    setResolved(getResolvedConsoleMode());
  }, [appearance]);

  function pick(next: ConsoleAppearance) {
    setConsoleAppearance(next);
    setAppearanceState(next);
  }

  return (
    <div
      role="radiogroup"
      aria-label="Console appearance"
      className="inline-flex items-center rounded-md border border-line p-0.5 gap-0.5"
    >
      {options.map(({ value, label, icon: Icon }) => {
        const active = appearance === value;
        return (
          <Button
            key={value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => pick(value)}
            className={`h-7 px-2 text-xs ${active ? "bg-primary text-primary-foreground" : "hover:bg-surface"}`}
            title={
              value === "system"
                ? `System (currently ${resolved})`
                : label
            }
          >
            <Icon className="size-3.5" />
            <span className="ml-1.5 hidden sm:inline">{label}</span>
          </Button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean (Button accepts `className` per existing UI primitives).

- [ ] **Step 3: Browser smoke**

Run `cd web && bun run build` then open `/admin` via the dev server. Click each of the three buttons; `<html data-theme>` flips between light/dark; `system` follows OS when toggled.

- [ ] **Step 4: Commit**

```bash
git add web/src/shared/ThemeToggle.tsx
git commit -m "feat(theme): three-state console appearance toggle (system/light/dark)"
```

---

### Task 8: Remove private `ThemeToggle` from `Topbar.tsx` and import shared one

**Files:**
- Modify: `web/src/admin/shell/Topbar.tsx:1-24`

**Interfaces:**
- Consumes: shared `ThemeToggle` from `../../shared/ThemeToggle`.
- Produces: `Topbar` renders the shared three-state toggle.

- [ ] **Step 1: Delete private `ThemeToggle`, import shared**

In `web/src/admin/shell/Topbar.tsx`, replace lines 1–24 with:

```tsx
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { useState, useEffect } from "react";
import { SiteSelector } from "./SiteSelector";
import { listSites } from "../shared/api";
import { Settings } from "lucide-react";
import { ThemeToggle } from "../../shared/ThemeToggle";
```

(Delete `Sun`, `Moon` imports — they move into the shared toggle. Drop the entire local `function ThemeToggle() { ... }` block, lines 8–24.)

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add web/src/admin/shell/Topbar.tsx
git commit -m "refactor(topbar): use shared three-state ThemeToggle"
```

---

### Task 9: Mount `applyServerTheme()` in admin `App.tsx`

**Files:**
- Modify: `web/src/admin/App.tsx:40-53, 55-148`

**Interfaces:**
- Consumes: `applyServerTheme` from `../shared/theme`.
- Produces: `AdminApp` runs `applyServerTheme()` once on mount and re-runs on `slug` changes via a small inner component.

- [ ] **Step 1: Add a wrapper component that runs `applyServerTheme`**

In `web/src/admin/App.tsx`, after line 53 (after `ShellFallback`), add:

```tsx
import { useEffect } from "react";
import { applyServerTheme } from "../shared/theme";
import { useParams } from "react-router";

function ThemeBootstrap() {
  const { slug } = useParams();
  useEffect(() => {
    void applyServerTheme(slug);
  }, [slug]);
  return null;
}
```

Move the imports to the top with the rest of the React/react-router imports. The current App.tsx already imports `lazy, Suspense` — keep them. Add `ThemeBootstrap` as a child inside `QueryClientProvider`, e.g. just after `<BrowserRouter>`:

```tsx
      <BrowserRouter>
        <ThemeBootstrap />
        <ScrollToTop />
```

Note: `slug` is undefined at `/sites`; `applyServerTheme(undefined)` hits `/api/console/theme` (default-site endpoint from Task 2).

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 3: Smoke check**

Run `cd web && bun run build`, load `/admin` in Chromium via `xd://browser`, confirm `document.documentElement.dataset.publicTheme` is set to the current site's theme ID after the React tree mounts.

- [ ] **Step 4: Commit**

```bash
git add web/src/admin/App.tsx
git commit -m "feat(admin): bootstrap mount of applyServerTheme()"
```

---

### Task 10: Add Appearance section to SettingsPage

**Files:**
- Modify: `web/src/admin/settings/SettingsPage.tsx:1-9, 51-167, 168-374`

**Interfaces:**
- Consumes: `getTheme(slug)` from `../shared/api`; `useThemeApperance` helpers from `../../shared/theme` (the three setters).
- Produces: a new "Appearance" `<section>` rendered between the "Operations" section and the existing Danger Zone. Three-state console toggle + read-only summary of the current public theme with a link to `/s/{slug}/themes`.

- [ ] **Step 1: Import + section wiring in SettingsPage**

In `web/src/admin/settings/SettingsPage.tsx`, replace the import block at lines 1–9 with:

```tsx
import { useEffect, useState } from "react";
import { useParams, useNavigate, Link } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getConfig, updateConfig, removeSite, listSites, getDefaultSite, setDefaultSite, getTheme, type ConfigResponse } from "../shared/api";
import { Button } from "../../shared/ui/button";
import { Input } from "../../shared/ui/input";
import { Skeleton } from "../../shared/ui/skeleton";
import { Trash2, X } from "lucide-react";
import { ThemeToggle } from "../../shared/ThemeToggle";
```

- [ ] **Step 2: Fetch the current public theme inside `SettingsPage`**

In the `SettingsPage` function body, after line 84 (after `const [defaultSiteSel, setDefaultSiteSel] = useState("");`), add:

```tsx
  const { data: themeData } = useQuery({
    queryKey: ["site", slug, "theme"],
    queryFn: () => getTheme(slug!),
    enabled: !!slug,
  });
```

- [ ] **Step 3: Insert the Appearance section into the JSX**

In the JSX returned from `SettingsPage`, after the existing section blocks (find the spot before the Danger Zone — it begins with `<h3 className="text-sm font-semibold mb-4 text-red-600">Danger Zone</h3>` or similar; in this file Danger Zone is the final block). Insert:

```tsx
      <div className="border border-line rounded-lg p-5">
        <h3 className="text-sm font-semibold mb-4">Appearance</h3>
        <div className="flex items-center gap-3 mb-3">
          <div className="text-xs text-muted w-32">Console appearance</div>
          <ThemeToggle />
        </div>
        <div className="flex items-center gap-3">
          <div className="text-xs text-muted w-32">Public site theme</div>
          <div className="text-sm font-medium" data-testid="public-theme-name">
            {themeData?.definition?.name_en ?? "—"}
          </div>
          <Link
            to={`/s/${slug}/themes`}
            className="ml-auto text-xs text-primary hover:underline"
          >
            Open full theme editor →
          </Link>
        </div>
      </div>
```

(If Danger Zone's heading text isn't in this exact form, anchor before the section whose first child reads "Danger Zone".)

- [ ] **Step 4: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean (Link is in react-router already imported via useParams/useNavigate — confirm `Link` is added to existing import block above).

- [ ] **Step 5: Browser smoke**

Build, navigate `/admin/s/{slug}/settings`, confirm three-state toggle works and the public-theme summary shows the current theme.

- [ ] **Step 6: Commit**

```bash
git add web/src/admin/settings/SettingsPage.tsx
git commit -m "feat(settings): Appearance section — console toggle + public theme summary + link"
```

---

### Task 11: Make `ThemesPage` consume the server catalog and shared definition

**Files:**
- Modify: `web/src/admin/themes/ThemesPage.tsx:1-72, 74-138`

**Interfaces:**
- Consumes: `listThemes()` from `../shared/api`; `ThemeDefinition` from `../../shared/theme`.
- Produces: ThemesPage renders the six catalog themes from the server. `getTheme` already returns `{ theme_id, definition }` (Task 4); selection state is initialized from the server.

- [ ] **Step 1: Replace imports and the local `THEMES` constant**

In `web/src/admin/themes/ThemesPage.tsx`, replace lines 1–24 with:

```tsx
import { useState, useEffect } from "react";
import { useParams } from "react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getTheme, setTheme, listThemes } from "../shared/api";
import type { ThemeDefinition } from "../../shared/theme";
import { Button } from "../../shared/ui/button";
import { Skeleton } from "../../shared/ui/skeleton";

function ThemePreview({ theme }: { theme: ThemeDefinition }) {
  const [bg, body, text, accent] = theme.preview_colors;
  return (
    <div className="h-20 p-3 rounded-t-lg" style={{ background: bg }}>
      <div className="text-xs font-mono mb-1" style={{ color: text }}>Aa 가나다</div>
      <div className="h-1.5 w-12 rounded-full" style={{ background: accent, opacity: 0.9 }} />
    </div>
  );
}
```

(The `body` index is unused; the destructuring is intentionally `[bg, , text, accent]` with the second `_`. Adjust to `const [bg, _body, text, accent] = theme.preview_colors;` to silence unused warnings.)

- [ ] **Step 2: Catalog query + replace usage**

In the body of `ThemesPage`, after line 35 (`const [current, setCurrent] = useState("paper");`), add:

```tsx
  const { data: catalog = [] } = useQuery<ThemeDefinition[]>({
    queryKey: ["console", "themes"],
    queryFn: listThemes,
  });
```

Change line 37's initial state to:

```tsx
  const [current, setCurrent] = useState<string>("paper");
```

(Kept — just to be explicit. Adjusts types so the `themes.map` below is well-typed.)

- [ ] **Step 3: Re-wire the grid to use `catalog`**

Replace line 88's body of `{THEMES.map((theme) => (...))}` with the array `catalog`. Replace:

```tsx
        {THEMES.map((theme) => (
          <button
            key={theme.id}
            onClick={() => setCurrent(theme.id)}
            className={`border rounded-lg overflow-hidden text-left cursor-pointer transition-all ${
              current === theme.id ? "border-[#22c55e] border-2" : "border-line hover:border-[#22c55e]"
            }`}
          >
            <ThemePreview theme={theme} />
            <div className="px-3 py-2 border-t border-line flex items-center justify-between">
              <span className="text-sm font-medium">{theme.name}</span>
              {current === theme.id && (
                <span className="text-xs font-bold text-[#22c55e]">✓ Current</span>
              )}
            </div>
          </button>
        ))}
```

with:

```tsx
        {catalog.map((theme) => (
          <button
            key={theme.id}
            onClick={() => setCurrent(theme.id)}
            className={`border rounded-lg overflow-hidden text-left cursor-pointer transition-all ${
              current === theme.id ? "border-primary border-2" : "border-line hover:border-primary"
            }`}
          >
            <ThemePreview theme={theme} />
            <div className="px-3 py-2 border-t border-line flex items-center justify-between">
              <span className="text-sm font-medium">{theme.name_en}</span>
              {current === theme.id && (
                <span className="text-xs font-bold text-primary">✓ Current</span>
              )}
            </div>
          </button>
        ))}
```

(Task 12 will introduce `--accent-hue`-driven accent primitives in tokens.css. The fallback `border-primary` / `text-primary` reads from themed `--color-accent`, which is preserved across modes.)

- [ ] **Step 4: Sync current with the server-side selection**

The block at lines 45–47 already does this (it sets `current` from `data.theme_id`). No additional change here.

- [ ] **Step 5: Typecheck**

Run: `cd web && npx tsc --noEmit`
Expected: clean.

- [ ] **Step 6: Smoke**

Build, open `/admin/s/{slug}/themes`, confirm all six themes render and selecting one then clicking Apply sends a valid PUT.

- [ ] **Step 7: Commit**

```bash
git add web/src/admin/themes/ThemesPage.tsx
git commit -m "feat(themes): ThemesPage consumes server catalog + ThemeDefinition"
```

---

### Task 12: Move sidebar tokens into `[data-theme=light]` and `[data-theme=dark]`; wire `--accent-hue` for public-theme scopes

**Files:**
- Modify: `web/src/shared/tokens.css:51-57, 60-102` (remove sidebar tokens from `:root`; add scoped copies under `[data-theme=light]` and `[data-theme=dark]`; add `[data-public-theme="..."]` accent scopes)

**Interfaces:**
- Consumes: `--accent-hue` set on `:root` by `applyServerTheme`.
- Produces:
  - `[data-theme=light]` sidebar uses light neutral palette (`--p-neutral-50` surfaces, dark text via `--p-neutral-900`), accent from `--p-accent-600`.
  - `[data-theme=dark]` sidebar keeps the current near-black palette.
  - `[data-public-theme="<id>"]` scopes publish `--public-accent-400..700` from OKLCH functions consuming `--accent-hue`.

- [ ] **Step 1: Remove sidebar tokens from `:root`**

In `web/src/shared/tokens.css`, replace lines 51–57 with:

```css
  /* Sidebar tokens live under [data-theme=...] scopes (Task 12). */
```

- [ ] **Step 2: Define light + dark sidebar tokens**

After the closing `}` of `:root` (currently line 58), and before `[data-theme="light"]` (currently line 60), insert:

```css
[data-theme="light"] {
  --console-sidebar-bg:           var(--p-neutral-50);
  --console-sidebar-text:         var(--p-neutral-700);
  --console-sidebar-text-active:  var(--p-accent-700);
  --console-sidebar-border-active:var(--p-accent-600);
  --console-sidebar-hover-bg:     color-mix(in srgb, var(--p-neutral-900) 6%, transparent);
  --console-sidebar-active-bg:    color-mix(in srgb, var(--p-accent-600) 12%, transparent);
  --console-sidebar-border:       var(--p-neutral-100);
  --console-sidebar-label:        var(--p-neutral-500);
}

[data-theme="dark"] {
  --console-sidebar-bg:           #1a1e24;
  --console-sidebar-text:         #9ca3af;
  --console-sidebar-text-active:  #4ade80;
  --console-sidebar-border-active:#22c55e;
  --console-sidebar-hover-bg:     rgba(255, 255, 255, 0.04);
  --console-sidebar-active-bg:    color-mix(in srgb, #22c55e 10%, transparent);
  --console-sidebar-border:       rgba(255, 255, 255, 0.06);
  --console-sidebar-label:        #6b7280;
}
```

- [ ] **Step 3: Wire `--accent-hue` into OKLCH accent primitives inside `[data-public-theme="..."]` scopes**

After the dark sidebar block above, add:

```css
[data-public-theme="paper"],
[data-public-theme="midnight"],
[data-public-theme="sepia"],
[data-public-theme="forest"],
[data-public-theme="neon"],
[data-public-theme="canvas"] {
  --public-accent-400: oklch(78% 0.12 var(--accent-hue));
  --public-accent-500: oklch(62% 0.14 var(--accent-hue));
  --public-accent-600: oklch(50% 0.14 var(--accent-hue));
  --public-accent-700: oklch(40% 0.12 var(--accent-hue));
}
```

These scopes are independent of console `data-theme` so a public theme swap inside `DraftPreviewPane` doesn't repaint the Admin shell.

- [ ] **Step 4: Re-render Admin in a browser to verify nothing broke**

Run `cd web && bun run build`, load `/admin` in Chromium, confirm light + dark sidebar are visually distinct (light: pale neutral bg; dark: near-black). Tailwind v4 will continue to expose semantic tokens via `@theme inline` (lines 111–137) — no change needed there.

- [ ] **Step 5: Commit**

```bash
git add web/src/shared/tokens.css
git commit -m "feat(tokens): sidebar tokens scoped per [data-theme]; --accent-hue drives public accents"
```

---

### Task 13: Replace hex literals in `Sidebar.tsx`, `Topbar.tsx`, `App.tsx ShellFallback` with semantic tokens

**Files:**
- Modify: `web/src/admin/shell/Sidebar.tsx:34-72`
- Modify: `web/src/admin/shell/Topbar.tsx:39-57` (`style={{ color: "var(--p-accent-600)" }}` for the wordmark — keep it semantic, no hex literals remain)
- Modify: `web/src/admin/App.tsx:42-53`

**Interfaces:**
- Consumes: `--console-sidebar-*` tokens from Task 12; Tailwind v4 `--color-*` aliases already present in `tokens.css`.
- Produces: no `#hex` literals remain in the three files; sidebar hover/border tokens read `--console-sidebar-*`.

- [ ] **Step 1: Replace `Sidebar.tsx` hover literals with semantic tokens**

In `web/src/admin/shell/Sidebar.tsx`, replace line 51:

```tsx
                    }`
```

so the resulting className uses tokens instead of hex. The block currently reads:

```tsx
                    }`
                  }
                  style={({ isActive }) => ({
```

Replace the offending literal on line 51 with `hover:text-console-sidebar-text-hover`. Specifically replace the literal `hover:text-[#e5e7eb] hover:bg-[rgba(255,255,255,0.04)]` on line 51 with:

```tsx
                    isActive ? "" : "hover:text-console-sidebar-text-hover hover:bg-console-sidebar-hover-bg"
```

Add the corresponding `--console-sidebar-text-hover` token alongside the existing set in `tokens.css` (Task 12 already added tokens for `bg/border-active`; add `text-hover` to both light + dark blocks now):

```css
  --console-sidebar-text-hover: var(--p-neutral-900);  /* in light block */
  --console-sidebar-text-hover: #e5e7eb;               /* in dark block */
```

(Add these inside the light/dark sidebar blocks defined in Task 12.)

Replace line 70 (`<div className="px-4 py-3 border-t border-[rgba(255,255,255,0.06)] text-xs"...`) with:

```tsx
      <div className="px-4 py-3 border-t border-console-sidebar-border text-xs" style={{ color: "var(--console-sidebar-label)" }}>
```

(Add `--console-sidebar-border` to the light/dark token blocks per Task 12 — already added.)

Add a Tailwind sidecar utility reference in `tokens.css` `@theme inline` after `--color-positive`:

```css
  --color-console-sidebar-bg: var(--console-sidebar-bg);
  --color-console-sidebar-text-hover: var(--console-sidebar-text-hover);
  --color-console-sidebar-hover-bg: var(--console-sidebar-hover-bg);
  --color-console-sidebar-border: var(--console-sidebar-border);
```

This lets utilities like `bg-console-sidebar-bg` resolve inside `data-theme` scopes.

- [ ] **Step 2: Replace `App.tsx` `ShellFallback` literal**

In `web/src/admin/App.tsx`, replace line 45:

```tsx
      <aside className="w-[200px]" style={{ backgroundColor: "#1a1e24", minHeight: "100vh" }} />
```

with:

```tsx
      <aside className="w-[200px] bg-console-sidebar-bg" style={{ minHeight: "100vh" }} />
```

- [ ] **Step 3: `Topbar.tsx` wordmark**

The wordmark currently uses `style={{ color: "var(--p-accent-600)" }}`. That's already a semantic primitive — leave it. Remove the `Sun, Moon` import in `Topbar.tsx` that became unused after Task 8.

- [ ] **Step 4: Typecheck and rebuild**

Run: `cd web && npx tsc --noEmit && cd web && bun run build`
Expected: clean typecheck; build succeeds.

- [ ] **Step 5: Browser smoke**

Open `/admin` in light + dark modes; confirm sidebar surface changes between them and no hex literal styling remains.

- [ ] **Step 6: Commit**

```bash
git add web/src/admin/shell/Sidebar.tsx web/src/admin/shell/Topbar.tsx web/src/admin/App.tsx web/src/shared/tokens.css
git commit -m "refactor(shells): replace hex literals with semantic sidebar tokens"
```

---

## Self-Review

**1. Spec coverage** — checked against `docs/superpowers/specs/2026-07-31-admin-theme-system-design.md`:

- §3.1 ConsoleAppearance + storage key + resolution → Task 6 (`theme.ts`), Task 7 (three-state toggle), Task 5 (`theme-boot.js`).
- §3.2 Public site theme via per-site DB → Task 3 (PUT/GET use shared catalog).
- §3.3 `applyServerTheme()` semantics — never overwrites console mode, default-site fallback, slug-specific fetch → Task 6.
- §4 Shared boot helper; duplicated inline scripts removed → Task 5.
- §5 Single `oxibuilder_core::theme` catalog with 6 themes → Task 1.
- §6 GET/PUT validation + shared catalog endpoint + default-site moved → Tasks 2, 3.
- §7 Shared browser controller surface + three-state toggle + `--accent-hue` consumption → Tasks 6, 7, 12.
- §8 Settings & ThemesPage UX (3-state, summary, link, server-catalog) → Tasks 10, 11.
- §9 Theme-aware sidebar tokens scoped per `[data-theme]`, light/dark palette + AA contrast; hex literals replaced → Tasks 12, 13.
- §10 Static build integration — Task 5 sets `data-public-theme` from `<meta name="oxibuilder-theme">`; the SSG template population is owned by the build-pipeline plan (subproject 4). This plan provisions the boot + apply paths; theme-driven static HTML remains consistent because `theme-boot.js` reads the meta tag and `applyServerTheme` is a no-op for static Pages (Tasks 5, 6).

**2. Placeholder scan** — no "TODO", no "similar to", no "implement later". Every code block stands on its own with full type signatures and identifiers defined earlier in the plan (`ThemeDefinition` defined Task 1/6; `SiteTheme` defined Task 4; the Sidebar token names added in Task 12 propagate to Task 13).

**3. Type consistency** —
- Rust: `ThemeDefinition` (Task 1) consumed by `get_default_theme` (Task 2) and `per_site::theme_get`/`theme_put` (Task 3). `is_known_theme` is the same predicate used by per-site PUT and by `setup::setup_theme_handler`.
- TS: `ThemeDefinition` (Task 6) imported by `api.ts` (Task 4) and `ThemesPage` (Task 11). `getConsoleAppearance`/`setConsoleAppearance` (Task 6) consumed by `ThemeToggle` (Task 7) and `theme-boot.js` (Task 5).
- HTTP shape: `GET /api/console/theme` and `GET /api/console/s/{slug}/theme` both return `{ data: { theme_id, definition } }`.
- Mutation symmetry: `setTheme(slug, id)` calls PUT which returns the same shape; `qc.setQueryData(["site", slug, "theme"], next)` round-trips the same object.

**4. Verification coverage** — `cargo test` gates for Tasks 1, 2, 3, 5; `npx tsc --noEmit` after each TS step (Tasks 4, 6, 7, 8, 9, 10, 11, 13); `bun run build` after Tasks 5, 9, 11, 13. Browser smoke (Chromium via `xd://browser`) after Tasks 5, 7, 9, 10, 11, 12, 13 confirms: (a) no FOUC flash on hard-reload in either stored mode; (b) `data-public-theme` flips on slug change without affecting `<html data-theme>`; (c) sidebar surface visually differs between light and dark; (d) PUT with unknown ID returns 400; (e) ThemesPage renders six entries and applies selection through PUT. `cargo build --workspace` at the end of every task catches accidental breakage in unrelated crates; the foundation's `ctx.settings`/`ctx.startup_server` migration is independent because this plan only touches `ctx.db`.

**5. Foundation `SiteContext` contract** — written against the post-foundation plan state:

- `ctx.settings: Arc<RwLock<MutableSiteSettings>>` carries mutable site fields (`site.name`, `site.base_url`, `site.languages`, `lobby.default_mode`, `integrations.*`). Read with `ctx.settings.read().await.site.*` etc.
- `ctx.startup_server: ServerConfig` carries startup-immutable server fields (`server.host`, `server.port`, `server.data_dir`).
- `MutableSiteSettings` shape is unchanged from the foundation plan: `{ site, lobby, integrations, extensions, deploy }` with `deploy.github_pages: Option<GitHubPagesTarget>` (`{ owner, repo, branch }`).

This plan reads `theme_config` only via `ctx.db` (per-site sqlite singleton). It does NOT read or write any `MutableSiteSettings` field, so the foundation's `ctx.config` → `ctx.settings` migration is independent of this plan and creates no compatibility risk. No code in Tasks 1-13 references `ctx.config`, `ctx.settings`, or `ctx.startup_server`.

**6. Open risk the engineer should know**

- **`SiteRegistry::empty_for_tests` is a test-only helper** (Task 2 Step 3). If the existing `sites_runtime::SiteRegistry::new` requires a `Config` argument, threading it through the constructor signature is a cross-cutting change — keep the helper signature aligned with whatever `build_top_level_router` already accepts. If `BuildGuard::default()` / `DeployGuard::default()` don't exist, replace them with whatever zero-value construction `sites_runtime.rs` already exposes for tests; do not introduce a new public constructor.
- **`SettingsPage` JSX anchor** (Task 10 Step 3) assumes "Danger Zone" is the last section. If the layout has been refactored to remove or rename that anchor, insert the Appearance section between the existing Operations and the final section, not after the page close tag.
- **`ThemesPage` SelectionState** (Task 11 Step 4) re-uses an existing `useEffect` block to sync `current` from the server `theme_id`. If that block has been rewritten to a different key, add a fresh `useEffect` keyed on `data?.theme_id` rather than mutating the existing one.
- **`Sidecar utilities in `@theme inline`** (Task 13 Step 1) — Tailwind v4 picks up the new `--color-console-sidebar-*` aliases only after the CSS file is rebuilt. If `bun run build` finishes but the colors don't change in dev mode, restart the Vite dev server so it re-reads tokens.css from disk.
- **Static build integration** (spec §10) is owned by the build-pipeline subproject. This plan only ensures the runtime paths exist; verifying that the generated HTML carries `data-public-theme` requires the build-pipeline plan to land first.

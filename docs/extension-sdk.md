# Oxipage Extension SDK — building a new extension (doc/01 §1.4)

An Oxipage extension is a Cargo workspace member crate that implements the
`oxipage_core::extension::Extension` trait. This guide walks through building one from scratch.

## 1. Crate scaffold

```
crates/oxipage-ext-myfeature/
├── Cargo.toml
├── migrations/0001_init.sql
├── src/lib.rs        # Extension impl
├── src/model.rs      # sqlx::FromRow models + Input/Patch
├── src/repo.rs       # DB functions
├── src/routes.rs     # axum handlers
└── tests/api.rs      # integration tests
```

`Cargo.toml`:
```toml
[package]
name = "oxipage-ext-myfeature"
version = "0.1.0"
edition = "2024"

[lib]
name = "oxipage_ext_myfeature"
path = "src/lib.rs"

[dependencies]
anyhow.workspace = true
async-trait.workspace = true
axum.workspace = true
oxipage-core = { path = "../oxipage-core" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true

[dev-dependencies]
tokio.workspace = true
tower = { version = "0.5", features = ["util"] }
```

Add the crate to `members` in the workspace `Cargo.toml`.

## 2. Implementing the Extension trait

```rust
use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxipage_core::state::AppState;

pub struct MyFeatureExtension;

#[async_trait]
impl Extension for MyFeatureExtension {
    fn id(&self) -> &'static str { "myfeature" }
    fn display_name(&self, lang: Lang) -> String {
        match lang { Lang::Ko => "내 기능".into(), Lang::En => "My Feature".into() }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![Migration { version: 1, name: "init", sql: include_str!("../migrations/0001_init.sql") }]
    }
    fn routes(&self) -> Router<AppState> {
        Router::new().route("/", get(routes::list).post(routes::create))
        // axum 0.8: path params use the {slug} form (NOT :slug). No trailing slash.
    }
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> { /* ... */ }
}
```

## 3. Core rules (must follow)

1. **Extension table namespace:** each extension's tables use a unique prefix/name. The core
   migration runner tracks schema migrations per extension id, keeping them isolated.
2. **FTS5 shared index:** on publish, upsert into the shared index via
   `oxipage_core::search::upsert(pool, "myfeature", &doc_id, &title, &body, lang, published_at)`.
   On delete/disable, call `delete` / `delete_extension` (doc/02 §2.13).
3. **Draft-first principle:** `create` always sets `published_at = NULL`. Publishing is a separate
   `POST /{id}/publish` action only (doc/04 §4.3).
4. **Write-route auth:** a handler argument `_auth: AdminAuth` means entry itself requires the
   `post:write` scope. Publish actions call `auth.require_scope("post:publish")?;` as the first
   line. Token management calls `require_scope("admin")?` (doc/01 §1.8).
5. **Errors:** use `oxipage_core::error::ApiError` (`new` / `validation` / `internal`). The response
   envelope is `DataEnvelope<T>`.
6. **`order` / `display_order`:** `order` is a SQL reserved word — always use `display_order`.
7. **No cross-extension JOINs:** if you need data from another extension, compose via the core API
   (doc/02 preamble).
8. **Background jobs:** if you need external-API polling or cache refresh, return a `ScheduledJob`
   from `background_jobs()`. The job's `run(&self, ctx: &AppState)` receives the app state so it
   can access `ctx.db` and `ctx.config`. The core scheduler spawns each active extension's jobs
   on a cron driver at boot. If the key is absent, silently disable (doc/01 §1.9).
9. **External API keys:** the `[integrations]` section of `oxipage.toml` holds the *env-var name*;
   read the value via the `Config::integrations` helpers (`tmdb_key()` / `aladin_key()` /
   `github_username()`). Never put plaintext keys in the config file.

## 4. Server registration

Add the dependency to `crates/oxipage-server/Cargo.toml` and add one line —
`Arc::new(MyFeatureExtension)` — to the `all_extensions()` vec in `src/lib.rs`.

## 5. Test patterns

In `tests/api.rs`, assemble the app with an in-memory DB and an `ExtensionRegistry`, then drive it
with `oneshot`. Baseline cases: 401 (no token), 503 (no server token), 422 (validation), the
create → show → publish flow, and FTS upsert verification. `oxipage-ext-blog` and
`oxipage-ext-projects` are the references.

## 6. Runtime-installable extensions (known limitation, doc/01 §1.4)

v1 supports compile-time static linking only. WASM-component-based runtime loading is a Phase 5
spike, and **runtime-installed extensions cannot add CLI subcommands** (clap requires static
linking). Third-party extensions are reachable via the API and web only.

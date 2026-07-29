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
4. **Write-route gate:** handlers that mutate state must live under the management API router.
   The management server is local-only (bind `127.0.0.1`) with no auth; if you expose it, put
   a reverse-proxy auth layer in front. Publish actions are a separate `POST /{id}/publish`
   endpoint to keep the draft-first principle auditable in the route table.
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


## 3.5 Setup wizard hooks (opt-in)

`Extension` 트레이트는 setup wizard가 동적으로 step을 조립할 수 있게 4개의 기본-impl 메서드를 제공한다. **어느 것도 구현하지 않으면 wizard에 등장하지 않는다** — 9개 기본 확장 중 wizard에 자기 자신을 노출하는 것은 profile/movies/books/blog/activity 5개뿐이다.

### `setup_wizard_step()` — 자기 도메인 데이터 입력 step

확장 활성화 후 사용자에게 보여줄 form을 선언한다. 코어가 form 라우팅·저장·렌더링을 모두 처리하고, 이 메서드는 **form 정의 + 저장 핸들러**만 제공한다.

```rust
use std::sync::Arc;
use oxipage_core::extension::{
    SetupStep, SetupField, SetupFieldKind, SetupSaveHandler,
};
use async_trait::async_trait;

struct ProfileSetupSave;

#[async_trait]
impl SetupSaveHandler for ProfileSetupSave {
    async fn save(&self, ctx: &AppState, form: &serde_json::Map<String, serde_json::Value>)
        -> anyhow::Result<()> {
        repo::update_from_setup_form(&ctx.db, form).await
    }
}

impl Extension for ProfileExtension {
    fn setup_wizard_step(&self) -> Option<SetupStep> {
        Some(SetupStep {
            id: "profile",
            title_ko: "프로필",
            title_en: "Profile",
            description_ko: "사이트에 표시할 신상 정보",
            description_en: "Profile info displayed on your site",
            fields: vec![
                SetupField {
                    name: "display_name",
                    label_ko: "표시 이름",
                    label_en: "Display name",
                    kind: SetupFieldKind::Text,
                    required: true,
                    placeholder_ko: None,
                    placeholder_en: None,
                },
            ],
            save_handler: Arc::new(ProfileSetupSave),
        })
    }
    // ...
}
```

**원칙:** `id`는 확장에서 유일. 코어는 첫 번째로 매칭되는 step만 사용한다.

### `external_api_keys()` — 외부 API 키 노출

확장이 외부 서비스(GitHub, TMDB, 알라딘 등)와 연동되면 키 입력란을 노출한다.

```rust
fn external_api_keys(&self) -> Vec<ExternalApiKey> {
    vec![ExternalApiKey {
        id: "tmdb_key",
        label_ko: "TMDB API 키",
        label_en: "TMDB API key",
        env_var: "OXIPAGE_TMDB_KEY",
        required: false,
        scope: ExternalKeyScope::ExtensionConfig,
    }]
}
```

- `scope: EnvOnly` — `std::env::set_var(env_var, value)`만.
- `scope: ExtensionConfig` — env set + `extension_state.config` JSON에도 기록.
- `save_external_key()`를 override하면 도메인별 추가 검증/저장 가능. 기본 impl은 위 두 가지 중 scope에 맞춰 동작한다.

### `seed_sample_data()` — setup 완료 시 시드

setup 완료 시점에 활성 확장에만 호출된다 (best-effort, 실패해도 setup 완료).

```rust
async fn seed_sample_data(&self, ctx: &AppState) -> anyhow::Result<()> {
    // blog 확장의 환영 글 INSERT
    sqlx::query(
        "INSERT INTO blog_post (slug, title, body, lang, tags, published_at, ...)
         VALUES (?, ?, ?, 'ko', '[]', strftime('%Y-%m-%dT%H:%M:%fZ','now'), ...)",
    )
    .bind("환영합니다")
    .bind("환영합니다")
    .bind(WELCOME_BODY)
    .execute(&ctx.db)
    .await?;
    Ok(())
}
```

이전(v1)에는 코어(`oxipage-core/src/setup.rs`)가 직접 `INSERT INTO blog_post` SQL을 작성했다. 이제는 blog 확장이 자기 도메인 데이터를 자기 시점에 시드한다 — **코어가 blog 테이블의 존재를 모른다**.

### 활용 가이드

- 새 확장이 wizard에 참여하려면 위 4개 메서드 중 필요한 것만 override.
- 참여하지 않더라도 정상 동작 — registry에 등록되고 routes/CLI/BuildExt는 그대로 작동.
- 두 확장이 같은 `id`의 외부 키를 노출하면 마지막 확장이 우선.

## 4. Server registration
Add the dependency to `crates/oxipage-console/Cargo.toml` and add one line —
`Arc::new(MyFeatureExtension)` — to the `all_extensions()` vec in `src/lib.rs`.

## 5. Test patterns

In `tests/api.rs`, assemble the app with an in-memory DB and an `ExtensionRegistry`, then drive it
with `oneshot`. Baseline cases: 404 (unknown extension), 422 (validation), the
create → show → publish flow, and FTS upsert verification. The management server has no
auth — write-route tests just exercise the handler. `oxipage-ext-blog` and
`oxipage-ext-projects` are the references.

## 6. Runtime-installable extensions (known limitation, doc/01 §1.4)

v1 supports compile-time static linking only. WASM-component-based runtime loading is a Phase 5
spike, and **runtime-installed extensions cannot add CLI subcommands** (clap requires static
linking). Third-party extensions are reachable via the API and web only.

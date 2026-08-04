# Phase 0 — 뼈대 (Bones) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 빈 사이트라도 실제로 켜져서 접속되는 상태 — Cargo 워크스페이스 + oxibuilder-core(Axum/SQLite/확장 레지스트리) + profile 확장(명함 페이지) + React SPA(OKLCH 토큰, 다크/라이트)가 단일 바이너리로 빌드·실행된다.

**Architecture:** `oxibuilder-core`(lib: config, db, Extension 트레이트, 레지스트리, HTTP, 인증) ← `oxibuilder-ext-profile`(lib) ← `oxibuilder-server`(bin name `oxibuilder-core`: 확장을 링크해 서버 기동). 프론트엔드는 Vite+React+TS SPA로 빌드해 `rust-embed`로 바이너리에 내장. DB는 SQLite(WAL).

**Tech Stack:** Rust 1.96 (edition 2024), axum 0.8, sqlx 0.8 (SQLite), rust-embed 8, tracing / Vite 7, React 19, TypeScript, react-router 7, TanStack Query 5, markdown-it, bun.

**Source spec:** `doc/00-overview.md` ~ `doc/06-roadmap.md` (사용자 승인된 설계 문서). Phase 0 범위는 `doc/06-roadmap.md` Phase 0 섹션.

## Global Constraints

- Rust stable 1.96, edition 2024. 모든 크레이트 `cargo fmt` + `cargo clippy --workspace --all-targets -- -D warnings` 클린.
- sqlx 컴파일타임 매크로(`query!`) 사용 금지 — `query_as` + `#[derive(sqlx::FromRow)]`만 사용 (DATABASE_URL 불필요).
- API 응답 봉투 (doc §4.5): 단건/목록 모두 `{ "data": ... }`, 에러는 `{ "error": { "code": "...", "message": "...", "field": "..."(선택) } }`.
- 확장 API 루트 경로는 trailing slash 없이 사용: `/api/v1/profile` (axum 0.8 nest 시맨틱 — nested root route는 prefix 무슬래시로 서빙. 계획 초기의 trailing slash 제약은 폐기).
- OKLCH 토큰 값은 `doc/03-design-system.md` §3.3에서 verbatim 복사.
- 웹 패키지 매니저는 bun (`bun install`, `bun run build`). 프론트 테스트 프레임워크는 Phase 0에서 도입하지 않음(브라우저 검증으로 대체).
- Phase 0에서는 CLI 크레이트를 만들지 않는다 (doc/06 Phase 1에서 추가 — §1.3 레이아웃 대비 명시적 지연).
- 인증은 v0 임시: 쓰기 API는 `OXIBUILDER_ADMIN_TOKEN` 환경변수와 Bearer 토큰 constant-time 비교. PAT 체계(doc §1.8)는 Phase 1/4에서 교체.
- 워크스페이스 멤버: `crates/oxibuilder-core`(lib), `crates/oxibuilder-ext-profile`(lib), `crates/oxibuilder-server`(bin, 바이너리명 `oxibuilder-core`).
- `lobby_config.display_order` 컬럼명 사용 (doc §2.12의 `order`는 SQL 예약어라 변경 — 명시적 편차).
- 커밋 메시지는 Conventional Commits (`feat:`, `chore:`, `test:` 등).
- 각 태스크 종료 시 해당 범위 테스트 전부 통과 + 커밋.
- **rust-embed 컴파일타임 요구사항:** `#[folder = "../../web/dist"]`는 debug/release 모두 컴파일 시점에 디렉토리가 존재해야 한다. `web/dist/index.html` 플레이스홀더(gitignored 로컬 파일)를 rust-embed 도입 전에 생성하는 것이 필수 선행 조건.

---

### Task 1: Cargo 워크스페이스 + 코어 설정 모듈

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/oxibuilder-core/Cargo.toml`
- Create: `crates/oxibuilder-core/src/lib.rs`
- Create: `crates/oxibuilder-core/src/config.rs`
- Create: `oxibuilder.toml` (repo root, 개발용 예시 설정)
- Modify: `.gitignore` (이미 `/target`, `/data` 포함 확인만)

**Interfaces:**
- Produces: `oxibuilder_core::config::{Config, SiteConfig, ServerConfig, ExtensionsConfig, LobbySection, ConfigError}` — 이후 모든 태스크가 사용.
  - `Config::from_toml_str(&str) -> Result<Config, toml::de::Error>`
  - `Config::load(&Path) -> Result<Config, ConfigError>`
  - `Config::apply_env_overrides(&mut self)` (pub; `OXIBUILDER_PORT`, `OXIBUILDER_DATA_DIR` 적용)
  - `impl Default for Config` — site.name `"Oxibuilder"`, base_url `"http://127.0.0.1:8787"`, default_lang `"ko"`, languages `["ko","en"]`
  - `ServerConfig` 기본값: host `127.0.0.1`, port `8787`, data_dir `data`, api_endpoint `None`

- [ ] **Step 1: 실패하는 테스트 작성**

`crates/oxibuilder-core/src/config.rs` 남은 부분은 아래 Step 3 구현을 참고해 우선 테스트만 `#[cfg(test)] mod tests`로 작성:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let cfg = Config::from_toml_str(r#"
[site]
name = "테스트 작업실"
base_url = "https://example.dev"
"#).unwrap();
        assert_eq!(cfg.site.name, "테스트 작업실");
        assert_eq!(cfg.site.default_lang, "ko");
        assert_eq!(cfg.site.languages, vec!["ko", "en"]);
        assert_eq!(cfg.server.port, 8787);
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert!(cfg.extensions.enabled.is_empty());
        assert_eq!(cfg.lobby.default_mode, "grid");
    }

    #[test]
    fn parses_full_config() {
        let cfg = Config::from_toml_str(r#"
[site]
name = "S"
base_url = "https://b.dev"
default_lang = "en"
languages = ["en", "ko"]

[server]
port = 9999
data_dir = "/var/oxibuilder"

[extensions]
enabled = ["profile", "blog"]

[lobby]
default_mode = "canvas"
"#).unwrap();
        assert_eq!(cfg.site.default_lang, "en");
        assert_eq!(cfg.server.port, 9999);
        assert_eq!(cfg.server.data_dir, std::path::PathBuf::from("/var/oxibuilder"));
        assert_eq!(cfg.extensions.enabled, vec!["profile", "blog"]);
        assert_eq!(cfg.lobby.default_mode, "canvas");
    }

    #[test]
    fn rejects_invalid_toml() {
        assert!(Config::from_toml_str("not [valid").is_err());
    }

    #[test]
    fn env_overrides_port_and_data_dir() {
        unsafe {
            std::env::set_var("OXIBUILDER_PORT", "1234");
            std::env::set_var("OXIBUILDER_DATA_DIR", "/tmp/oxi-test");
        }
        let mut cfg = Config::default();
        cfg.apply_env_overrides();
        assert_eq!(cfg.server.port, 1234);
        assert_eq!(cfg.server.data_dir, std::path::PathBuf::from("/tmp/oxi-test"));
        unsafe {
            std::env::remove_var("OXIBUILDER_PORT");
            std::env::remove_var("OXIBUILDER_DATA_DIR");
        }
    }
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p oxibuilder-core`
Expected: FAIL (컴파일 에러 — config 모듈 미구현)

- [ ] **Step 3: 구현**

`Cargo.toml` (root):

```toml
[workspace]
resolver = "2"
members = [
    "crates/oxibuilder-core",
]

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = "0.8"
mime_guess = "2"
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-rustls", "macros", "json", "chrono"] }
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
toml = "0.8"
tower-http = { version = "0.6", features = ["trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[profile.release]
lto = true
codegen-units = 1
```

`crates/oxibuilder-core/Cargo.toml`:

```toml
[package]
name = "oxibuilder-core"
version = "0.1.0"
edition = "2024"

[lib]
name = "oxibuilder_core"
path = "src/lib.rs"

[dependencies]
anyhow.workspace = true
async-trait.workspace = true
axum.workspace = true
mime_guess.workspace = true
rust-embed.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio.workspace = true
toml.workspace = true
tower-http.workspace = true
tracing.workspace = true

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
```

`crates/oxibuilder-core/src/lib.rs`:

```rust
pub mod config;
```

`crates/oxibuilder-core/src/config.rs`:

```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub lobby: LobbySection,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            site: SiteConfig {
                name: "Oxibuilder".into(),
                base_url: "http://127.0.0.1:8787".into(),
                default_lang: default_lang(),
                languages: default_languages(),
            },
            server: ServerConfig::default(),
            extensions: ExtensionsConfig::default(),
            lobby: LobbySection::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_lang")]
    pub default_lang: String,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

fn default_lang() -> String {
    "ko".into()
}

fn default_languages() -> Vec<String> {
    vec!["ko".into(), "en".into()]
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
    pub api_endpoint: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".into(),
            port: 8787,
            data_dir: PathBuf::from("data"),
            api_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LobbySection {
    pub default_mode: String,
}

impl Default for LobbySection {
    fn default() -> Self {
        LobbySection {
            default_mode: "grid".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("failed to parse config file {0}: {1}")]
    Parse(PathBuf, toml::de::Error),
}

impl Config {
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;
        let mut cfg: Config =
            toml::from_str(&raw).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))?;
        cfg.apply_env_overrides();
        Ok(cfg)
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(port) = std::env::var("OXIBUILDER_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                self.server.port = port;
            }
        }
        if let Ok(dir) = std::env::var("OXIBUILDER_DATA_DIR") {
            self.server.data_dir = PathBuf::from(dir);
        }
    }
}
```

(Step 1의 테스트 모듈을 이 파일 끝에 둔다.)

`oxibuilder.toml` (repo root):

```toml
[site]
name = "내 작업실"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko", "en"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile"]

[lobby]
default_mode = "grid"
```

- [ ] **Step 4: 테스트 실행 — 통과 확인**

Run: `cargo test -p oxibuilder-core`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/oxibuilder-core oxibuilder.toml
git commit -m "feat(core): workspace scaffold and oxibuilder.toml config loading"
```

---

### Task 2: DB 풀 + 마이그레이션 러너 + Extension 트레이트 + 레지스트리 + AppState + 인증

**Files:**
- Create: `crates/oxibuilder-core/src/db.rs`
- Create: `crates/oxibuilder-core/src/migrate.rs`
- Create: `crates/oxibuilder-core/src/extension.rs`
- Create: `crates/oxibuilder-core/src/registry.rs`
- Create: `crates/oxibuilder-core/src/state.rs`
- Create: `crates/oxibuilder-core/src/error.rs`
- Create: `crates/oxibuilder-core/src/auth.rs`
- Create: `crates/oxibuilder-core/migrations/core/0001_lobby_config.sql`
- Modify: `crates/oxibuilder-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1의 `Config`.
- Produces (Task 3~4가 사용):
  - `db::connect(&Path) -> anyhow::Result<SqlitePool>` — WAL, foreign_keys ON, 부모 디렉토리 생성
  - `db::connect_memory() -> anyhow::Result<SqlitePool>` — 테스트용 인메모리 (max_connections 1)
  - `extension::{Lang { Ko, En }, Migration { version: i64, name: &'static str, sql: &'static str }, LobbyCard, LobbyCardItem, Extension (async trait)}`
  - Extension 트레이트 시그니처:
    ```rust
    fn id(&self) -> &'static str;
    fn display_name(&self, lang: Lang) -> String;
    fn migrations(&self) -> Vec<Migration>;
    fn routes(&self) -> Router<AppState>;
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard>;
    async fn on_startup(&self, ctx: &AppState) -> anyhow::Result<()> { Ok(()) }
    ```
  - `registry::ExtensionRegistry::new(Vec<Arc<dyn Extension>>)`, `.find(id)`, `.iter()`, `.run_migrations(&SqlitePool)`
  - `migrate::run_migrations(&SqlitePool, extension: &str, &[Migration]) -> anyhow::Result<()>` — `schema_migrations(extension, version, name, applied_at)` 테이블로 멱등 적용
  - `CORE_MIGRATIONS: &[Migration]` (extension id `"_core"`로 실행, lobby_config 생성)
  - `state::AppState { db: SqlitePool, config: Arc<Config>, admin_token: Option<Arc<str>>, registry: Arc<ExtensionRegistry> }` (Clone)
  - `error::{ApiError, ErrorBody, ErrorDetail}` — `ApiError::new(status, code, message)`, `ApiError::validation(field, message)` (422), `ApiError::internal(anyhow::Error)` (500, 메시지는 "internal server error"로 고정 + tracing::error 로그), IntoResponse 구현
  - `auth::AdminAuth` — `FromRequestParts<AppState>` 구현 extractor. `OXIBUILDER_ADMIN_TOKEN` 미설정(admin_token None) → 503 `admin_not_configured`; 헤더 없음/불일치 → 401 `unauthorized`; 일치 시 통과. constant-time 비교.

- [ ] **Step 1: 실패하는 테스트 작성**

`crates/oxibuilder-core/src/migrate.rs` 끝에:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::Migration;

    const M1: Migration = Migration { version: 1, name: "one", sql: "CREATE TABLE t1 (id INTEGER PRIMARY KEY);" };
    const M2: Migration = Migration { version: 2, name: "two", sql: "CREATE TABLE t2 (id INTEGER PRIMARY KEY);" };

    #[tokio::test]
    async fn applies_migrations_once_and_records_them() {
        let pool = crate::db::connect_memory().await.unwrap();
        run_migrations(&pool, "test_ext", &[M1, M2]).await.unwrap();
        run_migrations(&pool, "test_ext", &[M1, M2]).await.unwrap(); // idempotent

        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM schema_migrations WHERE extension = 'test_ext'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 2);

        // tables actually exist
        sqlx::query("INSERT INTO t1 DEFAULT VALUES").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO t2 DEFAULT VALUES").execute(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn namespaces_migrations_per_extension() {
        let pool = crate::db::connect_memory().await.unwrap();
        run_migrations(&pool, "ext_a", &[M1]).await.unwrap();
        run_migrations(&pool, "ext_b", &[M1]).await.unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}
```

`crates/oxibuilder-core/src/auth.rs` 끝에 (extractor 동작 테스트는 Task 3 통합 테스트에서 커버하므로 여기선 constant-time 비교 단위 테스트만):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_compares_bytes() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
    }
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p oxibuilder-core`
Expected: FAIL (컴파일 에러 — 모듈 미구현)

- [ ] **Step 3: 구현**

`crates/oxibuilder-core/src/lib.rs`:

```rust
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod extension;
pub mod migrate;
pub mod registry;
pub mod state;
```

`crates/oxibuilder-core/src/db.rs`:

```rust
use anyhow::Context;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

pub async fn connect(db_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .context("failed to connect to sqlite database")?;
    Ok(pool)
}

pub async fn connect_memory() -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    Ok(pool)
}
```

`crates/oxibuilder-core/src/extension.rs`:

```rust
use crate::state::AppState;
use async_trait::async_trait;
use axum::Router;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCardItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCard {
    pub id: String,
    pub items: Vec<LobbyCardItem>,
}

#[async_trait]
pub trait Extension: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self, lang: Lang) -> String;
    fn migrations(&self) -> Vec<Migration>;
    fn routes(&self) -> Router<AppState>;
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard>;
    async fn on_startup(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}
```

`crates/oxibuilder-core/src/migrate.rs`:

```rust
use crate::extension::Migration;
use sqlx::SqlitePool;

pub const CORE_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "lobby_config",
    sql: include_str!("../migrations/core/0001_lobby_config.sql"),
}];

pub async fn run_migrations(
    pool: &SqlitePool,
    extension: &str,
    migrations: &[Migration],
) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            extension TEXT NOT NULL,
            version INTEGER NOT NULL,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            PRIMARY KEY (extension, version)
        )",
    )
    .execute(pool)
    .await?;

    for m in migrations {
        let applied: Option<(i64,)> = sqlx::query_as(
            "SELECT version FROM schema_migrations WHERE extension = ? AND version = ?",
        )
        .bind(extension)
        .bind(m.version)
        .fetch_optional(pool)
        .await?;
        if applied.is_some() {
            continue;
        }
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(m.sql).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO schema_migrations (extension, version, name) VALUES (?, ?, ?)",
        )
        .bind(extension)
        .bind(m.version)
        .bind(m.name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(extension, version = m.version, name = m.name, "migration applied");
    }
    Ok(())
}
```

(Step 1 테스트 모듈 포함)

`crates/oxibuilder-core/migrations/core/0001_lobby_config.sql`:

```sql
CREATE TABLE IF NOT EXISTS lobby_config (
    extension_id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    display_mode TEXT NOT NULL DEFAULT 'grid' CHECK (display_mode IN ('canvas', 'grid', 'list')),
    display_order INTEGER NOT NULL DEFAULT 0,
    style_params JSON NOT NULL DEFAULT '{}'
);
```

`crates/oxibuilder-core/src/registry.rs`:

```rust
use crate::extension::Extension;
use crate::migrate;
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct ExtensionRegistry {
    extensions: Vec<Arc<dyn Extension>>,
}

impl ExtensionRegistry {
    pub fn new(extensions: Vec<Arc<dyn Extension>>) -> Self {
        ExtensionRegistry { extensions }
    }

    pub fn find(&self, id: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.iter().find(|e| e.id() == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Extension>> {
        self.extensions.iter()
    }

    pub async fn run_migrations(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate::run_migrations(pool, "_core", migrate::CORE_MIGRATIONS).await?;
        for ext in &self.extensions {
            migrate::run_migrations(pool, ext.id(), &ext.migrations()).await?;
        }
        Ok(())
    }
}
```

`crates/oxibuilder-core/src/state.rs`:

```rust
use crate::config::Config;
use crate::registry::ExtensionRegistry;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub admin_token: Option<Arc<str>>,
    pub registry: Arc<ExtensionRegistry>,
}
```

`crates/oxibuilder-core/src/error.rs`:

```rust
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, serde::Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, serde::Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub body: ErrorBody,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, message: &str) -> Self {
        ApiError {
            status,
            body: ErrorBody {
                error: ErrorDetail {
                    code: code.to_string(),
                    message: message.to_string(),
                    field: None,
                },
            },
        }
    }

    pub fn validation(field: &str, message: &str) -> Self {
        let mut err = ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "validation_error", message);
        err.body.error.field = Some(field.to_string());
        err
    }

    pub fn internal(err: anyhow::Error) -> Self {
        tracing::error!(error = ?err, "internal server error");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "internal server error")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
```

`crates/oxibuilder-core/src/auth.rs`:

```rust
use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

/// v0 임시 인증 extractor. 쓰기 라우트의 핸들러 인자로 사용한다.
/// PAT 체계(doc §1.8)로 교체 예정 (Phase 1/4).
pub struct AdminAuth;

#[async_trait::async_trait]
impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let Some(expected) = &state.admin_token else {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "admin_not_configured",
                "server has no OXIBUILDER_ADMIN_TOKEN configured",
            ));
        };
        let provided = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        match provided {
            Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(AdminAuth),
            _ => Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "missing or invalid bearer token",
            )),
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
```

(Step 1 테스트 모듈 포함)

- [ ] **Step 4: 테스트 실행 — 통과 확인**

Run: `cargo test -p oxibuilder-core`
Expected: PASS (기존 4 + 신규 3)

- [ ] **Step 5: Commit**

```bash
git add crates/oxibuilder-core
git commit -m "feat(core): db pool, migration runner, extension trait, registry, admin auth"
```

---

### Task 3: `oxibuilder-ext-profile` — 스키마 + 모델 + 리포지토리 + 라우트

**Files:**
- Create: `crates/oxibuilder-ext-profile/Cargo.toml`
- Create: `crates/oxibuilder-ext-profile/src/lib.rs`
- Create: `crates/oxibuilder-ext-profile/src/model.rs`
- Create: `crates/oxibuilder-ext-profile/src/repo.rs`
- Create: `crates/oxibuilder-ext-profile/src/routes.rs`
- Create: `crates/oxibuilder-ext-profile/migrations/0001_init.sql`
- Create: `crates/oxibuilder-ext-profile/tests/api.rs`
- Modify: `Cargo.toml` (root — members에 `"crates/oxibuilder-ext-profile"` 추가)

**Interfaces:**
- Consumes: Task 2 전체 (`Extension`, `AppState`, `AdminAuth`, `ApiError`, `db::connect_memory`).
- Produces: `oxibuilder_ext_profile::{ProfileExtension, model::{Profile, ProfileInput, Education, CustomLink}}`. 라우트: `GET /` (공개), `PUT /` (AdminAuth). Task 4가 `ProfileExtension`을 레지스트리에 등록.
- API 경로 (Task 4에서 `/api/v1/profile` 아래 nest됨): `GET /api/v1/profile`, `PUT /api/v1/profile`.

- [ ] **Step 1: 실패하는 테스트 작성**

`crates/oxibuilder-ext-profile/tests/api.rs`:

```rust
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxibuilder_core::config::Config;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::AppState;
use oxibuilder_ext_profile::ProfileExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(admin_token: Option<&str>) -> Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ProfileExtension)]));
    registry.run_migrations(&pool).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: admin_token.map(Arc::from),
        registry: registry.clone(),
    };
    for e in registry.iter() {
        e.on_startup(&state).await.unwrap();
    }
    let ext_router = registry.find("profile").unwrap().routes();
    Router::new()
        .nest("/api/v1/profile", ext_router)
        .with_state(state)
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn get_profile_returns_seeded_singleton() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(Request::get("/api/v1/profile").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["display_name"], "Oxibuilder"); // Config::default().site.name
}

#[tokio::test]
async fn put_without_token_is_401() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"display_name":"X"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn put_without_configured_token_is_503() {
    let app = test_app(None).await;
    let res = app
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .header("authorization", "Bearer anything")
                .body(Body::from(r#"{"display_name":"X"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "admin_not_configured");
}

#[tokio::test]
async fn put_roundtrip_updates_profile() {
    let app = test_app(Some("tok")).await;
    let payload = r#"{
        "display_name": "김개발",
        "tagline_ko": "밤에 코드를 짜는 사람",
        "tagline_en": "codes at night",
        "email": "me@example.dev",
        "github_username": "myid",
        "education": [{"institution": "SNU", "degree": "BS", "field": "CS", "start_year": 2018, "end_year": 2022}],
        "custom_links": [{"label": "Blog", "url": "https://blog.example.dev", "icon": null}]
    }"#;
    let res = app
        .clone()
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .header("authorization", "Bearer tok")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["display_name"], "김개발");
    assert_eq!(json["data"]["education"][0]["institution"], "SNU");

    let res = app
        .oneshot(Request::get("/api/v1/profile").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"]["tagline_en"], "codes at night");
    assert_eq!(json["data"]["github_username"], "myid");
    assert_eq!(json["data"]["custom_links"][0]["url"], "https://blog.example.dev");
}

#[tokio::test]
async fn put_with_empty_display_name_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .header("authorization", "Bearer tok")
                .body(Body::from(r#"{"display_name":"  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "validation_error");
    assert_eq!(json["error"]["field"], "display_name");
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p oxibuilder-ext-profile`
Expected: FAIL (크레이트 미구현)

- [ ] **Step 3: 구현**

Root `Cargo.toml` members에 `"crates/oxibuilder-ext-profile"` 추가.

`crates/oxibuilder-ext-profile/Cargo.toml`:

```toml
[package]
name = "oxibuilder-ext-profile"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow.workspace = true
async-trait.workspace = true
axum.workspace = true
oxibuilder-core = { path = "../oxibuilder-core" }
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio.workspace = true
tower = { version = "0.5", features = ["util"] }
```

`crates/oxibuilder-ext-profile/migrations/0001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    display_name TEXT NOT NULL,
    tagline_ko TEXT,
    tagline_en TEXT,
    avatar_url TEXT,
    bio_ko TEXT,
    bio_en TEXT,
    email TEXT,
    github_username TEXT,
    linkedin_url TEXT,
    education JSON NOT NULL DEFAULT '[]',
    custom_links JSON NOT NULL DEFAULT '[]',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
```

`crates/oxibuilder-ext-profile/src/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Profile {
    pub display_name: String,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub avatar_url: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    #[sqlx(json)]
    pub education: Vec<Education>,
    #[sqlx(json)]
    pub custom_links: Vec<CustomLink>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Education {
    pub institution: Option<String>,
    pub degree: Option<String>,
    pub field: Option<String>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomLink {
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
}

/// PUT 전체 교체 입력. 생략된 Option 필드는 NULL로 덮어쓴다.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    pub display_name: String,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub avatar_url: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    #[serde(default)]
    pub education: Vec<Education>,
    #[serde(default)]
    pub custom_links: Vec<CustomLink>,
}
```

`crates/oxibuilder-ext-profile/src/repo.rs`:

```rust
use crate::model::{Profile, ProfileInput};
use sqlx::SqlitePool;

const COLUMNS: &str = "display_name, tagline_ko, tagline_en, avatar_url, bio_ko, bio_en,
                       email, github_username, linkedin_url, education, custom_links, updated_at";

pub async fn get(pool: &SqlitePool) -> anyhow::Result<Option<Profile>> {
    let profile = sqlx::query_as::<_, Profile>(&format!(
        "SELECT {COLUMNS} FROM profile WHERE id = 1"
    ))
    .fetch_optional(pool)
    .await?;
    Ok(profile)
}

pub async fn upsert(pool: &SqlitePool, input: &ProfileInput) -> anyhow::Result<Profile> {
    let education = serde_json::to_string(&input.education)?;
    let custom_links = serde_json::to_string(&input.custom_links)?;
    let profile = sqlx::query_as::<_, Profile>(&format!(
        "INSERT INTO profile (id, display_name, tagline_ko, tagline_en, avatar_url, bio_ko, bio_en,
                              email, github_username, linkedin_url, education, custom_links)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT (id) DO UPDATE SET
            display_name = ?1, tagline_ko = ?2, tagline_en = ?3, avatar_url = ?4,
            bio_ko = ?5, bio_en = ?6, email = ?7, github_username = ?8, linkedin_url = ?9,
            education = ?10, custom_links = ?11,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {COLUMNS}"
    ))
    .bind(&input.display_name)
    .bind(&input.tagline_ko)
    .bind(&input.tagline_en)
    .bind(&input.avatar_url)
    .bind(&input.bio_ko)
    .bind(&input.bio_en)
    .bind(&input.email)
    .bind(&input.github_username)
    .bind(&input.linkedin_url)
    .bind(education)
    .bind(custom_links)
    .fetch_one(pool)
    .await?;
    Ok(profile)
}

/// 싱글턴 행이 없으면 기본값으로 만든다 (서버 부팅 시 호출).
pub async fn ensure_singleton(pool: &SqlitePool, display_name: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO profile (id, display_name) VALUES (1, ?)")
        .bind(display_name)
        .execute(pool)
        .await?;
    Ok(())
}
```

`crates/oxibuilder-ext-profile/src/routes.rs`:

```rust
use crate::model::{Profile, ProfileInput};
use crate::repo;
use axum::Json;
use axum::extract::State;
use oxibuilder_core::auth::AdminAuth;
use oxibuilder_core::error::ApiError;
use oxibuilder_core::state::AppState;

#[derive(serde::Serialize)]
pub struct DataEnvelope<T: serde::Serialize> {
    pub data: T,
}

pub async fn get_profile(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<Profile>>, ApiError> {
    let profile = repo::get(&state.db).await.map_err(ApiError::internal)?;
    match profile {
        Some(p) => Ok(Json(DataEnvelope { data: p })),
        None => Err(ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "profile is not initialized",
        )),
    }
}

pub async fn put_profile(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<ProfileInput>,
) -> Result<Json<DataEnvelope<Profile>>, ApiError> {
    if input.display_name.trim().is_empty() {
        return Err(ApiError::validation(
            "display_name",
            "display_name must not be empty",
        ));
    }
    let profile = repo::upsert(&state.db, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: profile }))
}
```

`crates/oxibuilder-ext-profile/src/lib.rs`:

```rust
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxibuilder_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxibuilder_core::state::AppState;

pub struct ProfileExtension;

#[async_trait]
impl Extension for ProfileExtension {
    fn id(&self) -> &'static str {
        "profile"
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "프로필".to_string(),
            Lang::En => "Profile".to_string(),
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            name: "init",
            sql: include_str!("../migrations/0001_init.sql"),
        }]
    }

    fn routes(&self) -> Router<AppState> {
        Router::new().route("/", get(routes::get_profile).put(routes::put_profile))
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }

    async fn on_startup(&self, ctx: &AppState) -> anyhow::Result<()> {
        repo::ensure_singleton(&ctx.db, &ctx.config.site.name).await
    }
}
```

- [ ] **Step 4: 테스트 실행 — 통과 확인**

Run: `cargo test -p oxibuilder-ext-profile`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/oxibuilder-ext-profile
git commit -m "feat(ext-profile): profile singleton schema, repo, and REST routes"
```

---

### Task 4: `oxibuilder-server` 바이너리 + HTTP 앱 (healthz, manifest, SPA 정적 서빙)

**Files:**
- Create: `crates/oxibuilder-server/Cargo.toml`
- Create: `crates/oxibuilder-server/src/main.rs`
- Create: `crates/oxibuilder-core/src/http.rs`
- Create: `crates/oxibuilder-core/tests/http_app.rs`
- Modify: `Cargo.toml` (root — members에 `"crates/oxibuilder-server"` 추가)
- Modify: `crates/oxibuilder-core/src/lib.rs` (`pub mod http;` 추가)
- Create: `web/dist/index.html` (플레이스홀더 — rust-embed 컴파일타임 요구사항, gitignored 로컬 파일, Task 6의 실제 빌드가 대체)

**Interfaces:**
- Consumes: Task 2 전부 + Task 3의 `ProfileExtension`.
- Produces:
  - `http::build_app(AppState) -> Router` — 라우팅: `GET /healthz` → `{"status":"ok"}`; `/api/v1/{ext.id()}/**` 각 확장 라우터 nest; `GET /api/v1/lobby/manifest`; `/api/v1` 내 미매칭 → 404 JSON 에러 봉투; 그 외 GET 경로 → `web/dist` 임베드 정적 자산, miss 시 `index.html` 폭백(SPA). tower-http TraceLayer 적용.
  - 매니페스트 응답 형태:
    ```json
    { "data": { "site": { "name": "...", "base_url": "...", "default_lang": "ko", "languages": ["ko","en"] },
                "extensions": [ { "id": "profile", "display_name": { "ko": "프로필", "en": "Profile" } } ] } }
    ```
  - 바이너리 `oxibuilder-core` (package `oxibuilder-server`): 설정 로드(`OXIBUILDER_CONFIG` 또는 `./oxibuilder.toml`, 없으면 Default + 경고) → enabled 필터링된 레지스트리 → DB 연결(`{data_dir}/oxibuilder.db`) → 마이그레이션 → 각 확장 `on_startup` → `OXIBUILDER_ADMIN_TOKEN` 읽기(없으면 경고) → `http://{host}:{port}` serve + graceful shutdown.

- [ ] **Step 1: 플레이스홀더 + 실패하는 테스트 작성**

**선행 필수 (rust-embed 컴파일타임 요구사항 — Global Constraints 참조):**

```bash
mkdir -p web/dist
cat > web/dist/index.html <<'EOF'
<!doctype html><html><head><meta charset="utf-8"><title>Oxibuilder</title></head><body>placeholder</body></html>
EOF
```

(`web/dist`는 이미 gitignored — 커밋되지 않는 로컬 파일로 둔다.)

`crates/oxibuilder-core/tests/http_app.rs`:

```rust
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxibuilder_core::config::Config;
use oxibuilder_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

struct DummyExt;

#[async_trait::async_trait]
impl Extension for DummyExt {
    fn id(&self) -> &'static str {
        "dummy"
    }
    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "더미".to_string(),
            Lang::En => "Dummy".to_string(),
        }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn routes(&self) -> axum::Router<AppState> {
        use axum::routing::get;
        axum::Router::new().route(
            "/",
            get(|| async { axum::Json(serde_json::json!({"data": {"ok": true}})) }),
        )
    }
    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
}

async fn test_app() -> axum::Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: None,
        registry,
    };
    oxibuilder_core::http::build_app(state)
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn healthz_returns_ok() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains("\"ok\""));
}

#[tokio::test]
async fn manifest_lists_enabled_extensions() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/api/v1/lobby/manifest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.contains("\"id\":\"dummy\""));
    assert!(body.contains("\"ko\":\"더미\""));
    assert!(body.contains("\"en\":\"Dummy\""));
    assert!(body.contains("\"default_lang\":\"ko\""));
}

#[tokio::test]
async fn extension_routes_are_mounted_under_api_v1() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/api/v1/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains("\"ok\":true"));
}

#[tokio::test]
async fn spa_fallback_serves_index_html_for_unknown_paths() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/some/unknown/path").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.to_lowercase().contains("<!doctype html"));
}

#[tokio::test]
async fn unknown_api_path_returns_404_json() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/api/v1/nope/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_string(res).await;
    assert!(body.contains("\"error\""));
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test -p oxibuilder-core`
Expected: FAIL (`http` 모듈 미구현)

- [ ] **Step 3: 구현**

Root `Cargo.toml` members에 `"crates/oxibuilder-server"` 추가.

`crates/oxibuilder-core/src/lib.rs`에 `pub mod http;` 추가.

`crates/oxibuilder-core/src/http.rs`:

```rust
use crate::error::ApiError;
use crate::extension::Lang;
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

#[derive(serde::Serialize)]
struct DataEnvelope<T: serde::Serialize> {
    data: T,
}

#[derive(serde::Serialize)]
struct ManifestSite {
    name: String,
    base_url: String,
    default_lang: String,
    languages: Vec<String>,
}

#[derive(serde::Serialize)]
struct ManifestExtension {
    id: &'static str,
    display_name: ManifestLocalized,
}

#[derive(serde::Serialize)]
struct ManifestLocalized {
    ko: String,
    en: String,
}

#[derive(serde::Serialize)]
struct Manifest {
    site: ManifestSite,
    extensions: Vec<ManifestExtension>,
}

pub fn build_app(state: AppState) -> Router {
    let mut api = Router::new().route("/lobby/manifest", get(lobby_manifest));
    for ext in state.registry.iter() {
        api = api.nest(&format!("/{}", ext.id()), ext.routes());
    }
    let api = api.fallback(api_not_found);

    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api)
        .fallback(static_handler)
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn api_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "resource not found")
}

async fn lobby_manifest(State(state): State<AppState>) -> Json<DataEnvelope<Manifest>> {
    let extensions = state
        .registry
        .iter()
        .map(|e| ManifestExtension {
            id: e.id(),
            display_name: ManifestLocalized {
                ko: e.display_name(Lang::Ko),
                en: e.display_name(Lang::En),
            },
        })
        .collect();
    Json(DataEnvelope {
        data: Manifest {
            site: ManifestSite {
                name: state.config.site.name.clone(),
                base_url: state.config.site.base_url.clone(),
                default_lang: state.config.site.default_lang.clone(),
                languages: state.config.site.languages.clone(),
            },
            extensions,
        },
    })
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    serve_asset(path)
        .or_else(|| serve_asset("index.html"))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn serve_asset(path: &str) -> Option<Response> {
    Assets::get(path).map(|content| {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    })
}
```

`crates/oxibuilder-server/Cargo.toml`:

```toml
[package]
name = "oxibuilder-server"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "oxibuilder-core"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
oxibuilder-core = { path = "../oxibuilder-core" }
oxibuilder-ext-profile = { path = "../oxibuilder-ext-profile" }
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

`crates/oxibuilder-server/src/main.rs`:

```rust
use oxibuilder_core::config::Config;
use oxibuilder_core::extension::Extension;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::AppState;
use oxibuilder_ext_profile::ProfileExtension;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config_path = std::env::var("OXIBUILDER_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("oxibuilder.toml"));
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        tracing::warn!(path = %config_path.display(), "config file not found; using defaults");
        let mut cfg = Config::default();
        cfg.apply_env_overrides();
        cfg
    };
    let config = Arc::new(config);

    // Phase 0: 컴파일 타임 확장 목록. enabled 비어있으면 전부 활성.
    let all: Vec<Arc<dyn Extension>> = vec![Arc::new(ProfileExtension)];
    let enabled: Vec<Arc<dyn Extension>> = if config.extensions.enabled.is_empty() {
        all
    } else {
        all.into_iter()
            .filter(|e| config.extensions.enabled.iter().any(|id| id == e.id()))
            .collect()
    };
    let registry = Arc::new(ExtensionRegistry::new(enabled));

    let db_path = config.server.data_dir.join("oxibuilder.db");
    let db = oxibuilder_core::db::connect(&db_path).await?;
    registry.run_migrations(&db).await?;

    let admin_token: Option<Arc<str>> = std::env::var("OXIBUILDER_ADMIN_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(Arc::from);
    if admin_token.is_none() {
        tracing::warn!("OXIBUILDER_ADMIN_TOKEN is not set; write APIs will return 503 admin_not_configured");
    }

    let state = AppState {
        db,
        config: config.clone(),
        admin_token: admin_token.clone(),
        registry: registry.clone(),
    };
    for ext in registry.iter() {
        ext.on_startup(&state).await?;
    }

    let app = oxibuilder_core::http::build_app(state);
    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("oxibuilder listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
```

- [ ] **Step 4: 테스트 실행 — 통과 확인**

Run: `cargo test --workspace`
Expected: PASS (전부)

- [ ] **Step 5: 바이너리 스모크**

Run: `OXIBUILDER_DATA_DIR=$(mktemp -d) cargo run -p oxibuilder-server` 를 백그라운드로 띄우고 `curl -s localhost:8787/healthz` → `{"status":"ok"}` 확인 후 프로세스 종료.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/oxibuilder-server crates/oxibuilder-core
git commit -m "feat(server): oxibuilder-core binary with http app, manifest, and embedded SPA serving"
```

---

### Task 5: 웹 스캐폴드 + OKLCH 디자인 토큰 + 테마 시스템

**Files:**
- Create: `web/package.json`
- Create: `web/tsconfig.json`
- Create: `web/vite.config.ts`
- Create: `web/index.html`
- Create: `web/src/main.tsx`
- Create: `web/src/App.tsx`
- Create: `web/src/shared/tokens.css`
- Create: `web/src/shared/global.css`
- Create: `web/src/shared/theme.ts`
- Create: `web/src/shared/ThemeToggle.tsx`

**Interfaces:**
- Consumes: 없음 (백엔드 무관). `web/dist`는 Task 4 플레이스홀더가 있어도 `bun run build`가 비우고 다시 채우므로 충돌 없음.
- Produces (Task 6가 사용):
  - `shared/theme.ts`: `type Theme = 'light' | 'dark'`; `getStoredTheme(): Theme | null`; `getSystemTheme(): Theme`; `getEffectiveTheme(): Theme`; `applyTheme(t: Theme): void` (`document.documentElement.dataset.theme = t`); `setStoredTheme(t: Theme | null): void`; `toggleTheme(): Theme` (현재 effective 반전 + 저장 + 적용 + 반환); `watchSystemTheme(cb: (t: Theme) => void): () => void` (저장값 없을 때만 미디어 쿼리 추적, unsubscribe 반환).
  - `shared/ThemeToggle.tsx`: `<ThemeToggle />` — 현재 테마 기준 반대 모드로 전환하는 텍스트 버튼 ("Light"/"Dark" 표시), 클릭 시 `toggleTheme()`.
  - `index.html`: `<head>` 최상단에 FOUC 방지 인라인 스크립트 (localStorage `oxibuilder-theme` → 없으면 `prefers-color-scheme` → `data-theme` 설정).
  - 토큰: `--color-*` 시맨틱 토큰 전부 (light/dark), `.card`, `.btn-primary` 클래스.

- [ ] **Step 1: 스캐폴드 파일 작성**

`web/package.json`:

```json
{
  "name": "oxibuilder-web",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "@tanstack/react-query": "^5.62.0",
    "markdown-it": "^14.1.0",
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "react-router": "^7.6.0"
  },
  "devDependencies": {
    "@types/markdown-it": "^14.1.2",
    "@types/react": "^19.1.0",
    "@types/react-dom": "^19.1.0",
    "@vitejs/plugin-react": "^4.5.0",
    "typescript": "^5.8.0",
    "vite": "^7.0.0"
  }
}
```

`web/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "isolatedModules": true,
    "noFallthroughCasesInSwitch": true,
    "types": ["vite/client"]
  },
  "include": ["src", "vite.config.ts"]
}
```

`web/vite.config.ts`:

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: { '/api': 'http://127.0.0.1:8787' },
  },
});
```

`web/index.html`:

```html
<!doctype html>
<html lang="ko">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <script>
      (function () {
        try {
          var t = localStorage.getItem('oxibuilder-theme');
          if (t !== 'light' && t !== 'dark') {
            t = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
          }
          document.documentElement.dataset.theme = t;
        } catch (e) {
          document.documentElement.dataset.theme = 'light';
        }
      })();
    </script>
    <title>Oxibuilder</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

`web/src/shared/tokens.css` — **doc/03-design-system.md §3.3에서 verbatim** (아래 전체):

```css
:root {
  /* 중성색 — "종이/잉크", 아주 낮은 채도, 살짝 따뜻한 톤(95도) */
  --p-neutral-0:   oklch(98.5% 0.004 95);
  --p-neutral-50:  oklch(95%   0.006 95);
  --p-neutral-100: oklch(90%   0.007 95);
  --p-neutral-300: oklch(75%   0.010 95);
  --p-neutral-500: oklch(55%   0.012 95);
  --p-neutral-700: oklch(35%   0.012 265);
  --p-neutral-900: oklch(18%   0.015 265); /* 잉크 */
  --p-neutral-950: oklch(13%   0.020 265); /* 가장 깊은 다크 배경 */

  /* 악센트 — "딥 인디고-바이올렛" */
  --p-accent-400: oklch(78% 0.14 290);
  --p-accent-500: oklch(62% 0.19 290);
  --p-accent-600: oklch(52% 0.20 290);
  --p-accent-700: oklch(42% 0.18 290);

  /* 별점 전용 — "잉크에 찍은 금박" 톤 */
  --p-gold-500: oklch(78% 0.15 85);
  --p-gold-600: oklch(68% 0.15 85);

  /* 상태색 */
  --p-danger-500:  oklch(55% 0.19 25);
  --p-success-500: oklch(60% 0.15 145);

  /* 타이포/형태 토큰 */
  --font-body: "Pretendard Variable", Pretendard, -apple-system, BlinkMacSystemFont,
    "Segoe UI", Roboto, "Noto Sans KR", "Helvetica Neue", Arial, sans-serif;
  --font-mono: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas,
    "Liberation Mono", monospace;
  --radius-md: 0.75rem;
  --space-page-x: clamp(1rem, 4vw, 3rem);
  --content-max-width: 64rem;
}

[data-theme="light"] {
  --color-bg-canvas:      var(--p-neutral-0);
  --color-bg-surface:     var(--p-neutral-50);
  --color-bg-surface-raised: oklch(100% 0 0);
  --color-text-primary:   var(--p-neutral-900);
  --color-text-secondary: var(--p-neutral-700);
  --color-text-tertiary:  var(--p-neutral-500);
  --color-border:         var(--p-neutral-100);
  --color-accent:         var(--p-accent-600);
  --color-accent-contrast:oklch(100% 0 0);
  --color-rating-fill:    var(--p-gold-600);
  --color-danger:         var(--p-danger-500);
  --color-success:        var(--p-success-500);
}

[data-theme="dark"] {
  --color-bg-canvas:      var(--p-neutral-950);
  --color-bg-surface:     var(--p-neutral-900);
  --color-bg-surface-raised: oklch(22% 0.016 265);
  --color-text-primary:   var(--p-neutral-0);
  --color-text-secondary: var(--p-neutral-300);
  --color-text-tertiary:  var(--p-neutral-500);
  --color-border:         oklch(28% 0.015 265);
  --color-accent:         var(--p-accent-400);
  --color-accent-contrast:var(--p-neutral-950);
  --color-rating-fill:    var(--p-gold-500);
  --color-danger:         oklch(65% 0.18 25);
  --color-success:        oklch(68% 0.14 145);
}
```

`web/src/shared/global.css`:

```css
*, *::before, *::after { box-sizing: border-box; }

html, body { margin: 0; padding: 0; }

body {
  background: var(--color-bg-canvas);
  color: var(--color-text-primary);
  font-family: var(--font-body);
  line-height: 1.6;
  transition: background-color 150ms ease, color 150ms ease;
}

a { color: var(--color-accent); text-decoration: none; }
a:hover { text-decoration: underline; }

:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.card {
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md, 0.75rem);
  color: var(--color-text-primary);
  padding: 1.25rem;
}

.btn-primary {
  background: var(--color-accent);
  color: var(--color-accent-contrast);
  border: none;
  border-radius: var(--radius-md);
  padding: 0.5rem 1rem;
  font: inherit;
  cursor: pointer;
}

.app-shell {
  max-width: var(--content-max-width);
  margin: 0 auto;
  padding: 0 var(--space-page-x) 4rem;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 0;
  border-bottom: 1px solid var(--color-border);
  margin-bottom: 2rem;
}

.app-header .site-name {
  font-weight: 700;
  font-size: 1.125rem;
  color: var(--color-text-primary);
}
.app-header .site-name:hover { text-decoration: none; }

.header-actions { display: flex; align-items: center; gap: 0.5rem; }

.text-secondary { color: var(--color-text-secondary); }
.text-tertiary { color: var(--color-text-tertiary); }

.markdown p:first-child { margin-top: 0; }
.markdown p:last-child { margin-bottom: 0; }
.markdown pre {
  font-family: var(--font-mono);
  background: var(--color-bg-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: 0.75rem;
  overflow-x: auto;
}
.markdown code { font-family: var(--font-mono); font-size: 0.9em; }

@media (prefers-reduced-motion: reduce) {
  body { transition: none; }
}
```

`web/src/shared/theme.ts`:

```ts
export type Theme = 'light' | 'dark';

const STORAGE_KEY = 'oxibuilder-theme';

export function getStoredTheme(): Theme | null {
  try {
    const t = localStorage.getItem(STORAGE_KEY);
    return t === 'light' || t === 'dark' ? t : null;
  } catch {
    return null;
  }
}

export function getSystemTheme(): Theme {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function getEffectiveTheme(): Theme {
  return getStoredTheme() ?? getSystemTheme();
}

export function applyTheme(t: Theme): void {
  document.documentElement.dataset.theme = t;
}

export function setStoredTheme(t: Theme | null): void {
  try {
    if (t === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, t);
  } catch {
    /* 스토리지 불가 환경은 무시 */
  }
}

export function toggleTheme(): Theme {
  const next: Theme = getEffectiveTheme() === 'dark' ? 'light' : 'dark';
  setStoredTheme(next);
  applyTheme(next);
  return next;
}

/** 저장된 선택이 없을 때만 시스템 테마 변경을 추적한다. unsubscribe를 반환. */
export function watchSystemTheme(cb: (t: Theme) => void): () => void {
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  const listener = () => {
    if (getStoredTheme() === null) {
      const t = getSystemTheme();
      applyTheme(t);
      cb(t);
    }
  };
  mq.addEventListener('change', listener);
  return () => mq.removeEventListener('change', listener);
}
```

`web/src/shared/ThemeToggle.tsx`:

```tsx
import { useEffect, useState } from 'react';
import { type Theme, getEffectiveTheme, toggleTheme, watchSystemTheme } from './theme';

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(() => getEffectiveTheme());

  useEffect(() => watchSystemTheme(setTheme), []);

  return (
    <button
      type="button"
      className="theme-toggle"
      aria-label={theme === 'dark' ? '라이트 모드로 전환' : '다크 모드로 전환'}
      onClick={() => setTheme(toggleTheme())}
    >
      {theme === 'dark' ? 'Light' : 'Dark'}
    </button>
  );
}
```

`web/src/main.tsx`:

```tsx
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './shared/tokens.css';
import './shared/global.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

`web/src/App.tsx` (이 태스크에서는 헤더 + 플레이스홀더 본문만; Task 6에서 라우트/페이지로 교체):

```tsx
import { ThemeToggle } from './shared/ThemeToggle';

export function App() {
  return (
    <div className="app-shell">
      <header className="app-header">
        <span className="site-name">Oxibuilder</span>
        <div className="header-actions">
          <ThemeToggle />
        </div>
      </header>
      <main className="card">
        <p className="text-secondary">설계 토큰 스캐폴드 — Task 6에서 콘텐츠가 들어옵니다.</p>
      </main>
    </div>
  );
}
```

- [ ] **Step 2: 의존성 설치 + 타입체크 + 빌드**

Run: `cd web && bun install && bun run build`
Expected: 설치 성공, tsc 에러 없음, `web/dist/`에 번들 산출.

- [ ] **Step 3: Commit**

```bash
git add web/package.json web/tsconfig.json web/vite.config.ts web/index.html web/src
git commit -m "feat(web): vite react scaffold with OKLCH tokens and theme system"
```

---

### Task 6: 로비 셸 + 프로필 명함 페이지 + API 클라이언트

**Files:**
- Create: `web/src/shared/api.ts`
- Create: `web/src/shared/Markdown.tsx`
- Create: `web/src/shared/language.tsx`
- Create: `web/src/lobby/Lobby.tsx`
- Create: `web/src/lobby/lobby.css`
- Create: `web/src/extensions/profile/ProfilePage.tsx`
- Create: `web/src/extensions/profile/profile.css`
- Modify: `web/src/App.tsx` (전면 교체)
- Modify: `web/src/shared/global.css` (`.theme-toggle` 스타일 추가)

**Interfaces:**
- Consumes: Task 5의 `ThemeToggle`, theme.ts; 백엔드 `GET /api/v1/lobby/manifest`, `GET /api/v1/profile` (Task 3/4).
- Produces:
  - `shared/api.ts`: `fetchManifest(): Promise<Manifest>`, `fetchProfile(): Promise<Profile>` (+ 타입 `Manifest`, `Profile`, `Education`, `CustomLink`, `LocalizedName`). `/api/v1` 프리픽스 fetch 래퍼, `{data}` 봉투 언랩, 에러 시 상태코드 포함 throw.
  - `shared/language.tsx`: `LanguageProvider`, `useLanguage(): { lang: 'ko' | 'en'; setLang(l): void; pick(ko?: string | null, en?: string | null): string }` — pick은 현재 lang 우선, 없으면 다른 쪽, 둘 다 없으면 `''`.
  - 라우트: `/` → Lobby (매니페스트의 확장 카드 그리드, 카드 클릭 시 `/{id}`로 이동), `/profile` → ProfilePage, `*` → 간단한 404.
  - 헤더: 사이트명(매니페스트, `/` 링크) + 언어 토글(KO/EN) + ThemeToggle.

- [ ] **Step 1: 구현**

`web/src/shared/api.ts`:

```ts
export interface LocalizedName {
  ko?: string;
  en?: string;
}

export interface ManifestSite {
  name: string;
  base_url: string;
  default_lang: string;
  languages: string[];
}

export interface ManifestExtension {
  id: string;
  display_name: LocalizedName;
}

export interface Manifest {
  site: ManifestSite;
  extensions: ManifestExtension[];
}

export interface Education {
  institution: string | null;
  degree: string | null;
  field: string | null;
  start_year: number | null;
  end_year: number | null;
}

export interface CustomLink {
  label: string;
  url: string;
  icon: string | null;
}

export interface Profile {
  display_name: string;
  tagline_ko: string | null;
  tagline_en: string | null;
  avatar_url: string | null;
  bio_ko: string | null;
  bio_en: string | null;
  email: string | null;
  github_username: string | null;
  linkedin_url: string | null;
  education: Education[];
  custom_links: CustomLink[];
  updated_at: string;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function apiFetch<T>(path: string): Promise<T> {
  const res = await fetch(`/api/v1${path}`);
  if (!res.ok) {
    throw new ApiError(res.status, `API request failed: ${res.status} ${path}`);
  }
  const json = (await res.json()) as { data: T };
  return json.data;
}

export const fetchManifest = () => apiFetch<Manifest>('/lobby/manifest');
export const fetchProfile = () => apiFetch<Profile>('/profile');
```

`web/src/shared/language.tsx`:

```tsx
import { createContext, useContext, useMemo, useState, type ReactNode } from 'react';

export type Lang = 'ko' | 'en';

interface LanguageValue {
  lang: Lang;
  setLang: (l: Lang) => void;
  pick: (ko?: string | null, en?: string | null) => string;
}

const LanguageContext = createContext<LanguageValue | null>(null);

export function LanguageProvider({
  defaultLang,
  children,
}: {
  defaultLang: Lang;
  children: ReactNode;
}) {
  const [lang, setLang] = useState<Lang>(defaultLang);
  const value = useMemo<LanguageValue>(
    () => ({
      lang,
      setLang,
      pick: (ko, en) => (lang === 'ko' ? ko || en || '' : en || ko || ''),
    }),
    [lang],
  );
  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

export function useLanguage(): LanguageValue {
  const ctx = useContext(LanguageContext);
  if (!ctx) throw new Error('useLanguage must be used within LanguageProvider');
  return ctx;
}
```

`web/src/shared/Markdown.tsx`:

```tsx
import { useMemo } from 'react';
import MarkdownIt from 'markdown-it';

const md = new MarkdownIt({ linkify: true });

export function Markdown({ source }: { source: string }) {
  const html = useMemo(() => md.render(source), [source]);
  // 서버에 저장된 오너 본인의 마크다운이라 sanitize 없이 렌더링한다 (1인 오너 전제, doc §0.3).
  return <div className="markdown" dangerouslySetInnerHTML={{ __html: html }} />;
}
```

`web/src/lobby/lobby.css`:

```css
.lobby-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
  gap: 1rem;
}

.lobby-card {
  display: block;
  color: var(--color-text-primary);
  transition: transform 150ms ease, box-shadow 150ms ease;
}
.lobby-card:hover {
  transform: translateY(-2px);
  text-decoration: none;
  box-shadow: 0 4px 16px oklch(0% 0 0 / 0.08);
}
.lobby-card h2 {
  margin: 0 0 0.25rem;
  font-size: 1.05rem;
}

@media (prefers-reduced-motion: reduce) {
  .lobby-card { transition: none; }
  .lobby-card:hover { transform: none; }
}
```

`web/src/lobby/Lobby.tsx`:

```tsx
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { fetchManifest } from '../shared/api';
import { useLanguage } from '../shared/language';
import './lobby.css';

export function Lobby() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const { lang } = useLanguage();

  if (!manifest) return null;

  return (
    <section className="lobby-grid">
      {manifest.extensions.map((ext) => (
        <Link key={ext.id} to={`/${ext.id}`} className="card lobby-card">
          <h2>{(lang === 'ko' ? ext.display_name.ko : ext.display_name.en) ?? ext.id}</h2>
        </Link>
      ))}
    </section>
  );
}
```

`web/src/extensions/profile/profile.css`:

```css
.profile-hero {
  display: flex;
  gap: 1.25rem;
  align-items: flex-start;
}

.profile-avatar {
  width: 5rem;
  height: 5rem;
  border-radius: 50%;
  object-fit: cover;
  border: 1px solid var(--color-border);
}

.profile-hero h1 { margin: 0 0 0.25rem; font-size: 1.5rem; }

.profile-tagline { margin: 0; }

.profile-contacts {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
  margin-top: 1rem;
  font-size: 0.925rem;
}

.profile-section { margin-top: 1.5rem; }
.profile-section h2 { font-size: 1.05rem; margin: 0 0 0.5rem; }

.profile-education {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.5rem;
}
```

`web/src/extensions/profile/ProfilePage.tsx`:

```tsx
import { useQuery } from '@tanstack/react-query';
import { fetchProfile } from '../../shared/api';
import { useLanguage } from '../../shared/language';
import { Markdown } from '../../shared/Markdown';
import './profile.css';

export function ProfilePage() {
  const { data: profile, isLoading, error } = useQuery({
    queryKey: ['profile'],
    queryFn: fetchProfile,
  });
  const { pick, lang } = useLanguage();

  if (isLoading) return <p className="text-tertiary">…</p>;
  if (error || !profile) return <p className="text-tertiary">프로필을 불러오지 못했습니다.</p>;

  const tagline = pick(profile.tagline_ko, profile.tagline_en);
  const bio = pick(profile.bio_ko, profile.bio_en);

  return (
    <article>
      <div className="card profile-hero">
        {profile.avatar_url && (
          <img className="profile-avatar" src={profile.avatar_url} alt={profile.display_name} />
        )}
        <div>
          <h1>{profile.display_name}</h1>
          {tagline && <p className="profile-tagline text-secondary">{tagline}</p>}
          <nav className="profile-contacts">
            {profile.email && <a href={`mailto:${profile.email}`}>{profile.email}</a>}
            {profile.github_username && (
              <a href={`https://github.com/${profile.github_username}`} rel="me">
                GitHub
              </a>
            )}
            {profile.linkedin_url && <a href={profile.linkedin_url}>LinkedIn</a>}
            {profile.custom_links.map((l) => (
              <a key={l.url} href={l.url}>
                {l.label}
              </a>
            ))}
          </nav>
        </div>
      </div>

      {bio && (
        <section className="profile-section card">
          <Markdown source={bio} />
        </section>
      )}

      {profile.education.length > 0 && (
        <section className="profile-section">
          <h2>{lang === 'ko' ? '학력' : 'Education'}</h2>
          <ul className="profile-education">
            {profile.education.map((e, i) => (
              <li key={i} className="card">
                <strong>{e.institution}</strong>
                {(e.degree || e.field) && (
                  <span className="text-secondary">
                    {' '}
                    — {[e.degree, e.field].filter(Boolean).join(', ')}
                  </span>
                )}
                {(e.start_year || e.end_year) && (
                  <span className="text-tertiary">
                    {' '}
                    ({e.start_year ?? '?'}–{e.end_year ?? (lang === 'ko' ? '현재' : 'present')})
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
```

`web/src/App.tsx` (전면 교체):

```tsx
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { BrowserRouter, Link, Route, Routes } from 'react-router';
import { Lobby } from './lobby/Lobby';
import { ProfilePage } from './extensions/profile/ProfilePage';
import { fetchManifest } from './shared/api';
import { LanguageProvider, useLanguage, type Lang } from './shared/language';
import { ThemeToggle } from './shared/ThemeToggle';

const queryClient = new QueryClient();

function LangToggle() {
  const { lang, setLang } = useLanguage();
  return (
    <button
      type="button"
      className="theme-toggle"
      aria-label="언어 전환 / Switch language"
      onClick={() => setLang(lang === 'ko' ? 'en' : 'ko')}
    >
      {lang === 'ko' ? 'EN' : 'KO'}
    </button>
  );
}

function Shell() {
  const { data: manifest } = useQuery({ queryKey: ['manifest'], queryFn: fetchManifest });
  const defaultLang: Lang = manifest?.site.default_lang === 'en' ? 'en' : 'ko';

  return (
    <LanguageProvider defaultLang={defaultLang}>
      <div className="app-shell">
        <header className="app-header">
          <Link to="/" className="site-name">
            {manifest?.site.name ?? 'Oxibuilder'}
          </Link>
          <div className="header-actions">
            <LangToggle />
            <ThemeToggle />
          </div>
        </header>
        <main>
          <Routes>
            <Route path="/" element={<Lobby />} />
            <Route path="/profile" element={<ProfilePage />} />
            <Route path="*" element={<p className="text-tertiary">404</p>} />
          </Routes>
        </main>
      </div>
    </LanguageProvider>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Shell />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
```

`web/src/shared/global.css` 끝에 추가:

```css
.theme-toggle {
  background: var(--color-bg-surface);
  color: var(--color-text-secondary);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  padding: 0.25rem 0.75rem;
  font: inherit;
  font-size: 0.85rem;
  cursor: pointer;
}
.theme-toggle:hover { color: var(--color-text-primary); }
```

- [ ] **Step 2: 타입체크 + 빌드**

Run: `cd web && bun run build`
Expected: tsc 에러 없음, `web/dist` 갱신 산출.

- [ ] **Step 3: Commit**

```bash
git add web/src
git commit -m "feat(web): lobby shell, profile business card page, api client"
```

---

### Task 7: E2E 검증 + README

**Files:**
- Create: `README.md`

**Interfaces:**
- Consumes: 전 태스크.
- Produces: 루트 README (빌드/실행/개발 워크플로우 문서).

- [ ] **Step 1: 전체 게이트**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && bun install && bun run build && cd ..
```
Expected: 전부 통과. clippy 경고는 수정하고 통과시킨다.

- [ ] **Step 2: 릴리스 바이너리 빌드**

Run: `cargo build --release -p oxibuilder-server`
Expected: `target/release/oxibuilder-core` 생성 (web/dist가 임베드됨).

- [ ] **Step 3: 실행 + API 스모크**

```bash
export OXIBUILDER_DATA_DIR=$(mktemp -d)
export OXIBUILDER_ADMIN_TOKEN=phase0-smoke-token
./target/release/oxibuilder-core &
sleep 1
curl -sf localhost:8787/healthz                                          # {"status":"ok"}
curl -sf localhost:8787/api/v1/lobby/manifest                            # extensions에 profile 포함
curl -sf localhost:8787/api/v1/profile                                  # display_name = "내 작업실" (oxibuilder.toml site.name)
curl -s -o /dev/null -w '%{http_code}' -X PUT localhost:8787/api/v1/profile \
  -H 'content-type: application/json' -d '{"display_name":"x"}'          # 401
curl -sf -X PUT localhost:8787/api/v1/profile \
  -H "authorization: Bearer $OXIBUILDER_ADMIN_TOKEN" -H 'content-type: application/json' \
  -d '{"display_name":"내 작업실","tagline_ko":"밤에 코드를 짜다가 문장을 잇는 작업실","tagline_en":"a quiet studio","bio_ko":"**개발자**이자 작가.","github_username":"toru-ver4"}'
curl -sf localhost:8787/ | grep -qi 'doctype html'                       # SPA index.html 서빙
curl -sf localhost:8787/some/spa/route | grep -qi 'doctype html'         # SPA 폭백
```
Expected: 전부 기대값대로. 확인 후 서버 프로세스 종료.

- [ ] **Step 4: README.md 작성**

````markdown
# Oxibuilder

개인 창작 작업실 — 개발자·작가·비평가·큐레이터로서의 "나"를 한곳에 모으는 셀프호스팅 개인 홈페이지.
설계 문서: `doc/00-overview.md` ~ `doc/06-roadmap.md`.

## 요구 사항

- Rust 1.96+ (stable)
- bun 1.3+ (프론트엔드 빌드 전용 — 런타임에는 Node 불필요)

## 빌드 & 실행

```bash
cd web && bun install && bun run build && cd ..   # 프론트엔드 빌드 (바이너리에 임베드)
cargo build --release -p oxibuilder-server            # → target/release/oxibuilder-core
OXIBUILDER_ADMIN_TOKEN=<랜덤 토큰> ./target/release/oxibuilder-core
# http://127.0.0.1:8787
```

- 설정: `oxibuilder.toml` (없으면 기본값으로 기동). `OXIBUILDER_CONFIG`, `OXIBUILDER_PORT`, `OXIBUILDER_DATA_DIR` 환경변수로 오버라이드.
- 쓰기 API: `Authorization: Bearer $OXIBUILDER_ADMIN_TOKEN` (v0 임시 인증; PAT 체계는 로드맵 Phase 1/4).

## 개발 워크플로우

```bash
cargo run -p oxibuilder-server     # 백엔드 :8787 (debug 빌드는 web/dist를 디스크에서 읽음)
cd web && bun run dev           # 프론트엔드 개발 서버 :5173 (/api → :8787 프록시)
```

## 테스트

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
````

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: quickstart README"
```

---

## Self-Review 결과 (계획 작성자 검토)

- 스펙 커버리지 (doc/06 Phase 0 완료 기준 매핑): Cargo 워크스페이스 → T1; core 부트스트랩/SQLite/설정/레지스트리 → T1~T4; Vite+React+TS + OKLCH + 다크/라이트 → T5; profile 확장 + 명함 페이지 → T3, T6; 바이너리 빌드·실행 → T4, T7; 라이트/다크 렌더 검증 → T7 + 컨트롤러 브라우저 확인. container 패키징은 "선택"이라 Phase 0에서 제외 (deploy/ 는 Phase 1).
- 의도적 편차 3건: ① CLI 크레이트 미포함(Phase 1), ② `lobby_config.order` → `display_order` (SQL 예약어), ③ 임시 인증 OXIBUILDER_ADMIN_TOKEN (PAT는 Phase 1/4). 전부 Global Constraints에 명시.
- 타입 일관성: `ProfileExtension`, `AppState` 필드, `AdminAuth`, `DataEnvelope`, 매니페스트 JSON 필드(snake_case) ↔ 프론트 타입 일치 확인.
- rust-embed 컴파일타임 디렉토리 요구: Task 4 Step 1에서 `web/dist/index.html` 플레이스홀더를 cargo 실행 전에 생성. Task 5/6의 `vite build`가 dist를 비우고 실제 산출물로 대체.

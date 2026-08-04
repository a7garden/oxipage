# Console Follow-up Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 완료 보고서 §4의 코드 변경 4건(Books 레거시 status 정규화, TMDB 미설정 힌트, 빌드 경고 2건, 비-API `/preview/*` 404)을 구현한다.

**Architecture:** 4개 아이템은 독립적 — Rust 서버(books/console 크레이트)와 Admin SPA(web/)로 분리. Books 정규화는 repo 계층 단일 지점, TMDB 힌트는 `ApiError.code` 보존 + react-query 에러 분기, preview 404는 top-level 라우트로 SPA fallback보다 우선.

**Tech Stack:** Rust (axum 0.8, sqlx, tokio), React 19 + react-router v7 + TanStack Query (Vite), TypeScript.

**Spec:** `docs/superpowers/specs/2026-07-31-console-followup-design.md`

## Global Constraints

- 새 의존성, 마이그레이션, 스키마 변경 없음.
- 마이그레이션/스키마/쓰기 경로 수정 금지 — 정규화는 읽기 경로(repo)에서만.
- `ApiValidationError`와 기존 `Error` 경로 비파괴 유지 (TMDB 힌트 변경 시).
- canonical preview URL(`/api/console/preview/{slug}/`) 동작 불변.
- 커밋 메시지 규약: `fix(books): …`, `fix(web): …`, `feat(console): …` 스타일 (기존 로그 준수).
- 검증 게이트: `cargo check/test` (touched crates), `cd web && npx tsc --noEmit && bun run build`.

---

### Task 1: 빌드 경고 2건 제거 (Item C)

**Files:**
- Modify: `crates/oxibuilder-ext-books/src/repo.rs:50`
- Modify: `crates/oxibuilder-ext-projects/src/repo.rs:98`

**Interfaces:**
- Produces: 두 크레이트 경고 0건. 동작 불변.

- [ ] **Step 1: 수정 적용**

`crates/oxibuilder-ext-books/src/repo.rs:50`:

```rust
// before
    let sql = if let Some(s) = status {
// after
    let sql = if status.is_some() {
```

`crates/oxibuilder-ext-projects/src/repo.rs:98` — 동일 치환 (문맥: `list` 함수의 `published_clause` 직후).

- [ ] **Step 2: 경고 0건 확인**

Run: `cargo check -p oxibuilder-ext-books -p oxibuilder-ext-projects 2>&1 | grep -c warning`
Expected: `1` (profiles for non-root package 경고 1건만 — 이건 기존 workspace 경고로 무해). `unused variable: s` 0건.

- [ ] **Step 3: 커밋**

```bash
git add crates/oxibuilder-ext-books/src/repo.rs crates/oxibuilder-ext-projects/src/repo.rs
git commit -m "fix: remove unused-variable warnings in books/projects list"
```

---

### Task 2: Books 레거시 status 읽기 정규화 (Item A)

**Files:**
- Modify: `crates/oxibuilder-ext-books/src/model.rs` (Book struct 뒤, `default_status` fn 앞)
- Modify: `crates/oxibuilder-ext-books/src/repo.rs` (create/find_by_id/list/update/publish)
- Test: `crates/oxibuilder-ext-books/src/model.rs` `#[cfg(test)] mod tests` + `crates/oxibuilder-ext-books/src/repo.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `Book` (16필드, `#[derive(sqlx::FromRow)]`), repo 함수 5개.
- Produces: `pub fn normalize_status(s: &str) -> &str`, `impl Book { pub fn normalize_status(mut self) -> Self }` — repo 반환 지점에서 호출. 모든 소비자(API/빌드)가 정규화 값을 받음.

- [ ] **Step 1: model.rs에 정규화 함수 + 메서드 추가**

```rust
/// 레거시 status 값 정규화 — 구 DB의 `read`/`dnf`를 현재 4값 체계로 매핑.
/// (`ALLOWED_STATUSES` 참조) 쓰기 경로는 CHECK 제약이 이미 차단하므로 읽기 전용.
pub fn normalize_status(s: &str) -> &str {
    match s {
        "read" => "completed",
        "dnf" => "dropped",
        other => other,
    }
}

impl Book {
    /// 읽기 경로 정규화: 레거시 status 값을 현재 4값 체계로 변환해 반환.
    pub fn normalize_status(mut self) -> Self {
        self.status = normalize_status(&self.status).to_string();
        self
    }
}
```

- [ ] **Step 2: model.rs 단위 테스트 작성**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_status_maps_legacy_values() {
        assert_eq!(normalize_status("read"), "completed");
        assert_eq!(normalize_status("dnf"), "dropped");
    }

    #[test]
    fn normalize_status_keeps_current_values() {
        for s in ["wishlist", "reading", "completed", "dropped"] {
            assert_eq!(normalize_status(s), s);
        }
        assert_eq!(normalize_status("unknown"), "unknown");
    }
}
```

- [ ] **Step 3: repo.rs 5개 지점에 적용**

```rust
// create — 마지막 줄
    Ok(row.normalize_status())

// find_by_id
    Ok(row.map(Book::normalize_status))

// list — 마지막 줄
    Ok(rows.into_iter().map(Book::normalize_status).collect())

// update
    Ok(row.map(Book::normalize_status))

// publish
    Ok(row.normalize_status())
```

- [ ] **Step 4: repo.rs 통합 테스트 작성**

파일 끝에 추가 (`use crate::model::Book;`는 이미 있음; trait import 필요):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxibuilder_core::extension::Extension;

    /// CHECK 제약 우회가 필요한 레거시 행 삽입용 풀. 프로덕션 마이그레이션은
    /// 4값만 허용하므로 PRAGMA로 우회한다 (max_connections=1이라 단일 커넥션 적용).
    async fn test_pool() -> SqlitePool {
        let pool = oxibuilder_core::db::connect_memory().await.unwrap();
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&pool)
            .await
            .unwrap();
        for m in crate::BooksExtension.migrations() {
            sqlx::query(&m.sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn list_normalizes_legacy_status() {
        let pool = test_pool().await;
        sqlx::query("INSERT INTO book_entry (title, status) VALUES ('Legacy Read', 'read')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO book_entry (title, status) VALUES ('Legacy Dnf', 'dnf')")
            .execute(&pool)
            .await
            .unwrap();

        let books = list(&pool, None, 10, true).await.unwrap();
        let read_book = books.iter().find(|b| b.title == "Legacy Read").unwrap();
        let dnf_book = books.iter().find(|b| b.title == "Legacy Dnf").unwrap();
        assert_eq!(read_book.status, "completed");
        assert_eq!(dnf_book.status, "dropped");
    }
}
```

- [ ] **Step 5: 테스트 실행**

Run: `cargo test -p oxibuilder-ext-books`
Expected: 신규 테스트 3건 포함 전체 PASS.

- [ ] **Step 6: 커밋**

```bash
git add crates/oxibuilder-ext-books/src/model.rs crates/oxibuilder-ext-books/src/repo.rs
git commit -m "fix(books): normalize legacy read/dnf status on read"
```

---

### Task 3: TMDB 키 미설정 인라인 힌트 (Item B)

**Files:**
- Modify: `web/src/admin/shared/api.ts:44-88` (ApiError 클래스 + jsonOrThrow)
- Modify: `web/src/admin/content/MoviesTab.tsx` (import + TmdbSearchRow 힌트, ~479-497행)

**Interfaces:**
- Consumes: `jsonOrThrow` (기존), `searchTmdb(slug, q)`, `TmdbSearchRow({ slug, onPick })`.
- Produces: `export class ApiError extends Error { code: string }` — `tmdb_disabled` 등 field 없는 API 에러 감지용. 기존 호출부 영향 없음.

- [ ] **Step 1: api.ts에 ApiError 추가 + jsonOrThrow 분기**

`ApiValidationError` 클래스 뒤에:

```ts
/// field 없는 API 에러도 code를 보존 (ApiValidationError는 field 검증 전용 유지).
export class ApiError extends Error {
  code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }
}
```

`jsonOrThrow` 비-OK 분기 수정:

```ts
    const msg = detail?.message ?? detail ?? `HTTP ${res.status}`;
    if (detail?.code) {
      throw new ApiError(detail.code, msg);
    }
    throw new Error(msg);
```

- [ ] **Step 2: MoviesTab.tsx 힌트 추가**

import 수정:

```tsx
import { ApiError, contentClient, searchTmdb, type TmdbSearchResult } from "../shared/api";
```

추가 (react-router Link):

```tsx
import { Link } from "react-router";
```

`TmdbSearchRow`의 `relative` div 내부, `<Input>` 뒤에:

```tsx
      {search.isError && (search.error as ApiError)?.code === "tmdb_disabled" && (
        <p className="mt-1 text-xs text-muted max-w-56">
          TMDB 검색 비활성 —{" "}
          <Link className="underline" to={`/s/${slug}/settings`}>Settings</Link>
          에서 TMDB Key Env를 확인하거나 <code>OXIBUILDER_TMDB_KEY</code> 환경변수를
          설정하세요. 제목/포스터는 수동 입력할 수 있습니다.
        </p>
      )}
```

- [ ] **Step 3: 타입/빌드 검증**

Run: `cd web && npx tsc --noEmit && bun run build`
Expected: 둘 다 성공.

- [ ] **Step 4: 커밋**

```bash
git add web/src/admin/shared/api.ts web/src/admin/content/MoviesTab.tsx
git commit -m "fix(web): surface TMDB-disabled hint in movies search"
```

---

### Task 4: 비-API `/preview/*` 404 라우트 (Item D)

**Files:**
- Modify: `crates/oxibuilder-console/src/lib.rs` (앱 조립 추출 + 라우트 추가, 173-176행)
- Create: `crates/oxibuilder-console/tests/preview_legacy_404.rs`

**Interfaces:**
- Consumes: `oxibuilder_core::http::build_app(state) -> Router<AppState>`, `crate::router::build_console_router(registry) -> Router<()>`.
- Produces: `pub fn build_console_app(state: AppState, registry: Arc<SiteRegistry>) -> Router<AppState>` — `run_console_with_extensions`과 테스트가 공용.

- [ ] **Step 1: lib.rs에 build_console_app 추출 + 404 핸들러**

`run_console_with_extensions`의 조립부(현재 173-176행)를 교체:

```rust
/// 전체 콘솔 앱 조립: core 앱 + /api/console nest + 비-API /preview/* 404 가드.
pub fn build_console_app(
    state: AppState,
    registry: Arc<SiteRegistry>,
) -> axum::Router<AppState> {
    let console = crate::router::build_console_router(registry);
    let mut app = oxibuilder_core::http::build_app(state);
    app = app.nest("/api/console", console);
    // canonical 경로는 /api/console/preview/{slug}/. 비-API /preview/* 는
    // SPA fallback(admin.html 200) 대신 명시적 404를 반환한다 (design §6).
    app = app.route("/preview/{*rest}", get(preview_legacy_404));
    app
}

/// stateless 404 — Router<AppState>와 호환. axum 라우트는 fallback보다 우선.
async fn preview_legacy_404() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "preview_missing")
}
```

`run_console_with_extensions`의 기존 4줄 조립을:

```rust
    let console = crate::router::build_console_router(site_registry.clone());
    let mut app = oxibuilder_core::http::build_app(state);
    app = app.nest("/api/console", console);
```

다음으로 교체:

```rust
    let app = build_console_app(state, site_registry);
```

import 추가: `use axum::http::StatusCode;` + `use axum::routing::get;`

- [ ] **Step 2: 통합 테스트 작성**

`crates/oxibuilder-console/tests/preview_legacy_404.rs`:

```rust
//! 비-API `/preview/*` 404 가드 + canonical preview 경로 불변 (design §6).

use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use oxibuilder_console::operations::SiteOperationGuard;
use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::config::Config;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::sites::SitesFile;
use oxibuilder_core::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile", "blog"]
"#
    )
}

fn create_site_dir(name: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::with_prefix(format!("oxibuilder-followup-{name}-")).unwrap();
    let toml_path = dir.path().join("oxibuilder.toml");
    std::fs::write(&toml_path, minimal_toml(name)).unwrap();
    (dir, dir.path().to_path_buf())
}

async fn build_console_app() -> axum::Router<AppState> {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry: Arc::new(ExtensionRegistry::new(vec![])),
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };

    let (_dir, path) = create_site_dir("Test");
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path);
    let guard = Arc::new(SiteOperationGuard::new());
    let site_registry = Arc::new(SiteRegistry::new(sf, guard).await.unwrap());
    oxibuilder_console::build_console_app(state, site_registry)
}

#[tokio::test]
async fn non_api_preview_paths_return_404_not_admin_html() {
    let app = build_console_app().await;
    for uri in ["/preview/nope/", "/preview/nope", "/preview/blog/whatever/x"] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "uri={uri}");
        let text = String::from_utf8_lossy(&to_bytes(resp.into_body(), 4096).await.unwrap());
        assert!(!text.contains("admin"), "uri={uri} must not serve admin.html: {text}");
    }
}

#[tokio::test]
async fn api_preview_canonical_path_unaffected() {
    let app = build_console_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/console/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // 등록 사이트 + 무빌드 → 424 build_required (기존 동작 불변).
    assert_eq!(resp.status(), StatusCode::FAILED_DEPENDENCY);
}
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test -p oxibuilder-console --test preview_legacy_404`
Expected: 2건 PASS.

- [ ] **Step 4: 커밋**

```bash
git add crates/oxibuilder-console/src/lib.rs crates/oxibuilder-console/tests/preview_legacy_404.rs
git commit -m "feat(console): 404 for non-API /preview/* instead of SPA fallback"
```

---

## 최종 검증

```bash
cargo check -p oxibuilder-ext-books -p oxibuilder-ext-projects 2>&1 | grep -c "unused variable"   # 0
cargo build --workspace
cargo test -p oxibuilder-ext-books -p oxibuilder-console
cd web && npx tsc --noEmit && bun run build
```

모두 green이면 스위트 완료.

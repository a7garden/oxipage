# Console Follow-up Suite — Design

> **Date:** 2026-07-31
> **Status:** Design draft — awaiting user review
> **Scope:** 2026-07-31 console suite 완료 보고(§4)의 코드 변경 항목 4건

## 1. Goal

5개 서브프로젝트 완료 보고서 `docs/superpowers/2026-07-31-console-suite-progress.md` §4의 비차단 후속 6건 중, 코드 변경이 필요한 4건(§4 항목 3–6)을 마무리한다. 검증 전용 항목(§4 항목 1–2: Profile first-write 스모크, 배포 실측 스모크)은 이 스위트 범위에서 제외한다(사용자 결정, 2026-07-31).

변경은 4개 아이템으로 분리되며 서로 독립적이다 — 병렬 작업 가능:

| # | 항목 | 계층 | 크기 |
|---|---|---|---|
| A | Books 레거시 status 읽기 정규화 | `oxipage-ext-books` | 중 |
| B | TMDB 키 미설정 인라인 힌트 | `web/` (Admin SPA) | 소 |
| C | 빌드 경고 2건 제거 | `oxipage-ext-books`/`oxipage-ext-projects` | 극소 |
| D | 비-API `/preview/*` 404 라우트 | `oxipage-console` | 소 |

## 2. 확정 결정 (사용자, 2026-07-31 ask)

- **스코프:** 코드 변경 항목만 (A–D). 항목 1(T23 스모크), 2(배포 실측)는 설계/구현 제외.
- **Books 레거시 status:** 읽기 시 서버 정규화 (DB 마이그레이션 없음).
- **TMDB 미설정 UX:** 인라인 힌트 + Settings 링크.
- **Preview fallback:** top-level `/preview/*` 404 라우트 추가.

## 3. Item A — Books 레거시 status 읽기 정규화

### 배경

- 스키마/코드는 `wishlist | reading | completed | dropped` 4값으로 정리됨 (`model.rs::ALLOWED_STATUSES`, `0001_init.sql` CHECK 제약).
- 구 DB에는 구버전 값 `read` / `dnf`가 남아 있을 수 있음. CHECK 제약은 신규 입력만 차단하며 기존 행은 그대로.
- 현재 `BookCard.tsx`의 `STATUS_LABEL`은 4키만 있어 레거시 값이면 라벨이 undefined로 렌더링됨.

### 변경

`crates/oxipage-ext-books/src/model.rs`:

```rust
/// 레거시 status 값 정규화. 구 DB의 `read`/`dnf`를 현재 4값 체계로 매핑한다.
pub fn normalize_status(s: &str) -> &str {
    match s {
        "read" => "completed",
        "dnf" => "dropped",
        other => other,
    }
}
```

`Book`에 `pub fn normalize_status(mut self) -> Self { self.status = normalize_status(&self.status).to_string(); self }` 메서드 추가.

`crates/oxipage-ext-books/src/repo.rs` — Book 행을 반환하는 5개 지점에 적용:

| 함수 | 행 |
|---|---|
| `create` | 8 |
| `find_by_id` | 33 |
| `list` | 42 |
| `update` | 84 |
| `publish` | 169 |

예: `list`의 `q.bind(limit).fetch_all(pool).await?` 결과를 `rows.into_iter().map(Book::normalize_status).collect()`.

### 근거

- DB/마이그레이션 불변 — CHECK 제약과 신규 쓰기 경로는 이미 안전, 레거시 행만 읽기 시 정규화.
- repo 계층 단일 지점이라 모든 소비자(API 응답, `build_search_docs`, 공개 사이트 빌드, 프론트)가 정규화 값을 받음. 프론트 변경 불필요.
- 쓰기 경로(`validate_status`, CHECK)는 그대로 — 레거시 값 재저장 불가.

### 테스트

1. `normalize_status` 순수 단위 테스트: `read`→`completed`, `dnf`→`dropped`, 기존 4값/기타 입력은 불변.
2. 통합: 레거시 행을 DB에 직접 삽입 후 `list()`가 정규화 값 반환. 신규 DB는 CHECK가 레거시 삽입을 막으므로 `PRAGMA ignore_check_constraints = ON`을 테스트 풀 커넥션에 적용 후 삽입 (sqlx `after_connect`로 설정).

## 4. Item B — TMDB 키 미설정 인라인 힌트

### 배경

- 키 미설정 시 `tmdb_search`는 503 `tmdb_disabled` 반환 (`oxipage-ext-movies/src/routes.rs:227`).
- 프론트 `TmdbSearchRow`는 성공·빈 배열일 때만 드롭다운을 렌더 → 에러 시 **조용히 무반응** (`MoviesTab.tsx:479-497`).
- 원인: `jsonOrThrow`가 field 없는 에러에 `code`를 보존하지 않음 → `tmdb_disabled` 구분 불가 (`api.ts:70-88`).

### 변경

`web/src/admin/shared/api.ts`:

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

`jsonOrThrow` 비-OK 분기:

```ts
if (detail?.field) {
  throw new ApiValidationError(detail.code ?? "validation_error", detail.field, detail.message ?? "Validation failed");
}
const msg = detail?.message ?? detail ?? `HTTP ${res.status}`;
if (detail?.code) throw new ApiError(detail.code, msg);   // 신규
throw new Error(msg);                                      // code 없음(네트워크/500)은 기존 유지
```

`web/src/admin/content/MoviesTab.tsx` `TmdbSearchRow` (~479행):

- `import { ApiError } from "../shared/api"` 추가 (기존 import에 병합).
- `search.isError && (search.error as ApiError)?.code === "tmdb_disabled"`일 때 인풋 아래 인라인 노트 렌더:
  - 텍스트: "TMDB 검색 비활성 — Settings에서 TMDB Key Env를 확인하거나 `OXIPAGE_TMDB_KEY` 환경변수를 설정하세요. 제목/포스터 수동 입력은 계속 사용할 수 있습니다."
  - 링크: `<Link to={`/s/${slug}/settings`}>Settings</Link>` (SPA 라우트 `s/:slug/settings` 존재, `admin/App.tsx:148`).
- 성공/기타 에러(네트워크 등) 분기는 기존 동작 유지 — code 없는 에러는 힌트 미표시.

### 근거

- `ApiError` 추가는 비파괴 — `ApiValidationError`와 기존 `Error` 경로 불변, 기존 호출부 영향 없음.
- SettingsPage는 이미 TMDB Key Env 필드 보유 (`SettingsPage.tsx:278`) — 변경 없음.

### 검증

- `cd web && npx tsc --noEmit && bun run build` green.
- 스모크(선택): `bun run dev`, 키 미설정 상태로 Movies 탭에서 검색어 입력 → 힌트 + Settings 링크 표시 확인.

## 5. Item C — 빌드 경고 2건 제거

`crates/oxipage-ext-projects/src/repo.rs:98`와 `crates/oxipage-ext-books/src/repo.rs:50`:

```rust
// before
let sql = if let Some(s) = status {
// after
let sql = if status.is_some() {
```

2줄 기계적 수정, 동작 불변.

**검증:** `cargo check -p oxipage-ext-books -p oxipage-ext-projects`에서 경고 0건.

## 6. Item D — 비-API `/preview/*` 404 라우트

### 배경

- canonical preview URL은 `/api/console/preview/{slug}/` (프론트 `CONSOLE_BASE = "/api/console"` 고정, `web/src/admin/shared/api.ts:1`).
- top-level(비-API) `/preview/{slug}/`는 콘솔 라우터에 매치되지 않아 `.fallback(static_handler)`가 **admin.html 200**을 반환 (`oxipage-core/src/http.rs:354-363`). 등록 여부와 무관하게 SPA HTML이 나가는 죽은 네임스페이스.
- 등록 사이트의 `/api/console/preview/{slug}/`는 이미 정상(미등록 404 `site_not_found`, 무빌드 424 `build_required`, 테스트 검증됨) — 변경 대상 아님.

### 변경

`crates/oxipage-console/src/lib.rs` — 앱 조립을 테스트 가능한 함수로 추출하고 404 라우트 추가:

```rust
/// 전체 콘솔 앱 조립: core 앱 + /api/console nest + 비-API /preview/* 404 가드.
pub fn build_console_app(
    state: oxipage_core::state::AppState,
    registry: Arc<SiteRegistry>,
) -> axum::Router {
    let console = crate::router::build_console_router(registry);
    let mut app = oxipage_core::http::build_app(state);
    app = app.nest("/api/console", console);
    // canonical 경로는 /api/console/preview/{slug}/. 비-API /preview/* 는
    // SPA fallback(admin.html 200) 대신 명시적 404를 반환한다.
    app = app.route("/preview/{*rest}", get(preview_legacy_404));
    app
}

/// stateless 404 — Router<AppState>와 호환.
async fn preview_legacy_404() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "preview_missing")
}
```

- `run_console_with_extensions`(현재 173-176행 인라인 조립)는 `build_console_app(state, site_registry)` 호출로 교체.
- axum 라우트는 fallback보다 우선하므로 `/preview/...`는 404, 그 외 비-API 경로는 기존 SPA fallback 유지. `/preview`(bare)도 `{*rest}`가 빈 경로를 매치해 404.

### 테스트

`crates/oxipage-console/tests/preview_legacy_404.rs`:

- `AppState`는 `oxipage-core/tests/cache_headers.rs:10-32` 레시피 재사용(메모리 DB + dummy 확장 레지스트리 + `Config::default`).
- 레지스트리는 기존 콘솔 테스트의 `create_site_dir("Test")` + `SitesFile::add` 패턴으로 등록 사이트 1개 구성 (`build_deploy_preview.rs:39-50`).
- Assert:
  1. `GET /preview/nope/` → 404, 본문이 admin.html 아님 (`preview_missing`).
  2. `GET /preview/nope` (bare) → 404.
  3. `GET /api/console/preview/{등록슬러그}/` → 424 `build_required` — canonical 경로 불변.

## 7. 검증 명령

```bash
cargo check -p oxipage-ext-books -p oxipage-ext-projects   # 경고 0건 (Item C)
cargo build --workspace
cargo test -p oxipage-ext-books -p oxipage-console          # Item A, D 테스트
cd web && npx tsc --noEmit && bun run build                # Item B
```

새 의존성, 마이그레이션, 스키마 변경 없음.

## 8. 스코프 외 (후속 유지)

- §4 항목 1 — T23 Profile first-write 브라우저 스모크 (`expected_updated_at=""`): 코드 완료 상태, 사용자 환경에서 수동 확인.
- §4 항목 2 — 배포 실측 스모크: gh 자격 증명 필요, 사용자 환경에서 수동 확인. (대안으로 file:// bare 저장소 원격 검증이 가능하나 이번 스위트에서는 제외.)
- TMDB/알라딘 키 설정값 변경, Books 정렬/필터 UX, preview SEO 헤더 등은 이번 스위트 범위 아님.

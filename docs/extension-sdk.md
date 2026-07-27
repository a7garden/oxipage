# Oxipage Extension SDK — 새 확장 만들기 (doc/01 §1.4)

Oxipage 확장은 Cargo 워크스페이스 멤버 크레이트로, `oxipage_core::extension::Extension`
트레이트를 구현한다. 이 문서는 처음부터 확장 하나를 만드는 과정을 안내한다.

## 1. 크레이트 스캐폴드

```
crates/oxipage-ext-myfeature/
├── Cargo.toml
├── migrations/0001_init.sql
├── src/lib.rs        # Extension impl
├── src/model.rs      # sqlx::FromRow 모델 + Input/Patch
├── src/repo.rs       # DB 함수
├── src/routes.rs     # axum 핸들러
└── tests/api.rs      # 통합 테스트
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

workspace `Cargo.toml`의 `members`에 크레이트 추가.

## 2. Extension 트레이트 구현

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
        // axum 0.8: 경로 파라미터는 {slug} 형식 (NOT :slug). trailing slash 없음.
    }
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> { /* ... */ }
}
```

## 3. 핵심 규칙 (준수 필수)

1. **확장 테이블 네임스페이스**: 각 확장의 테이블은 고유 접두사/이름. 코어 마이그레이션
   러너가 확장 id별로 스키마 마이그레이션을 격리 추적.
2. **FTS5 공통 인덱스**: 발행 시점에 `oxipage_core::search::upsert(pool, "myfeature",
   &doc_id, &title, &body, lang, published_at)`. 삭제/비활성화 시 `delete` /
   `delete_extension`. (doc/02 §2.13)
3. **초안 우선 원칙**: `create`는 무조건 `published_at = NULL`. 발행은 별도 POST
   `/{id}/publish` 액션만 (doc/04 §4.3).
4. **쓰기 라우트 인증**: 핸들러 인자 `_auth: AdminAuth` → 진입 자체가 `post:write` 스코프
   필요. 발행 액션은 본문 첫 줄 `auth.require_scope("post:publish")?;`. 토큰 관리는
   `require_scope("admin")?` (doc/01 §1.8).
5. **에러**: `oxipage_core::error::ApiError` (new/validation/internal). 응답 봉투는
   `DataEnvelope<T>`.
6. **`order`/`display_order`**: `order`는 SQL 예약어 → 항상 `display_order` 사용.
7. **다른 확장 테이블 직접 JOIN 금지**: 필요하면 코어 API로 조합 (doc/02 서문).
8. **백그라운드 잡**: 외부 API 폴링/캐시 갱신이 필요하면 `background_jobs()`에서
   `ScheduledJob` 반환. 키가 없으면 조용히 비활성화 (doc/01 §1.9).
9. **외부 API 키**: `oxipage.toml [integrations]`의 환경변수 이름 → `Config::integrations`
   헬퍼(`tmdb_key()`/`aladin_key()`/`github_username()`). 평문 키를 설정 파일에 적지 말 것.

## 4. 서버 등록

`crates/oxipage-server/Cargo.toml`에 의존성 추가 + `src/lib.rs`의
`all_extensions()` vec에 `Arc::new(MyFeatureExtension)` 한 줄 추가.

## 5. 테스트 패턴

`tests/api.rs`에서 in-memory DB + `ExtensionRegistry`로 앱 조립 후 `oneshot`.
기본 케이스: 401(토큰 없음), 503(서버 토큰 없음), 422(검증), create→show→publish 흐름,
FTS upsert 검증. `oxipage-ext-blog`/`oxipage-ext-projects`가 레퍼런스.

## 6. 런타임 설치 확장 (알려진 한계, doc/01 §1.4)

v1은 컴파일 타임 정적 링크만 지원. WASM 컴포넌트 기반 런타임 로딩은 Phase 5 스파이크
대상이며, **런타임 설치 확장은 CLI 서브커맨드를 추가할 수 없다** (clap 정적 링크 필요).
서드파티 확장은 API/웹으로만 다룬다.

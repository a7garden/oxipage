# Site-Picker Unified Console — Design

> 2026-07-30. v2 SSG 모델 위에 "여러 oxipage 사이트를 한 콘솔에서 전환·편집"하는 통합 관리 UX를 설계한다.

## 0. 확정 결정

| # | 결정 | 근거 |
|---|---|---|
| D1 | 사이트 = **별도 oxipage 프로젝트 디렉토리** (각자 oxipage.toml / oxipage.db) | v2 SSG §2.2 "테이블 스키마 불변" 위배 없음. 콘텐츠 격리 단위 = 디렉토리 |
| D2 | 서버 토폴로지: **:8787 = 관리 콘솔 전용** · 퍼블릭 미리보기는 `console --preview` · :8788 폐기 | v2 SSG §2 "관리는 동적으로, 배포는 정적으로". `:8788`은 `admin/mod.rs:66` "v1 호환" 레거시 |
| D3 | GUI = **admin-web SPA 코드를 web/src/admin/ 으로 흡수** 후 :8787에서 서빙 | 단일 Vite 프로젝트, 사이트 피커·콘솔·위저드를 같은 셸에 |
| D4 | v2.0 범위 = **단일 :8787 인스턴스 통합** | 활성 사이트 한 번에 하나. 다중 인스턴스 동시 실행은 v2.1+ |
| D5 | 위저드 1단계에서 사이트 디렉토리 신설/등록 → 완료 시그널에 콘솔로 | "위저드 시작 = 내 사이트 시작" |
| D6 | 위저드 종료 후 헤더에 "콘솔로" 버튼 없음 | 콘솔이 곧 메인 서피스가 됨. 톱니바퀴도 제거 |
| D7 | 원격(배포된 정적) 사이트 = 콘솔 사이트 피커 **미포함**. CLI `site list`에는 보존 | 정적 산출물은 콘솔이 편집할 수 없음. UI에 거짓 기대 생성하지 않음 |

## 1. 문제 진단 (현재 코드 증거)

| 컴포넌트 | 위치 | 진단 |
|---|---|---|
| 메인 콘솔 | `crates/oxipage-console/src/lib.rs::run_console_with_extensions` (`:8787`) | 웹 SPA + `/api/console/*` 서빙. **퍼블릭 사이트 + 위저드를 함께 서빙** — v2 SSG "퍼블릭은 정적" 원칙 위배 |
| 관리 콘솔 (레거시) | `crates/oxipage-console/src/admin/mod.rs::run_admin` (`:8788`) | `mod.rs:66` 자체 코멘트 "v1 호환: oxipage_console::run_admin 별칭". proxy.rs/원격 사이트 모드 v2에서 제거됨. 미구현 잔재 |
| 위저드 종료 후 | `web/src/setup/SetupWizard.tsx` → `web/src/App.tsx` Shell | `/setup` 종료 시 `/` (퍼블릭 로비)로. 헤더 톱니바퀴 클릭이 `:8788` 새 탭 → 사용자 불만의 직접 원인 |
| SiteSwitcher | `admin-web/src/shell/SiteSwitcher.tsx` | `useSite()` 컨텍스트의 "active site" — admin-web `api.ts:25-28` 주석이 "v2: display-only in v2"라고 명시. 실제로 작동은 표시만 |
| 사이트 식별 단위 | `sites.toml` (CLI 메타) vs 단일 oxipage.toml (서버 시작) | 두 모델이 동시에 존재. 콘솔이 둘을 연결하지 않음 |
| 멀티사이트 빌드 | ssg-design §12 = `oxipage build --site <name>` 정의, but 콘솔 피커 책임 0 | §12가 정의한 사이트 = 확장 서브셋 (콘텐츠 격리 X). 우리가 원하는 것은 디렉토리 격리 — §2.2 "테이블 스키마 불변" 내에서만 가능 |

## 2. 목표

- G1. 사용자가 `cargo install oxipage` → `oxipage console` 한 줄로 자기 사이트 콘솔에 진입. 첫 부팅이면 위저드 → 자동 콘솔 셸.
- G2. 콘솔 사이트 피커가 등록된 사이트들을 마운트된 라우트 prefix `/s/:slug`와 매핑. URL에서 slug가 곧 어느 사이트인지를 결정.
- G3. 콘솔에서 게시물 작성·발행, 확장 토글, 테마 변경, 빌드/배포 트리거 — `sites.toml`의 `default_site`(= 활성 사이트 1개)가 디폴트지만, 백엔드는 모든 사이트를 항상 마운트하므로 `/s/<other>/blog` 같은 직접 접근도 가능.
- G4. 퍼블릭 미리보기는 `oxipage console --preview --site <slug>`로 같은 :8787의 `/preview/:slug/*`에서 그 사이트의 `out/` 서빙. 셸은 관리 콘솔 모드 유지.
- G5. v2 SSG 모델(공유 DB 금지, 런타임 의존 최소화)과의 정합. "활성 사이트 = 백엔드 글로벌 상태"라는 이전 v0의 가정은 폐기(§5 참조 — 사이트별 동시 라우팅 가능).

## 3. 아키텍처

```
┌──────────────────────────────────────────────────────────────────────┐
│  oxipage console                    ← 단일 통합 관리 진입점             │
│                                                                      │
│  :8787 (127.0.0.1 강제 — 보안 경계, auth 없음)                          │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Admin SPA (web/src/admin/ 이주)                              │    │
│  │   /setup/*       ── SetupGuard 리다이렉트 (첫 부팅)            │    │
│  │   /              ── SiteShell: 사이트 피커 + 사이트 목록       │    │
│  │   /sites         ── CRUD                                      │    │
│  │   /sites/new     ── 신설                                       │    │
│  │   /s/:slug/...   ── 사이트 콘텐츠/대시보드 (slug = 사이트 식별) │    │
│  │   /preview/:slug/* ── --preview 활성 시 out/ 정적 서빙        │    │
│  └──────────────────────────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │  Axum Backend                                                  │    │
│  │   /api/console/setup/*    loopback-only (doc/13)               │    │
│  │   /api/console/sites/*    sites.toml CRUD                      │    │
│  │   /api/console/sites/default  default_site read/write          │    │
│  │   /api/console/s/:slug/<ext>/...  slug-prefixed 핸들러 (sites-runtime │    │
│  │                                   가 db_for(slug)로 풀 해결)   │    │
│  │   /api/console/s/:slug/build, /deploy                          │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  --preview 모드: /preview/:slug/* 가 slug별 out/ 정적 서빙 (관리 셸 유지) │
└──────────────────────────────────────────────────────────────────────┘
```

핵심: 사이트 컨텍스트는 백엔드 글로벌 상태가 아니라 **startup에서 사이트 레지스트리로 모두 로드**되고, 라우트 prefix(`/s/:slug`)가 곧 사이트 식별자. 동시 여러 사이트 라우트가 항상 마운트 → swap·리마운트 없음.

## 4. 데이터 모델

### 4.1 `~/.config/oxipage/sites.toml`

```toml
default_site = "blog"

[sites.blog]
path = "~/oxipage/blog"

[sites.portfolio]
path = "~/oxipage/portfolio"
```

- v1의 `endpoint`/`token` 폐기. 사이트 = **로컬 디렉토리 경로** only.
- `path`는 canonicalize 후 저장(해시 안전). 미존재 디렉토리는 검증 단계에서 거절.
- 권한 0600 유지 (현재 구현 그대로).

### 4.2 사이트 디렉토리 구조

```
<path>/
├── oxipage.toml           # 사이트별 설정
├── data/
│   ├── oxipage.db         # 사이트별 DB
│   ├── oxipage.db-wal
│   └── media/
└── out/                   # oxipage build 결과 (정적 산출물)
```

위저드 `create-site`이 이 구조를 한번에 생성. `oxipage.toml` 시드 = 현재 `oxipage.toml.example` 단순화 버전.

### 4.3 런타임 SiteContext

### 4.3 런타임 — SiteRegistry

```rust
pub struct SiteContext {
    pub slug: String,
    pub path: PathBuf,
    pub config: Arc<Config>,
    pub db: sqlx::SqlitePool,
    pub registry: Arc<ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}

pub struct SiteRegistry {
    sites: Arc<RwLock<HashMap<String, Arc<SiteContext>>>>,  // startup에 모두 로드
    sites_file: Arc<RwLock<SitesFile>>,                    // CRUD 시에만 잠금
}

pub struct AppState {
    pub sites: Arc<SiteRegistry>,
    pub csrf_token: String,
}

impl SiteRegistry {
    pub async fn db_for(&self, slug: &str) -> Option<sqlx::SqlitePool> {
        self.sites.read().await.get(slug).map(|c| c.db.clone())
    }
    pub async fn ctx_for(&self, slug: &str) -> Option<Arc<SiteContext>> {
        self.sites.read().await.get(slug).cloned()
    }
    pub async fn default_slug(&self) -> Option<String> {
        let sf = self.sites_file.read().await;
        sf.default_site.clone().or_else(|| sf.sites.keys().next().cloned())
    }
}
```

- **사이트 컨텍스트 동시성**: 모든 사이트가 startup 시 한 번 로드되어 레지스트리에 들어간다. swap·리마운트가 없다. 라우트 prefix가 slug로 곧장 풀을 해결한다.
- **사이트 추가/삭제**: 사용자가 `/sites/new` 또는 `POST /api/console/sites`로 신규 사이트를 등록하면 핸들러가 (a) 새 `SiteContext`를 빌드(동기, 다음 await 전 완료) → (b) 레지스트리 맵에 insert → (c) 라우터는 빌드 타임에 모든 사이트에 대해 마운트되어 있으므로 새로 추가된 사이트는 *다음 부팅부터* 라우트 등장. (단, `active` 같은 prefix 자체는 변하지 않음.) 신규 사이트는 setup 흐름으로 등록 → 사용자는 한 번 재시작해서 그 사이트 라우트를 활성화하거나, 부팅 시 동적 라우팅을 별도 구현한다 — v2.0에서는 **재시작 필요** (범위 단순화).
- **빌드/배포 트리거**: `POST /api/console/s/:slug/build` 또는 `POST /api/console/s/:slug/deploy` — slug를 Path로 받아 `ctx_for(slug)`가 풀+builders를 꺼낸다.
- **active 개념**: 백엔드 글로벌 상태가 아니다. 프론트엔드/UX(사이트 피커의 chip 강조)와 CLI 디폴트(`OXIPAGE_SITE` env, `default_site`)에만 사용된다. 어느 사이트인지 헷갈리는 핸들러는 slug를 Path로 받으면 그만이다.

## 5. 라우터 디스패치

### 5.1 사이트 prefix 라우팅

`/api/console/s/:slug/<ext>/...` — `:slug`가 `sites.toml`에 등록되어 있어야 통과. 모르는 slug = 404 `unregistered_site`. 미들웨어가 `state.db_for(&slug)`로 풀을 해결하고 `Request::extensions()`에 주입한다.

라우트 빌드 흐름 (startup 시, 등록된 각 사이트에 대해):

```rust
for (slug, ctx) in registry.iter() {
    for ext in ctx.registry.iter() {
        if ext.route_dispatcher().is_some() { continue; } // WASM은 api_fallback로
        let scoped = ext.routes()
            .layer(axum::middleware::from_fn_with_state(
                state.clone(), db_for_middleware)) // Request.extensions 에 Pool 주입
            .layer(axum::middleware::from_fn_with_state(
                state.clone(), site_exists_middleware));
        api = api.nest(&format!("/s/{slug}/{}", ext.id()), scoped);
    }
}
```

원칙: `state.db` 한 개라는 단일 사이트 전제를 **request-scope pool 주입**으로 일반화한다. 핸들러는 v1 형식 `state.db` → 신규 형식 `req.extensions().get::<SiteScopedDb>()`로 한 줄 교체. 각 확장 crate에 **개별 PR**로 적용 — §13 작업 분할 T8과 정렬.

### 5.2 어댑터 레이어 — `SiteScopedDb`

```rust
#[derive(Clone)]
pub struct SiteScopedDb {
    db: sqlx::SqlitePool,
    // 향후 extension이 라우트 빌더에 필요한 다른 per-site 자원이 늘어나면 확장
}

async fn db_for_middleware<B>(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, ApiError> {
    let ctx = state.sites.ctx_for(&slug).await
        .ok_or_else(|| ApiError::NotFound("site_not_found"))?;
    req.extensions_mut().insert(SiteScopedDb { db: ctx.db.clone() });
    req.extensions_mut().insert(ctx); // 확장 핸들러가 SiteContext도 꺼낼 수 있게
    Ok(next.run(req).await)
}
```

**v1 핸들러를 새 모델로 옮기는 작업**(필요한 변경 단위):

```rust
// before
async fn list(state: State<AppState>) -> ... { let db = &state.db; ... }
// after
async fn list(pool: Extension<SiteScopedDb>) -> ... { let db = &pool.db; ... }
```

`State<AppState>`는 라우터-글로벌이라 per-request db 주입과 공존할 수 없다. 따라서 기존 핸들러는 위 한 줄 교체가 필요하고, `state.config` / `state.registry` 등 다른 글로벌 자원이 코드에서 사라지지는 않음 — `state.db`만 `SiteScopedDb.db`로 옮기면 된다.

### 5.3 활성 사이트 / 디폴트

`/api/console/sites/default` (GET/PUT) — sites.toml의 `default_site`만 변경. 백엔드 라우트 재구축 X. 프론트 사이트 피커 chip과 `/` 진입 시 어디로 리다이렉트할지를 위해 사용.

## 6. 위저드 통합

### 6.0 setup과 사이트 등록의 관계

§4.3의 분리로 setup_state는 사이트별 DB에 둘 수 없다 — 사이트 0개 상태에는 사이트 DB가 아예 존재하지 않는다. 따라서:

- **setup_state = 콘솔 단위 DB** (`~/.config/oxipage/console.db`, sites.toml과 같은 디렉토리). setup_state 테이블 + 콘솔 wide 메타만 보관. 사이트 콘텐츠/스키마는 사이트별 DB.
- **`is_setup_needed`** = `console.db`의 `setup_state.setup_completed_at IS NULL`. sites.toml 무관.
- 사이트 0개 → `is_setup_needed=true`로 파생되는 게 아니라 **그 자체가 setup 미완료 신호**. 둘은 사실상 같은 것 — 사이트를 등록하는 행위 자체가 setup 종결.
- site 신규 추가는 `POST /api/console/sites`로 — 첫 호출이면 setup_state 마저 동시 시드. 이후 추가 시에는 `setup_completed_at`가 이미 있는 상태에서 한 사이트만 추가.
- `OXIPAGE_CONFIG`로 사이트별 oxipage.toml을 가리키던 v0 흐름은 폐기. 콘솔은 sites.toml만 본다.

§6.0의 "setup = 콘솔 단위 한 번" 불변. §6.1의 시나리오별 의미는 다음과 같이 단순화:

| 부팅 조건 | 동작 |
|---|---|
| console.db 없거나 setup_state.setup_completed_at IS NULL (대개 사이트 0개 동반) | `:8787/setup` 위저드 모드. Step 1에서 사이트 디렉토리 신설/등록 후 완성 |
| setup_state 완성 + 사이트 0개 | `/sites`에서 "사이트를 추가하세요" 안내만. setup 재실행은 별도 액션(데이터 destructive하지 않게 의도적 분리) |
| setup_state 완성 + 사이트 ≥1, default_site 부재 | `/sites`에서 default_site 지정 UI |
| setup_state 완성 + default_site 유효 | 사이트 컨텍스트 자동 빌드, `/s/<default>/` 진입 |

### 6.1 첫 부팅 흐름 (console.db 없음 + 사이트 0개)

```
:8787 부팅
  ├── console.db 없음 (또는 setup_state.setup_completed_at IS NULL)
  ├── sites.toml 비어있음
  └── 브라우저 자동 오픈 :8787/setup
       SetupWizard
         Step 1 "Site" — 강제:
           ○ 새 사이트 디렉토리 만들기
              → 입력 경로 (e.g. ~/oxipage/blog)
              → POST /api/console/setup/create-site {path:"~/oxipage/blog"}
                 → 디렉토리 신설, oxipage.toml 시드
                 → sites.toml에 slug 등록, default_site = slug
                 → 응답 {slug:"blog", path:"~/oxipage/blog"}
           ○ 기존 디렉토리 등록
              → 경로 입력 (oxipage.toml 존재 검증)
              → sites.toml에 slug 기록
         Step 2 (확장) … Step N (theme) — 방금 만든 사이트 컨텍스트에 적용
         Step Done:
           → console.db의 setup_state.setup_completed_at = now()
           → 사이트 컨텍스트 빌드 후 브라우저 자동 오픈 :8787/s/<slug>/
```

### 6.2 정상 부팅 (setup 완료 + default_site 유효)

```
console.db.setup_state.setup_completed_at 존재 + sites.toml.default_site 유효
→ 레지스트리에서 default_site SiteContext 사용
→ 브라우저 자동 오픈 :8787/s/<default>/
```

### 6.3 사이트 0개 (이미 setup 완료된 콘솔)

```
console.db.setup_state.setup_completed_at 존재 + sites.toml 비어있음
→ /sites 페이지: "등록된 사이트가 없습니다." + 새 사이트 만들기 CTA
→ setup 재실행은 별도 액션(데이터 destructive하지 않게 의도적 분리)
```

### 6.4 헤더 정리

- `:8787`은 관리 콘솔이므로 퍼블릭 헤더(태그라인·검색·language 토글) 노출 안 함.
- 톱니바퀴 = 제거. 콘솔 셸 자체가 곧 관리 진입점.

## 7. 백엔드 변경

### 7.1 제거

| 위치 | 처리 |
|---|---|
| `crates/oxipage-console/src/admin/mod.rs::run_admin` | 삭제. `pub fn run_admin` 사용처 없음 확인 후 |
| `:8788` 포트 / `OXIPAGE_ADMIN_PORT` env | 무시 + deprecation 로그 |
| `crates/oxipage-console/src/admin/proxy.rs` (있는 경우) | 이미 v2에서 제거됨. 잔재 확인만 |
| `admin-web/` 디렉토리 | web/src/admin/ 으로 이관 후 삭제 |

### 7.2 신규 모듈

- `crates/oxipage-console/src/sites_runtime.rs`
  - `SiteRegistry` (startup 시 sites.toml + console.db로 모든 사이트 SiteContext 일괄 로드)
  - `SiteLoader::load(slug, path) -> SiteContext` (디스크 검증 + DB connect + extension migrations)
  - `db_for(slug) -> Pool`, `ctx_for(slug) -> Arc<SiteContext>` (per-request 풀 해결)
  - `add_site(path)` / `remove_site(slug)` / `set_default(slug)` (sites.toml CRUD; 레지스트리 reload는 부팅 후)
- `crates/oxipage-console/src/console_state.rs`
  - `console.db` 연결 + 마이그레이션 (setup_state, 콘솔 메타)
  - `is_setup_needed()` 쿼리

### 7.3 setup 모듈 추가 핸들러

```
POST /api/console/setup/create-site
  body: {path: "~/oxipage/blog"}
  → {data: {slug: "blog", path: ".../oxipage/blog"}}
```

(loopback-only + setup-완료 후 410 게이트 그대로)

### 7.4 사이트 라우트 헬퍼

```rust
// crates/oxipage-console/src/router.rs
pub fn build_console_router(state: AppState) -> Router {
    let mut r = Router::new()
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/:slug", put(update_site).delete(remove_site))
        .route("/sites/default", get(get_default).put(set_default))
        .route("/setup/*", setup::router())           // loopback-only
        .fallback(static_admin_handler);

    // 등록 사이트별로 확장 라우트 마운트
    for (slug, _ctx) in state.sites.iter() {
        let scoped = Router::new()
            .route("/build", post(build_handler_for(slug)))
            .route("/deploy", post(deploy_handler_for(slug)))
            // 확장 라우트:
            // nest("/{ext_id}", ext.routes())
            //   .layer(db_for_middleware(slug))
            .layer(Extension(SiteScopedDb { db: ... }));
        r = r.nest(&format!("/s/{slug}"), scoped);
    }

    r.layer(rate_limit).with_state(state)
}
```

## 8. 프론트 (admin-web → web/src/admin 이주)

### 8.1 라우트 트리

```
<BrowserRouter>
  <Routes>
    <Route path="/setup/*" element={<SetupGuard><SetupWizard/></SetupGuard>}/>
    <Route path="/" element={<SetupGuard><SiteShell/></SetupGuard>}>
      <Route index element={<PickerOrDashboard/>}/>     {/* 사이트 없으면 피커, 있으면 활성 대시보드 */}
      <Route path="sites" element={<SitesPage/>}/>
      <Route path="sites/new" element={<NewSiteWizardPage/>}/>
      <Route path="s/:slug/*" element={<ActiveSiteShell/>}>
        <Route index element={<DashboardPage/>}/>
        <Route path="content/blog" element={<BlogListPage/>}/>
        <Route path="content/blog/new" element={<BlogEditorPage/>}/>
        <Route path="content/blog/:slug" element={<BlogEditorPage/>}/>
        <Route path="content/projects" element={<ProjectsListPage/>}/>
        <Route path="content/links" element={<LinksPage/>}/>
        <Route path="content/movies" element={<MoviesPage/>}/>
        <Route path="content/books" element={<BooksPage/>}/>
        <Route path="themes" element={<ThemesPage/>}/>
        <Route path="extensions" element={<ExtensionsPage/>}/>
        <Route path="settings" element={<SettingsPage/>}/>
        <Route path="build" element={<BuildPage/>}/>
        <Route path="deploy" element={<DeployPage/>}/>
      </Route>
    </Route>
  </Routes>
</BrowserRouter>
```

### 8.2 셸

- **SiteSwitcher** = 현재 admin-web의 그것을 의미 변경: `setActiveSite`이 slug를 등록 → 페이지 새로고침. 현재 사이트 chip + 다른 사이트 chip + `[+ 새 사이트]`.
- **공개 사이트 헤더 잔재 제거**: web/src/App.tsx의 톱니바퀴, 검색 버튼, 언어 토글 제거 (이 셸은 관리용).

### 8.3 코드 이주 매핑

| admin-web 위치 | web/src/admin/ 안 위치 |
|---|---|
| `admin-web/src/shell/AdminShell.tsx` | `web/src/admin/shell/SiteShell.tsx` |
| `admin-web/src/shell/SiteSwitcher.tsx` | `web/src/admin/shell/SiteSwitcher.tsx` |
| `admin-web/src/dashboard/` | `web/src/admin/dashboard/` |
| `admin-web/src/content/` | `web/src/admin/content/` |
| `admin-web/src/extensions/` | `web/src/admin/extensions/` |
| `admin-web/src/themes/` | `web/src/admin/themes/` |
| `admin-web/src/settings/` | `web/src/admin/settings/` |
| `admin-web/src/shared/` | `web/src/admin/shared/` (또는 web/src/shared 재사용) |
| `admin-web/src/main.tsx` | `web/src/admin/main.tsx` |

`admin-web/embedded-spa` rust-embed 타깃 → `web/embedded-spa/` 통합. `vite.config.ts` 두 개 → `web/`만.

### 8.4 API base URL 변경

`admin-web/src/shared/api.ts`의 `ADMIN_BASE = "/api/admin"` → `CONSOLE_BASE = "/api/console"`. 이후 admin-web과 web/API가 같은 호스트(:8787) → cross-origin 사라짐.

## 9. CLI 변경

| 명령 | 변경 |
|---|---|
| `oxipage site add` | 인자: `<name> --path <dir>` (endpoint/token 폐기) |
| `oxipage site list` | path + "active" 표시 |
| `oxipage site use` | default_site 변경 (그대로 의미 유지) |
| `oxipage site migrate` | v1 site.toml (endpoint+token) 발견 시 한 번 변환 권장 — 자동 변경 X |
| `oxipage site rm` | 미변경 |
| `oxipage console` | --project <dir> 플래그 추가 (기본: default_site 또는 현재 디렉토리). 첫 부팅 시 /setup 자동 오픈 (현재 동작 유지). 사이트 0개면 /sites 페이지 |
| `oxipage console --preview` | 활성 사이트 out/을 /preview로 서빙. 셸은 관리 콘솔 모드 유지 |
| `oxipage build --site <slug>` | 활성 사이트만 빌드. multi-build는 v2.1+에서 |
| `oxipage deploy` | 활성 사이트만 배포 (--site는 옵션, 현재 그대로의 동작) |
| `oxipage status` | sites.toml 요약 + active 표시 |

## 10. preview 동작

```
oxipage console --preview --site blog
  → :8787 (관리 셸 + /preview/* 경로만 out/ 서빙)
  → 사용자는 콘솔 셸에서 활성 사이트 콘텐츠를 편집하면서 같은 창에서 /preview로 결과 확인 가능
```

또는 `--preview-only`로 :8787이 그냥 out/만 서빙 (관리 셸 off). --preview는 두 모드의 별칭이 아니라 **추가 모드**.

## 11. 보안 경계

- `127.0.0.1` 강제 바인딩 (이미 doc/13 §13.5 loopback 게이트). 원격 바인드 시도 = 거부 + 경고.
- setup API는 loopback-only + setup-완료 후 410 Gone (현재 그대로).
- 사이트 디렉토리 canonicalize 후 path traversal 방지.
- 토큰 / Bearer / PAT: v2.0 범위 밖 (현재 모델은 auth 없음). 콘솔 셸에 admin 비번 추가하는 것은 별도 PR.

## 12. 마이그레이션·호환성

- `OXIPAGE_ADMIN_PORT` 무시 (경고 로그).
- 구 v1 사이트 프로필: `endpoint`만 있고 `path` 없음 → 마이그레이션 도구 실행 안내. **자동 변경 X** (사용자 의도 확인).
- `oxipage.toml` 자체 v1 → v2는 별도 마이그레이션 가이드 (이 spec 범위 밖).
- 백워드: 구 사이트 endpoint/token은 CLI 쪽에 일시 보존. 콘솔 사이트 피커는 표시 X.

## 13. 작업 분할 (구현 단계)
1. **T1: sites.toml 스키마 (`path` 필드만)** + `SiteRegistry` 골격 (load/db_for/ctx_for) — SitesFile 단순 마이그레이션 + sites.toml CRUD API. 부팅 시 등록된 사이트 모두 일괄 로드 (TDD).
2. **T2: console.db + setup_state 이전** — `console.db` 연결·마이그레이션, `is_setup_needed` 출처 교체. setup 모드 진입·종료 의미를 새 출처에 맞춰 정렬 (T1 의존).
3. **T3: 사이트 라우트 빌더 + `SiteScopedDb` 미들웨어** — 루트 빌더가 등록 사이트마다 `/s/<slug>/<ext>`를 nest. `db_for_middleware`가 Request.extensions에 풀 주입. 모르는 slug = 404 (T1 의존).
4. **T4: 첫 번째 확장 PR (blog)** — blog 핸들러의 `state.db` → `Extension<SiteScopedDb>.db` 일괄 교체 + `s/<slug>/blog` prefix 라우트 정상 응답 (T3 의존). 다른 확장은 후속 PR.
5. **T5: admin-web → web/src/admin/ 이주 + 셸 라우트 트리 재구성** — admin/ 폴더 신설, `/s/:slug/*` 라우트 + `/sites` CRUD 페이지 + 톱니바퀴 제거 (T3 의존).
6. **T6: 위저드 Step 1 사이트 디렉토리 결정 + `create-site` 핸들러 + 완료 후 `/s/<slug>/`** — UX 통합. setup과 사이트 등록을 한 흐름에 묶음 (T2 의존).
7. **T7: `:8788` / `run_admin` / `OXIPAGE_ADMIN_PORT` 제거** — T1-T6 머지 후. 한 사이클 동안 v1 alias만 보관하고 deprecation.
8. **T8: 나머지 확장 PR (projects / links / movies / books / scrap / activity / novels / profile 순서대로)** — 각 확장에서 T4와 같은 한 줄 교체 + 라우트 prefix 통과 검증. 가장 부피 큰 작업 — 페이지 분할로 PR 압박 완화.
9. **T9: 콘솔 빌드/배포 트리거 + `/preview/:slug/*` 라우트** — 사용자가 만든 콘텐츠를 빌드/배포 흐름과 연결, `--preview --site` 플래그 통합.

각 Task는 TDD(테스트 먼저) + 단계별 검증 (`cargo test --workspace`, `bun run build`, browser smoke).

## 14. 향후 확장 (out of scope, v2.1+)

- 사이트마다 다른 포트로 동시 인스턴스 실행 — `port` 필드 도입.
- 다중 사이트 빌드 (`oxipage build --all`, `out/<slug>/`).
- OS 키체인 통합 (`OXIPAGE_TOKEN` 대체).
- 원격 사이트(배포된 정적 호스트) 점검 — `site health` 같은 read-only 모드.
- 콘솔 셸 안 admin 인증 (PAT 체계와 함께).

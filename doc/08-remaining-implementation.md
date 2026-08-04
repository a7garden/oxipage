# 8장 — 남은 구현 (Remaining Implementation)

> 2026-07-28 세션에서 Phase 전 영역 구현 완료 + 빌드 클린. 본 문서는
> **이력 보존** 및 **외부 자격증명/수동 실측이 필요한 잔여 항목**을 기록합니다.
> workspace 160+ tests 전부 통과, clippy -D warnings 클린 상태.

| Phase | 완료 | 비고 |
|---|---|---|
| Foundation | ✅ 완료 | FTS5·Rating·scheduler·Extension trait·IntegrationsConfig·CLI 스캐폴드 |
| Phase 1 | ✅ 완료 | blog·projects·links + CLI + 프론트 lazy route |
| Phase 2 | ✅ 완료 | novels·movies·books·scraps·activity + 별점 + background_jobs |
| Phase 3 | ✅ 완료 | 로비 3모드·LobbyConfig API·/search UI·SSR 스냅샷·WCAG AA |
| Phase 4 | ✅ 완료 | PAT 스코프·레이트리밋·OpenAPI/Swagger·SKILL.md |
| Phase 5 | ✅ 완료 | deploy·LICENSE·SDK·레지스트리·starter·WASM v2 |
| 다중 사이트 (doc/09) | ✅ 완료 | sites.toml CRUD, CLI site 명령, endpoint/token 해상도 |
| CLI 확장성 (doc/11) | ✅ 완료 | CliCommand trait, Dynamic subcommand, 5개 확장 CLI, 서버 위임 |
| 관리 콘솔 (doc/12) | ✅ 완료 | oxibuilder-admin crate, admin-web SPA, proxy/themes/sites API |
| 빌드 | ✅ 클린 | `cargo test --workspace` 163 tests pass, clippy -D warnings, web build |
| **배포 & 접근성** | ⏳ 수동/외부 | 배포 스모크(자격증명), 브라우저 접근성 실측(VoiceOver/키보드) |
**검증 상태:** `cargo test --workspace` 163 tests ok · `cargo clippy --workspace --all-targets -- -D warnings` clean ·
`cd web && bun run build` ok · `cd admin-web && bun run build` ok · SSR end-to-end verified.

## 8.2 SSR 스냅샷 확장 연결 (Phase 3, ✅ 완료 — 2026-07-27)

**상태:** ✅ 완료. `crates/oxibuilder-core/src/snapshot.rs`에 `render_with_index()`/`write_snapshot_for()` 추가, `crates/oxibuilder-core/src/http.rs`에 `spa_index_html()` 추가. 7개 확장(blog/projects/novels×2/movies/books/scraps)의 publish 핸들러 + 7개 delete 핸들러에 연결. SSR end-to-end 스모크로 검증 완료.

**남은 작업:**

1. **`http::spa_index_html()` 추가** — `crates/oxibuilder-core/src/http.rs`의 `Assets`
   에서 `index.html`을 읽어 `Option<String>` 반환(pub). SPA 진입 스크립트 해시
   파일명을 스냅샷에 주입하기 위함.
2. **`snapshot::render_with_index(index_html, data)` 추가** — `snapshot.rs`에 새 함수.
   `index.html`의 `<title>…</title>` 교체 + `</head>` 앞에 OG 메타/canonical 삽입 +
   `<div id="root"></div>`에 `<main data-snapshot="true">{본문}</main>` 주입.
   브라우저는 같은 HTML을 받아 React가 `#root` 하이드레이트(기존 `main` 교체).
3. **각 확장 publish 핸들러에 연결** — 7곳:
   - `oxibuilder-ext-blog/src/routes.rs::publish`
   - `oxibuilder-ext-projects/src/routes.rs::publish`
   - `oxibuilder-ext-novels/src/routes.rs::publish_novel` + `publish_chapter`
   - `oxibuilder-ext-movies/src/routes.rs::publish`
   - `oxibuilder-ext-books/src/routes.rs::publish`
   - `oxibuilder-ext-scraps/src/routes.rs::publish`
   
   각 publish에서 reindex 후:
   ```rust
   if let Some(index_html) = oxibuilder_core::http::spa_index_html() {
       let data = oxibuilder_core::snapshot::SnapshotData {
           title: post.title.clone(),
           description: post.body.chars().take(200).collect(),
           canonical_url: format!("{}/blog/{}", state.config.site.base_url, post.slug),
           og_image: None,
           body_markdown: post.body.clone(),
           lang: post.lang.clone(),
       };
       let html = oxibuilder_core::snapshot::render_with_index(&index_html, &data);
       let _ = oxibuilder_core::snapshot::write_snapshot(
           &state, &format!("/blog/{}", post.slug), &html
       ).await; // best-effort
   }
   ```
4. **삭제 시 스냅샷 제거** — 각 delete 핸들러에서 `snapshot::remove_snapshot` (선택).
5. **테스트** — publish 후 `data/snapshots/blog_*.html` 파일 생성 확인. 테스트
   격리를 위해 `Config::default()`의 `data_dir = "data"` 대신 tempdir 사용 권장
   (테스트 helper에서 `data_dir`을 `tempfile::tempdir()`로 오버라이드).

**편차 (doc/01 §1.6 대비):** Askama 대신 수동 `format!`/`replacen` 템플릿 —
의존성 절약, 1인 사이트 규모에서 충분.

## 8.3 WCAG AA 대비 재검증 (Phase 3, ✅ 완료 — 2026-07-27)

**상태:** ✅ 완료. OKLCH → sRGB → WCAG 상대 휘도 변환 직접 구현(eval 셀) 후
라이트/다크 양쪽 주요 텍스트·아이콘 쌍 측정. 미달 2건 발견 → 토큰 조정 후 재측정 통과.
상세 결과는 [`docs/accessibility.md`](../docs/accessibility.md) 참조.

**조정 내역:**
- `--p-gold-600`: `oklch(68% 0.15 85)` → `oklch(55% 0.12 65)` (라이트 별점 fill.
  H=85 유지 시 L<60%에서 gamut 밖 튐 → H=65로 gamut 안전)
- 다크 `--color-text-tertiary`: `var(--p-neutral-500)` (55% L) → `oklch(65% 0.012 265)`
  (다크 surface-raised에서 3.58:1 → 5.34:1)

**알려진 한계:** 라이트 gold-600 / surface-50 (card 배경) = 4.34:1로 미세 미달.
별점 컴포넌트가 card 위에 놓이지 않는다는 설계 전제. 추후 card 위 별점 도입 시 재검토.

## 8.4 WASM 런타임 적재 v2 (Phase 5, ✅ 완료 — 2026-07-28)

**상태:** ✅ v2 완료. v1 스파이크의 한계 3종(store 재사용, HTTP 라우트, 핫 리로드)를
해결하고 §7의 fuel 제한·DB/HTTP capability·ed25519 서명 검증을 구현. 상세는
[`docs/wasm-spike.md`](../docs/wasm-spike.md).

**v2 구현 내역:**

1. **Store 재사용** — lobby Store+Instance를 `Mutex`로 캐싱. 매 호출 재인스턴스화 제거.
2. **Fuel 제한** — `consume_fuel(true)` + `set_fuel(10M/request, 1M/lobby)`. CPU DoS 방지.
3. **HTTP 라우트** — route manifest ABI + `handle_request` + `RouteDispatcher` trait.
   폴백 핸들러가 동적 디스패치. 데모: GET /info, /time, /db.
4. **DB/HTTP capability** — `host_db_query`(SELECT 한정, JSON 반환), `host_http_get`.
5. **핫 리로드** — `ExtensionRegistry` RwLock + `WasmLoader` trait + install 라이브 활성화.
6. **서명 검증** — ed25519 (`ed25519-dalek`). registry `signature` 필드로 `.wasm` 위변조 탐지.
7. **core trait 확장** — `RouteDispatcher`/`WasmLoader` trait 추가. 컴파일 확장은 영향 없음.

**v1 구현 내역 (유지):**

1. **`wasmtime` v33** — workspace deps. 코어 WASM 모듈 + 수동 ABI.
2. **`oxibuilder-wasm` 호스트** — `WasmExtensionAdapter: impl Extension + RouteDispatcher`.
3. **`oxibuilder-ext-wasm-demo`** — `no_std` cdylib. lobby 카드 + 3개 라우트.
4. **`oxibuilder extension install`** — `POST /extensions/install` → 서명 검증 → 파일/DB/활성화.
5. **서버 통합(feature `wasm`)** — 부팅 시 `data/extensions/*.wasm` + install 시 라이브 등록.

**의도적 편차 (유지):**
- 코어 WASM 모듈 + 수동 ABI (`cargo-component`/`wit-bindgen` 미설치). 컴포넌트 모델은 future work.
- `Extension::id(&self) -> &str` 축소 유지.

**검증:** `cargo test -p oxibuilder-wasm`(6 tests) · install round-trip(서명 검증 포함) ·
`cargo clippy --workspace --all-targets -- -D warnings` 클린 ·
`cargo clippy -p oxibuilder-console --features wasm -- -D warnings` 클린.

**남은 과제:** 컴포넌트 모델(WIT), 메모리 상한, 다중 서명 키, 원격 레지스트리, 핫 언로드.

## 8.5 배포 스모크 (Verification, 외부 자격증명 필요)

**상태:** 미착수. deploy 템플릿은 완료(`deploy/Caddyfile.example`,
`deploy/oxibuilder.plist.example`, `deploy/oxibuilder.service.example`,
`deploy/Dockerfile`, `deploy/deploy.yaml.example`).

**남은 작업 (자격증명 제공 시):**

1. **릴리스 빌드** — `cargo build --release -p oxibuilder-console` (macOS 27
   `strip = "none"` 프로필 유지).
2. **호스트 기동** — Mac mini(launchd plist) 또는 Linux(systemd unit).
3. **Caddy 설정** — `deploy/Caddyfile.example` 복사 → 도메인 수정 → Caddy 기동.
4. **Cloudflare Tunnel** — `cloudflared tunnel create` + DNS 라우팅 → Caddy로.
5. **스모크:**
   - `curl https://<domain>/healthz` → 200
   - `curl https://<domain>/api/console/lobby/manifest` → JSON
   - `OXIBUILDER_TOKEN=<admin> oxibuilder blog new "테스트" --json` → slug 반환
   - `oxibuilder blog publish <slug>` → published_at 설정
   - `curl https://<domain>/blog/<slug>` → 발행본
   - 슬랙/카카오톡에 `<domain>/blog/<slug>` 링크 → 미리보기(OG 메타) 확인
6. **`OXIBUILDER_TMDB_KEY`/`OXIBUILDER_ALADIN_TTBKEY`/`OXIBUILDER_GITHUB_USERNAME`** 설정 시
   각 확장 검색/동기화 동작 확인.

## 8.6 브라우저 라이트/다크 + 접근성 확인 (Verification, 실측)

**상태:** 미착수. FOUC 방지 전환(`index.html` 인라인 스크립트)은 Phase 0 완료.

**남은 작업 (실측):**

1. **라이트/다크 전환** — `ThemeToggle` 클릭 → 즉시 전환(FOUC 없음). 시스템
   `prefers-color-scheme` 초기값 반영.
2. **`prefers-reduced-motion`** — 로비 canvas 모드 → grid 폴백(Lobby.tsx JS).
3. **키보드 내비게이션** — Tab 순서 논리적, 포커스 가시적.
4. **스크린리더** — VoiceOver/NVDA로 주요 페이지(로비/블로그 상세/검색) 읽기.
   `aria-label`/`role` 점검(RatingStars, LangToggle, SearchInput).
5. **반응형** — 모바일/태블릿/데스크톱 레이아웃.
6. **문서화** — `docs/accessibility.md`(8.3 WCAG 결과와 통합).

## 8.7 정리 필요 (clippy -D warnings 정상화) — ✅ 완료 (2026-07-27)

**상태:** ✅ 완료. `cargo clippy --workspace --all-targets -- -D warnings` 클린.

**수정 내역 (보고된 것):**
- `crates/oxibuilder-ext-blog/src/` — collapsible_if 1건
- `crates/oxibuilder-ext-projects/src/` — map_or → is_none_or 2건
- `crates/oxibuilder-ext-novels/src/` — `NovelPatch` 미사용 2건 (repo.rs, routes.rs)
- `crates/oxibuilder-ext-movies/src/` — unnecessary_unwrap 1건 + map_or 3건
- `crates/oxibuilder-ext-books/tests/api.rs` — `ENV_LOCK` await_holding_lock allow (함수 레벨)
- `crates/oxibuilder-cli/src/` — `Output::value` 미사용 메서드 제거, `Client::endpoint/post` dead_code allow 명시
- `crates/oxibuilder-core/src/auth.rs, rate_limit.rs` — collapsible_if 2건
- `crates/oxibuilder-core/tests/http_app.rs` — unused json 변수 → body 검증 assert 추가
- `crates/oxibuilder-console/Cargo.toml` — `[[bin]] name=oxibuilder-core` 오타 → `oxibuilder-console` 수정

## 8.8 다음 세션 권장 순서

1. ✅ **8.7 clippy 정리** (완료)
2. ✅ **8.2 SSR 확장 연결** (완료 — 7개 publish + 7개 delete)
3. ✅ **8.3 WCAG 실측** (완료 — `docs/accessibility.md`, 토큰 2개 조정)
4. **8.6 브라우저 접근성 실측** (VoiceOver/NVDA, prefers-reduced-motion 등)
5. **8.5 배포 스모크** (자격증명 제공 시)
6. **8.4 WASM 스파이크** (옵션, 수요 확인 후)

## 8.9 핵심 설계 편차 (다음 세션 준수)

- **Rating:** 0~10 정수 저장(`/2`→0~5점 별 5개). doc/02 §2.1의 "0~20"은 오타로
  정정(코드+doc 모두 0~10).
- **확장 API 경로:** axum 0.8 `{slug}` 형식(Not `:slug`), trailing slash 없음.
- **`order` 예약어:** 항상 `display_order`.
- **PAT 인증:** `OXIBUILDER_ADMIN_TOKEN` = 슈퍼유저(scopes `["admin"]`). PAT는
  `post:write`/`post:publish`/`read` 스코프. `AdminAuth` 진입 단계에서 `post:write`
  중앙 강제(23개 쓰기 라우트 자동 보호). `publish` 7개는 `require_scope("post:publish")`,
  토큰 관리 3개(`/api/console/auth/tokens*`)는 `require_scope("admin")`.
- **OpenAPI:** `utoipa` 대신 수동 `serde_json` 스펙(의존성 절약).
- **SSR:** Askama 대신 수동 템플릿(의존성 절약).
- **server `all_extensions()`:** 확장 추가/수정 시 `edit`/`SWAP` 대신 **반드시
  `write`로 통째 재작성**(이 세션에서 4회 연속 `vec![` 누락/중복 사고).
- **subagent 429:** Phase 2처럼 병렬 dispatch 시 가끔 `Token Plan usage limit`로
  즉시 죽음. inline 폴백 준비.

## 8.10 셀프호스팅 갭 수정 (2026-07-28)

**배경:** 셀프호스팅 아키텍처 평가에서 설계 문서가 "완료"로 표기한 핵심 운영
서브시스템들이 코드에 통합되지 않은 채 dead code로 남아 있고, tracker(§7.1,
§8.1)가 이를 거짓으로 완료 표기하고 있음이 확인됐다. 아래 항목을 수정했다.

### 수정 내역

1. **백그라운드 잡 스케줄러 통합 (치명적 갭)**
   - `ScheduledJob::run` 시그니처를 `run(&self)` → `run(&self, &AppState)`로 변경
     (`crates/oxibuilder-core/src/scheduler.rs`). 이전 시그니처는 job body가 DB
     pool/config에 접근할 수 없어 구조적으로 no-op이었다.
   - `Scheduler`에 6-field cron 파서 + `spawn_all()` 드라이버 추가
     (`tokio::time::sleep` 기반, `tokio-cron-scheduler` 외부 의존성 없이).
   - `run_server_with_extensions`(`crates/oxibuilder-console/src/lib.rs`)에 활성
     확장의 `background_jobs()` 수집 + `scheduler.spawn_all(state)` 연결.
   - `ActivitySyncJob::run` 실제 구현: `GithubClient::fetch_public_events()` →
     `repo::upsert()` (`crates/oxibuilder-ext-activity/src/lib.rs`).
   - `ScrapCollectJob::run` 실제 구현: HN topstories + GeekNews RSS fetch →
     `repo::upsert_queue_item()` (`crates/oxibuilder-ext-scraps/src/lib.rs`).
     scraps에 `reqwest` 의존성 추가.
   - workspace `Cargo.toml`에 tokio `time` feature 추가.

2. **GitHub webhook HMAC-SHA256 서명 검증 (보안 결함)**
   - `crates/oxibuilder-ext-activity/src/routes.rs`의 `webhook` 핸들러에
     `X-Hub-Signature-256` 검증 추가. `OXIBUILDER_GITHUB_WEBHOOK_SECRET` 환경변수
     필요 — 미설정 시 503(조용히 비활성화 원칙).
   - `hmac`/`sha2` 의존성 추가, `oxibuilder_core::auth::constant_time_eq` pub 노출.
   - 단위 테스트 6건 + 통합 테스트 2건(서명 없음/잘못된 서명 → 401) 추가.

3. **Dockerfile 비root 실행 (보안 결함)**
   - `deploy/Dockerfile` 런타임 단계에 `groupadd`/`useradd` + `USER oxibuilder` 추가.
     doc/05 §5.5 "최소 권한 컨테이너 실행" 준수.

4. **SIGTERM graceful shutdown**
   - `shutdown_signal()`이 `ctrl_c`(SIGINT)만 잡던 것을 `tokio::select!`로
     SIGTERM도 대기하도록 수정. systemd/launchd `stop` 시 드레인 보장.

5. **백업 메커니즘 (코드 레벨)**
   - `crates/oxibuilder-core/src/backup.rs`: `vacuum_into(pool, dest)` — SQLite
     `VACUUM INTO` 온라인 포인트-인-타임 스냅샷.
   - `POST /api/console/backup/snapshot` (admin) — `data_dir/backups/oxibuilder-<epoch>.db` 생성.
   - `oxibuilder backup snapshot` CLI 서브커맨드.

6. **Caddyfile container 모드 커버**
   - `deploy/Caddyfile.example`에 binary-direct(127.0.0.1:8787) vs container
     모드(컨테이너 전용 고정 IP) 분기 주석 추가.

### 검증

- `cargo test --workspace`: **128 tests, 0 failed**
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo check --workspace --all-targets`: clean

### 여전히 남은 항목 (외부 자격증명 필요)

- **배포 스모크 (§8.5):** Caddy + Cloudflare Tunnel + 실제 도메인 기동 검증.
- **브라우저 접근성 실측 (§8.6):** VoiceOver/NVDA, prefers-reduced-motion.
- **litestream 운영 설정:** doc/05 §5.4의 WAL 스트리밍은 운영 레벨(설정 파일)
  영역. 코드 레벨 폴백(`VACUUM INTO`)은 위 5번에서 구현됨.

## 8.11 2026-07-28 완료 배치

**상태:** ✅ 일괄 완료. 모든 빌드 게이트 통과 (163 tests, clippy -D warnings clean).

### 구현 완료 항목

1. **빌드 정리 (§8.7 확장):** Rust 1.96 clippy 신규 린트 대응 (`needless_borrow` 8건,
   `collapsible_if` 7건, `derivable_impls` 2건). `test_site_add_default_flag` 테스트 격리
   버그 수정 (고정 temp dir 재사용으로 인한 중복 add 실패 — cleanup 추가).

2. **멀티사이트 (doc/09):** 이미 구현 완료된 상태였음.
   - `crates/oxibuilder-cli/src/sites.rs` — `SitesFile` load/save/resolve
   - `crates/oxibuilder-cli/src/commands/site.rs` — `SiteCommand` (add/list/show/use/edit/rm)
   - `crates/oxibuilder-admin/src/sites_api.rs` — admin console CRUD API
   - `crates/oxibuilder-cli/src/commands/mod.rs` — `resolve_site_name`/`resolve_endpoint`/`resolve_token`
   - `Client::new()` endpoint/token resolution via site profile fallthrough
   - OXIBUILDER_SITE env + --site flag + default_site 3단계 우선순위
   - sites.toml 0600 권한, corrupt file graceful fallback

3. **CLI 확장성 (doc/11):** 이미 구현 완료된 상태였음.
   - `Extension::cli_commands()` trait + `CliCommand`/`CliSubcommand`/`CliArg`/`CliHandler` 타입
   - `Command::Dynamic(Vec<String>)` + `#[clap(external_subcommand)]`
   - `dispatch_dynamic()` / `resolve_command_registry()` / `parse_dynamic_args()`
   - `GET /api/console/cli/commands` + `POST /api/console/cli/exec/{ext_id}/{sub_command}` 서버 엔드포인트
   - 5개 확장 CLI 구현: `novels`(new/list/chapter add), `movies`(review add/series create),
     `books`(review add), `scraps`(add/queue/delete), `activity`(sync)
   - 컴파일 + 서버 디스커버리 이중 경로, 미발견 시 컴파일 목록 폴백

4. **관리 콘솔 (doc/12):** `oxibuilder admin` CLI 명령 + `admin-web` React SPA + proxy layer.

### 검증

- `cargo test --workspace`: **163 tests, 0 failed**
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cd web && bun run build`: clean
- `cd admin-web && bun run build`: clean

### 여전히 남은 항목 (외부 자격증명/수동 필요)

- **배포 스모크 (§8.5):** Caddy + Cloudflare Tunnel + 실제 도메인 기동 검증.
- **브라우저 접근성 실측 (§8.6):** VoiceOver/NVDA, prefers-reduced-motion, 키보드 내비게이션.
- **litestream 운영 설정:** doc/05 §5.4 — 운영 레벨 설정 파일 영역.

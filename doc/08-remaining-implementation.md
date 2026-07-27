# 8장 — 남은 구현 (Remaining Implementation)

> 2026-07-27 한 세션에서 Phase 1–5의 29/34 작업을 완료했습니다. 본 문서는
> **다음 세션에서 이어서 할 구체적 구현**을 남깁니다. workspace는 34 test suites
> 전부 통과, 빌드 OK 상태로 커밋됩니다.

## 8.1 현재 상태 요약

| Phase | 완료 | 비고 |
|---|---|---|
| Foundation | 4/4 | FTS5·Rating·scheduler·Extension trait·IntegrationsConfig·CLI 스캐폴드 |
| Phase 1 | 5/5 | blog·projects·links + CLI + 프론트 lazy route |
| Phase 2 | 7/7 | novels·movies·books·scraps·activity + 별점 + background_jobs |
| Phase 3 | 3/5 | 로비 3모드·LobbyConfig API·/search UI. **SSR 확장 연결·WCAG 실측 남음** |
| Phase 4 | 4/4 | PAT 스코프 분리·레이트리밋(build_app 연결)·OpenAPI/Swagger·SKILL.md |
| Phase 5 | 5/6 | deploy·LICENSE·SDK 문서·레지스트리·starter·개인화. **WASM 옵션 남음** |
| Verification | 1/3 | cargo test/clippy 통과. **배포 스모크·브라우저 접근성 실측 남음** |

**검증 상태:** `cargo test --workspace` 34 suites ok · `cd web && bun run build` ok ·
clippy는 unused import 정리만 남음(`-D warnings` 정상화 필요).

## 8.2 SSR 스냅샷 확장 연결 (Phase 3, core 인프라는 완료)

**상태:** `crates/oxipage-core/src/snapshot.rs` 모듈 + 단위 테스트 완료.
`render()`/`write_snapshot()`/`remove_snapshot()` + traversal 방지 sanitize.

**남은 작업:**

1. **`http::spa_index_html()` 추가** — `crates/oxipage-core/src/http.rs`의 `Assets`
   에서 `index.html`을 읽어 `Option<String>` 반환(pub). SPA 진입 스크립트 해시
   파일명을 스냅샷에 주입하기 위함.
2. **`snapshot::render_with_index(index_html, data)` 추가** — `snapshot.rs`에 새 함수.
   `index.html`의 `<title>…</title>` 교체 + `</head>` 앞에 OG 메타/canonical 삽입 +
   `<div id="root"></div>`에 `<main data-snapshot="true">{본문}</main>` 주입.
   브라우저는 같은 HTML을 받아 React가 `#root` 하이드레이트(기존 `main` 교체).
3. **각 확장 publish 핸들러에 연결** — 7곳:
   - `oxipage-ext-blog/src/routes.rs::publish`
   - `oxipage-ext-projects/src/routes.rs::publish`
   - `oxipage-ext-novels/src/routes.rs::publish_novel` + `publish_chapter`
   - `oxipage-ext-movies/src/routes.rs::publish`
   - `oxipage-ext-books/src/routes.rs::publish`
   - `oxipage-ext-scraps/src/routes.rs::publish`
   
   각 publish에서 reindex 후:
   ```rust
   if let Some(index_html) = oxipage_core::http::spa_index_html() {
       let data = oxipage_core::snapshot::SnapshotData {
           title: post.title.clone(),
           description: post.body.chars().take(200).collect(),
           canonical_url: format!("{}/blog/{}", state.config.site.base_url, post.slug),
           og_image: None,
           body_markdown: post.body.clone(),
           lang: post.lang.clone(),
       };
       let html = oxipage_core::snapshot::render_with_index(&index_html, &data);
       let _ = oxipage_core::snapshot::write_snapshot(
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

## 8.3 WCAG AA 대비 재검증 (Phase 3, 실측)

**상태:** 미착수. `web/src/shared/tokens.css`의 OKLCH 토큰은 시작값(doc/03 §3.7).

**남은 작업:**

1. **대비율 측정** — 주요 텍스트/배경 쌍:
   - `--color-text-primary` vs `--surface-1` (라이트/다크 각각)
   - `--color-text-secondary` / `--color-text-tertiary` vs 배경
   - `--accent` 버튼 텍스트 vs `--accent` 배경
   - 별점 `--color-rating-fill` vs 배경
2. **WCAG 2.1 AA 기준:** 일반 텍스트 ≥ 4.5:1, 큰 텍스트(18pt+/14pt bold) ≥ 3:1.
3. **OKLCH → sRGB → 상대 휘도 변환** 필요. Lea Verou의 `oklch.com` 또는
   `apca` 라이브러리로 측정 권장.
4. **필요 시 tokens.css 값 조정** — 기준 미달 색상의 L(밝기) 조정.
5. **문서화** — `docs/accessibility.md`에 측정 결과와 AA 준수 여부 기록.

이 항목은 코드 변경이 아닌 **측정 + 필요 시 토큰 조정** 작업.

## 8.4 WASM 컴포넌트 스파이크 (Phase 5, 옵션)

**상태:** 미착수. doc/07 §7.7 "수요 확인 후" — 우선순위 낮음.

**남은 작업 (스파이크 범위):**

1. **`wasmtime` 의존성 추가** — workspace deps.
2. **`Extension` 트레이트의 호스트 함수 미러링 설계** — WASM 컴포넌트가 호출할
   host functions(DB 접근, HTTP, 로비 카드 등)을 capability 기반 샌드박스로.
3. **최소 예제 확장 1개를 WASM으로 컴파일** — `oxipage-ext-wasm-demo` (예: hello
   world 카드 반환).
4. **`oxipage extension install <name>`** — `registry/index.json`에서 메타데이터
   읽어 `.wasm` 파일 다운로드 + `/data/extensions/` 저장. 단, **런타임 설치 확장은
   CLI 서브커맨드 추가 불가** (doc/01 §1.4 알려진 한계) — API/웹으로만.
5. **문서화** — `docs/wasm-spike.md`에 설계/한계/다음 단계.

doc/07 §7.7 명시: "명시적 설계 제약". 본 스파크는 가능성 탐색이며 v1 기능이 아님.

## 8.5 배포 스모크 (Verification, 외부 자격증명 필요)

**상태:** 미착수. deploy 템플릿은 완료(`deploy/Caddyfile.example`,
`deploy/oxipage.plist.example`, `deploy/oxipage.service.example`,
`deploy/Dockerfile`, `deploy/deploy.yaml.example`).

**남은 작업 (자격증명 제공 시):**

1. **릴리스 빌드** — `cargo build --release -p oxipage-server` (macOS 27
   `strip = "none"` 프로필 유지).
2. **호스트 기동** — Mac mini(launchd plist) 또는 Linux(systemd unit).
3. **Caddy 설정** — `deploy/Caddyfile.example` 복사 → 도메인 수정 → Caddy 기동.
4. **Cloudflare Tunnel** — `cloudflared tunnel create` + DNS 라우팅 → Caddy로.
5. **스모크:**
   - `curl https://<domain>/healthz` → 200
   - `curl https://<domain>/api/v1/lobby/manifest` → JSON
   - `OXIPAGE_TOKEN=<admin> oxipage blog new "테스트" --json` → slug 반환
   - `oxipage blog publish <slug>` → published_at 설정
   - `curl https://<domain>/blog/<slug>` → 발행본
   - 슬랙/카카오톡에 `<domain>/blog/<slug>` 링크 → 미리보기(OG 메타) 확인
6. **`OXIPAGE_TMDB_KEY`/`OXIPAGE_ALADIN_TTBKEY`/`OXIPAGE_GITHUB_USERNAME`** 설정 시
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

## 8.7 정리 필요 (clippy -D warnings 정상화)

**상태:** `cargo clippy --workspace`에 unused import warnings.

**수정 대상 (보고된 것):**
- `crates/oxipage-ext-blog/src/` — unused import 1건
- `crates/oxipage-ext-projects/src/` — 2건
- `crates/oxipage-ext-novels/src/` — `NovelPatch` 미사용 2건 (repo.rs, routes.rs)
- `crates/oxipage-ext-movies/src/` — 3건
- `crates/oxipage-ext-books/tests/api.rs` — 1건
- `crates/oxipage-cli/src/` — `Output::value` 미사용 메서드 (제거 또는 사용)

**작업:** `cargo clippy --fix --workspace --allow-dirty` 또는 수동 제거.
`-D warnings` CI 게이트 통과 목적.

## 8.8 다음 세션 권장 순서

1. **8.7 clippy 정리** (10분, CI 게이트) → 커밋
2. **8.2 SSR 확장 연결** (1–2시간, blog부터 패턴 → 나머지 일괄)
3. **8.3 WCAG 실측** (측정 + 필요 시 tokens 조정)
4. **8.6 브라우저 접근성 실측** (8.3과 병행)
5. **8.5 배포 스모크** (자격증명 제공 시)
6. **8.4 WASM 스파이크** (옵션, 수요 확인 후)

## 8.9 핵심 설계 편차 (다음 세션 준수)

- **Rating:** 0~10 정수 저장(`/2`→0~5점 별 5개). doc/02 §2.1의 "0~20"은 오타로
  정정(코드+doc 모두 0~10).
- **확장 API 경로:** axum 0.8 `{slug}` 형식(Not `:slug`), trailing slash 없음.
- **`order` 예약어:** 항상 `display_order`.
- **PAT 인증:** `OXIPAGE_ADMIN_TOKEN` = 슈퍼유저(scopes `["admin"]`). PAT는
  `post:write`/`post:publish`/`read` 스코프. `AdminAuth` 진입 단계에서 `post:write`
  중앙 강제(23개 쓰기 라우트 자동 보호). `publish` 7개는 `require_scope("post:publish")`,
  토큰 관리 3개(`/api/v1/auth/tokens*`)는 `require_scope("admin")`.
- **OpenAPI:** `utoipa` 대신 수동 `serde_json` 스펙(의존성 절약).
- **SSR:** Askama 대신 수동 템플릿(의존성 절약).
- **server `all_extensions()`:** 확장 추가/수정 시 `edit`/`SWAP` 대신 **반드시
  `write`로 통째 재작성**(이 세션에서 4회 연속 `vec![` 누락/중복 사고).
- **subagent 429:** Phase 2처럼 병렬 dispatch 시 가끔 `Token Plan usage limit`로
  즉시 죽음. inline 폴백 준비.

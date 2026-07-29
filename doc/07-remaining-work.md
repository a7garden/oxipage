# 7장 — 남은 작업 (Remaining Work)

> 2026-07-28 기준 **Phase 0~5 전 영역 구현 완료 + v2 SSG 설계 완료.** 하위 섹션은 이력 보존용이며,
> 현재 상태는 §7.1과 `08` 기준.

## 7.1 현재 상태 (2026-07-28 갱신, v2 SSG 설계 추가)

Phase 0~5 전 영역 구현 완료 + 멀티사이트·CLI 확장성·관리 콘솔까지 추가 구현 완료.
**2026-07-28: v2 SSG 전환 설계 완료. 미구현.**

| 영역 | 상태 |
|---|---|
| Cargo 워크스페이스 (core/server/cli/admin/wasm + 9개 확장 + demo) | ✅ |
| Axum 0.8 서버, SQLite(WAL) + 확장별 네임스페이스 마이그레이션, 레지스트리 | ✅ |
| FTS5(`tokenize='trigram'`) 전문검색, SSR 발행 시점 스냅샷, PAT 스코프 인증, 레이트리밋, OpenAPI | ✅ |
| 9개 확장 + 공통 Rating 값객체 + 백그라운드 잡 스케줄러 | ✅ |
| Vite 7 + React 19 + TS SPA, OKLCH 토큰, 다크/라이트, 로비 3모드, `/search` | ✅ |
| Oxipage CLI(init/status/serve/auth/blog/project/link/lobby/backup/site/admin) + Dynamic 확장 명령 | ✅ |
| 배포 템플릿·LICENSE(MIT)·SDK 문서·레지스트리·starter | ✅ |
| 멀티사이트 (sites.toml CRUD, --site flag, OXIPAGE_SITE env, 3단계 fallthrough) | ✅ |
| CLI 확장성 (doc/11: external_subcommand + 5개 확장 CLI + server discovery) | ✅ |
| 관리 콘솔 (oxipage-admin + admin-web SPA + proxy/themes/sites API) | ✅ |
| WASM 컴포넌트 런타임 v2 (fuel/DB HTTP capability/hot reload/ed25519 서명) | ✅ |
| **SSG 전환 설계 (BuildExt/build/deploy/query/schema)** | **✅ 설계 완료 (미구현)** |
| 검증: `cargo test --workspace` 163 tests ok · clippy `-D warnings` clean · web build | ✅ |
| 검증: 배포 스모크·브라우저 접근성 실측 | ⏳ 외부 자격증명 및 수동 실측 필요 |

### 제로잔여 (Zero-Remaining) — 빌드 게이트 기준

```bash
cargo test --workspace          # 163 passed, 0 failed
cargo clippy --all-targets -- -D warnings  # clean
cd web && bun run build         # clean
cd admin-web && bun run build   # clean
```

이상의 게이트는 지속적으로 통과 중. 이후 PR/커밋마다 동일 조건을 유지할 것.

## 7.2 Phase 0에서 이월된 항목

| 항목 | 내용 | 시점 |
|---|---|---|
| `constant_time_eq` 토큰 길이 타이밍 누출 | 현재 구현이 길이 불일치 시 즉시 `false` 반환 → 원격 타이밍으로 토큰 길이 추론 가능. v0 단일 오너 모델(localhost/LAN)에서는 무시 가능하다고 판정·보류(parked). PAT 도입 시 HMAC-SHA256 비교로 함께 정비. | Phase 1/4 |
| v0 임시 인증 → PAT 체계 | `OXIPAGE_ADMIN_TOKEN` 단일 토큰 + constant-time 비교. `doc/01` §1.8의 PAT(scope: `post:write`/`post:publish`, SHA-256 해시 저장, `oxipage auth token create/list/revoke`)로 교체. | Phase 1/4 |
| CLI 크레이트 | Phase 0 범위에서 의도적 제외. `oxipage` CLI 바이너리(`doc/04` §4.3 명령 체계) 신규 추가. | Phase 1 |
| container 패키징 1회 확인 | `doc/06` Phase 0 완료 기준의 "선택" 항목. `deploy/Dockerfile` + Apple `container build`/`container run`으로 동일 결과 1회 검증. | Phase 1 |

## 7.3 Phase 1 — 핵심 콘텐츠 확장 + CLI

**목표:** 매일 쓰는 최소 기능. CLI로 블로그 글을 처음부터 끝까지(작성 → 초안 → 발행) 실제 도메인에 올릴 수 있을 것.

**작업 항목:**
- `oxipage-ext-blog`: `blog` 테이블(`doc/02` §2.6), `lang`/`translation_group_id` 느슨한 번역 연결, 초안(`published_at` NULL)/발행 상태, 태그(JSON 배열). 라우트 `GET/POST /api/console/blog/posts`, `GET/PATCH/DELETE /api/console/blog/posts/{slug}`.
- `oxipage-ext-projects`: `doc/02` §2.4 — `title_ko`/`title_en`, `description_ko`/`description_en`(구조적 이중언어 강제), `tech_stack`, `status`, `links`, `featured`, `screenshots` 테이블.
- `oxipage-ext-links`: `doc/02` §2.11 — `LinkCard` + `display_order` + `featured`.
- **공통 `search_documents` FTS5 인덱스**: `doc/01` §1.7 — `tokenize='trigram'`(한국어 대응). Phase 3의 `/search` UI 전에 인덱싱 훅을 각 확장 `on_publish`에 붙인다(확장 비활성화 시 즉시 동기 삭제 — `doc/02` §2.13).
- `oxipage-cli` 크레이트 신규: `doc/04` §4.3 — `auth`(login/token create/list/revoke), `init`, `status`, `serve`, `blog`, `project`, `link`. **원칙: CLI가 하는 모든 일은 인증된 HTTP 호출**(`doc/04` §4.1). PAT 읽기: `OXIPAGE_TOKEN` 환경변수 → 키체인/`~/.config/oxipage/credentials`(0600). `--json` 전역 지원.
- **초안 우선 원칙**(`doc/04` §4.3): `add`/`new`는 초안만 생성, 발행은 별도 `publish`/`--publish`.
- 프론트엔드: `/blog`, `/projects`, `/links` lazy route chunk(코드 스플리팅), 목록/상세 페이지, 마크다운 렌더러 공통 컴포넌트(`markdown-it` + 코드 하이라이팅).
- SSR 스냅샷(`doc/01` §1.6)은 Phase 3로 이관 — Phase 1은 순수 SPA로 우선 배포.

**배포(완료 기준의 일부):**
- `deploy/Caddyfile.example`, `deploy/oxipage.plist.example`(macOS launchd)/`deploy/oxipage.service.example`(Linux).
- Cloudflare Tunnel(`cloudflared`) + Caddy 호스트 네이티브 리버스 프록시(`doc/05` §5.2).

**완료 기준:** CLI로 블로그 글 1건을 작성→초안 확인→발행까지 실제 도메인에서 완결.

**선행 조건(사용자 결정 필요):**
- 배포 대상 도메인 / Cloudflare 계정 / `cloudflared` 터널 자격증명.
- Mac mini 호스트(macOS 26 Tahoe + Apple Silicon) 또는 단일 Linux 호스트.
- 위 배포 자격증명이 준비되지 않은 경우, **배포는 제외하고 기능 확장 + CLI까지만 먼저 진행**하는 변형 경로 가능(완료 기준 중 "실제 도메인 배포"만 연기).

## 7.4 Phase 2 — 풍부한 확장 (외부 API 연동)

**목표:** 요구사항의 "재미있는 부분" 채우기.

**작업 항목:**
- `oxipage-ext-movies` + TMDB 연동(`doc/02` §2.9): `MovieEntry` + `SeriesGroup`. TMDB 검색/캐시 흐름(sequence diagram). `OXIPAGE_TMDB_KEY` 미설정 시 수동 입력만 지원(조용히 비활성화 원칙). TMDB attribution 푸터 표기.
- `oxipage-ext-books` + 알라딘 OpenAPI(1순위) / Google Books(폴백) 연동(`doc/02` §2.10). `OXIPAGE_ALADIN_TTBKEY` 미설정 시 `manual`만.
- `oxipage-ext-novels`: `Novel` + `NovelChapter`(`char_count` 자동 계산, `doc/02` §2.5).
- `oxipage-ext-scraps` + HN(Firebase API)/GeekNews(RSS) 수집 잡(`doc/02` §2.7). **발행은 항상 사람 선택** — 백그라운드 잡은 "추천 큐"만 채운다.
- `oxipage-ext-activity` + GitHub 활동(`doc/02` §2.8): webhook 1순위 + Events API 15분 폴링(보조). **공개 Events API만 사용 → private repo 노출 안 됨(설계 보장)**.
- 공통 `Rating`(0~10 정수 = 0.5점 단위) 값 객체 + 프론트 별점 5개 컴포넌트(`--color-rating-fill`).
- 백그라운드 잡 스케줄러: `tokio-cron-scheduler` 코어 단일 인스턴스, 각 확장 `background_jobs()` 등록(`doc/01` §1.9 표 참고).

**완료 기준:** 영화 리뷰 1건을 TMDB 검색부터 `SeriesGroup` 묶음까지 CLI로 완결, 로비에서 최근 활동/스크랩이 실시간에 가깝게 갱신.

## 7.5 Phase 3 — 로비 완성 + SEO

**목표:** "보여주기" 완성도.

**작업 항목:**
- 로비 레이아웃 3종(`doc/03` §3.6): `list`(정갈) / `grid`(카드, 현재 기본) / `canvas`(플로팅 — 시그니처 요소). `LobbyConfig.display_mode`(`doc/02` §2.12, Phase 0에서 `display_order`로 생성됨) per-extension 설정.
- `oxipage lobby layout set <extension> --mode canvas|grid|list` CLI.
- **접근성**: `prefers-reduced-motion: reduce` 시 `canvas` → `grid` 자동 폴백(예외 없음).
- SSR 스냅샷 파이프라인(`doc/01` §1.6): 발행 시점 Askama로 prerendered HTML 생성 → `/data/snapshots/`. OG 메타 + canonical + SPA 스크립트 태그. **User-Agent 기반 봇 판별 사용 금지**(설계 원칙).
- `/search` UI: Phase 1에서 깔아둔 FTS5 `search_documents` 인덱스 소비.
- WCAG AA 대비 재검증(`doc/03` §3.7). 현재 OKLCH 토큰은 시작값이므로 실측 필요.

**완료 기준:** 슬랙/카카오톡 링크 미리보기(제목·요약·이미지) 정상, `prefers-reduced-motion`에서 `canvas`→`grid` 폴백 확인.

## 7.6 Phase 4 — 에이전트 통합 + API 하드닝

**목표:** "말로 시켜서 올리기"가 안전하게 동작.

**작업 항목:**
- `.agent/skills/oxipage-cli/SKILL.md` 작성(`doc/04` §4.6 초안 그대로 시작점). oh-my-pi(GLM/MiniMax/DeepSeek 등 어떤 모델이든) 실사용 테스트.
- PAT 스코프 분리 적용: `post:write`(초안) vs `post:publish`(발행). 에이전트 토큰은 기본 `post:write`만 → 명시적 승인 없이 발행 불가(초안 우선 원칙의 인증층 보장).
- OpenAPI 자동 생성(`utoipa`) + `/api/console/docs`(Swagger UI).
- 레이트리밋, 요청 로깝. 공개 읽기 API는 레이트리밋만, 쓰기 API는 오너 토큰 필수.

**완료 기준:** 반복 테스트로 "초안까지는 자동, 명시적 승인 없이는 절대 발행 안 됨" 확인.

## 7.7 Phase 5 — OSS 제품화 (선택, 후순위)

**목표:** 제3자가 문서만 보고 자기 Mac/서버에 설치해 블로그 확장 하나를 켜는 데까지 성공.

**작업 항목(`doc/05` §5.7 + `doc/06` Phase 5):**
- 개인화 요소 전부 `oxipage.toml`/`profile`로 이관.
- `oxipage-starter` 템플릿 저장소 + 원클릭 설치 스크립트(`curl … | sh`).
- 확장 레지스트리: GitHub 저장소 1개의 curated JSON 인덱스(Homebrew tap류). `oxipage extension install <name>`.
- `Extension` 트레이트 공개 SDK 문서화("새 확장 만들기" 가이드 1종).
- `oxipage deploy` 매니페스트(`deploy/deploy.yaml`, compose-spec 유사 키)를 단일 진실 소스로 — (a) 바이너리 직접 기동(launchd/systemd) 또는 (b) `container run` 선택.
- (수요 확인 후) WASM 컴포넌트 기반 런타임 로딩 스파이크. **알려진 한계(`doc/01` §1.4):** 런타임 설치 확장은 CLI 서브커맨드 추가 불가(정적 링크 필요) — 명시적 설계 제약.
- 라이선스 결정(MIT 또는 Apache-2.0 권장).

## 7.8 명시적 v1 범위 밖 (`doc/00` §0.6)

재확인 — 의도적으로 미루는 항목:
- 멀티테넌트 SaaS(서버 1대가 다수 사이트 서빙) — "각자 셀프호스팅" 모델 유지.
- 댓글/방명록 등 소셜 기능.
- 실시간 협업 편집.
- TV 시리즈 시즌/에피소드 세분화 리뷰(`movies`는 작품 단위 + `SeriesGroup` 묶음만).
- 콘텐츠 개정 이력(발행 후 수정 이력 보존). `*_revisions` 보조 테이블 확장 여지만 남겨둠.

## 7.9 환경 메모 (재발 방지용)

- **macOS 27 빌드**: release 프로필 `strip = "none"` 고정 필수(`doc/05` §5.1의 Apple Silicon 전제와 무관한 dyld 제약 — `rust-lang/rust#157750`). 신규 크레이트 추가 시에도 동일 프로필 적용.
- **rust-embed 컴파일타임 요구**: 신규 확장이나 바이너리 크레이트에서 `#[derive(RustEmbed)]` 도입 시, cargo 실행 전에 대상 폴더(`web/dist` 등)가 존재해야 debug/release 모두 컴파일됨.
- **확장 API 경로**: axum 0.8 `nest` 시맨틱상 루트 라우트는 prefix 무슬래시로 서빙됨(`/api/console/<ext>`, trailing slash 없음). `oxipage-cli` 및 프론트 fetch 경로도 이 규칙을 따를 것.

## 7.10 즉시 시작 가능 vs 사용자 결정 필요

| 경로 | 내용 | 즉시 가능? |
|---|---|---|
| A | Phase 1 기능 확장(`blog`/`projects`/`links` + FTS 인덱싱) + CLI + 단위/통합 테스트 — **배포 제외** | ✅ 바로 착수 가능 |
| B | A + Caddy/Cloudflare Tunnel 배포 파이프라인(`deploy/`) | ⚠️ 도메인·Cloudflare 자격증명 필요 |
| C | Phase 2 외부 API 연동 확장 | ✅ (TMDB/알라딘 키는 미설정 시 `manual` 폴백으로 진행 가능) |

Phase 1 진행 시 A→C 순이 무난하며, B는 자격증명이 확보되는 시점에 끼워 넣는다.

## 7.11 v2 SSG 전환 작업 (2026-07-28~)

**설계 문서:** `docs/superpowers/specs/2026-07-28-static-site-generator-design.md`

**목표:** 상시 서버 의존형(v1)에서 정적 사이트 생성기(v2)로 전환. `oxipage build && oxipage deploy`로 GitHub Pages에 배포 가능.

### 작업 항목

| # | 작업 | 영역 | 의존성 |
|---|---|---|---|
| 1 | `BuildExt` 트레이트 정의 (build_pages/build_data/build_search_docs) | Core | 없음 |
| 2 | 9개 확장 `BuildExt` 구현체 (각 확장의 DB → HTML + JSON 검증 로직) | Extensions | 1 |
| 3 | 빌드 파이프라인 (rayon 병렬, out/ 디렉토리 출력) | Core | 1, 2 |
| 4 | `oxipage build` CLI 명령 | CLI | 3 |
| 5 | `oxipage deploy` CLI 명령 (GitHub Pages 1순위, git worktree 기반) | CLI | 4 |
| 6 | `oxipage query` / `oxipage schema` CLI 명령 (AI 에이전트용) | CLI | 없음 |
| 7 | `oxipage cache refresh` CLI 명령 (외부 API 수집, 빌드와 분리) | CLI | 없음 |
| 8 | `oxipage console --preview` (out/ 디렉토리 HTTP 서빙) | Server | 4 |
| 9 | React SPA `VITE_DATA_MODE` 전환 (개발: API, 프로덕션: 정적 JSON) | Web | 4 |
| 10 | Admin-web은 불변 (확인) | Admin | 없음 |
| 11 | README / 설계 문서 업데이트 | Docs | 전체 |

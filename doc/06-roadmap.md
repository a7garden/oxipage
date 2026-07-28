# 6장 — 로드맵

> 2026-07-28 기준 **Phase 0~5 전 영역 구현 완료.** 하위 섹션은 전체 구현 이력
> 보존용으로 유지하며, 현재 상태는 §7.1/§8.11 기준.

## 달성 현황 요약

| Phase | 목표 | 상태 |
|---|---|---|
| Phase 0 — 뼈대 | Cargo 워크스페이스, Axum/SQLite 서버, Vite+React SPA, Profile 확장 | ✅ 완료 |
| Phase 1 — 핵심 콘텐츠 | blog/projects/links 확장 + CLI + 초안/발행 | ✅ 완료 |
| Phase 2 — 풍부한 확장 | novels/movies/books/scraps/activity + TMDB/알라딘 연동 + 별점 | ✅ 완료 |
| Phase 3 — 로비 완성 + SEO | 로비 3모드 + SSR 스냅샷 + /search + WCAG AA | ✅ 완료 |
| Phase 4 — 에이전트 통합 | PAT 스코프 + OpenAPI + 레이트리밋 + SKILL.md | ✅ 완료 |
| Phase 5 — OSS 제품화 | deploy + LICENSE + SDK + 레지스트리 + WASM v2 | ✅ 완료 |
| 파생 — 멀티사이트 | sites.toml, CLI site 명령, 3단계 endpoint 해상도 | ✅ 완료 |
| 파생 — CLI 확장성 | CliCommand trait, Dynamic subcommand, 5개 확장 CLI | ✅ 완료 |
| 파생 — 관리 콘솔 | oxipage-admin crate, admin-web SPA, proxy/themes | ✅ 완료 |
| **Phase 6 — SSG 전환** | **BuildExt / build/deploy/query/schema / 정적 사이트** | **⏳ 계획 완료** |
| **잔여** | 배포 스모크(자격증명), 브라우저 접근성 실측(수동) | ⏳

**목표:** 빈 사이트라도 실제로 켜져서 접속되는 상태.

- Cargo 워크스페이스 구성 (1장 §1.3 구조 그대로)
- `oxipage-core`: Axum 서버 부트스트랩, SQLite 연결, `oxipage.toml` 로딩, 확장 레지스트리 골격
- `web/`: Vite+React+TS 스캐폴드, 3장 OKLCH 토큰 세트 적용, 다크/라이트 토글
- `profile` 확장(가장 단순한 싱글턴)만 우선 구현해 "명함 페이지 하나"가 실제로 뜨는지 확인
- `oxipage-server` 바이너리 로컬 빌드·실행 성공 (container 패키징은 선택, 5장 §5.2-5.3)

**완료 기준:** `oxipage-server` 바이너리를 로컬에서 빌드·실행해 브라우저로 접속, 명함 페이지가 라이트/다크 둘 다 정상 렌더. (container 패키징은 선택 — `container build && container run`으로도 동일 결과가 나오는지 1회 확인만.)

## Phase 1 — 핵심 콘텐츠 확장 + CLI

**목표:** 실제로 매일 쓰는 최소 기능.

- `projects`, `blog`, `links` 확장 (요구사항 중 사용 빈도가 가장 높을 것으로 예상되는 3개)
- Oxipage CLI 첫 버전: `auth`, `init`, `blog`, `project`, `link`, `status`
- 초안/발행 흐름(§4.3 초안 우선 원칙) 구현
- Caddy + Cloudflare Tunnel로 실제 도메인에 배포 (5장)
- SSR 스냅샷(1장 §1.6)까지는 아직 없어도 됨 — 순수 SPA로 우선 배포

**완료 기준:** CLI로 블로그 글 하나를 처음부터 끝까지(작성→초안 확인→발행) 실제 도메인에 올릴 수 있음.

## Phase 2 — 풍부한 확장

**목표:** 요구사항의 "재미있는 부분" 채우기.

- `movies` + TMDB 연동, `SeriesGroup` 묶음 평가
- `books` + 알라딘/Google Books 연동
- `novels` (Novel + Chapter)
- `scraps` + HN/GeekNews 수집 잡
- `activity` + GitHub 이벤트 폴링
- 공통 `Rating` 컴포넌트(별점 0.5 단위) 프론트 구현

**완료 기준:** 영화 리뷰 하나를 TMDB 검색부터 시리즈 그룹 묶음까지 CLI로 완결할 수 있고, 로비에서 최근 활동/스크랩이 실시간에 가깝게 갱신됨.

## Phase 3 — 로비 완성 + SEO

**목표:** "보여주기"의 완성도.

- 로비 레이아웃 3종(`list`/`grid`/`canvas`) 구현, `oxipage lobby layout set`
- SSR 스냅샷 파이프라인(§1.6) — 블로그/프로젝트/리뷰 상세 페이지 OG 카드 대응
- 전문 검색(SQLite FTS5, 한국어 위해 `tokenize='trigram'` — §1.7) 통합, `/search` (토크나이저 결정은 Phase 0에서 이미 마친 상태)
- 접근성 점검(§3.7 체크리스트 전항목)

**완료 기준:** 슬랙/카카오톡에 블로그 글 링크를 붙였을 때 제목·요약·이미지가 제대로 뜨고, `prefers-reduced-motion`에서 `canvas` 모드가 자동으로 `grid`로 폴백됨.

## Phase 4 — 에이전트 통합 + API 하드닝

**목표:** "말로 시켜서 올리기"가 실제로 안전하게 동작.

- `.agent/skills/oxipage-cli/SKILL.md` 작성 및 oh-my-pi로 실사용 테스트 (4장 §4.6)
- API 토큰 스코프(`post:write` / `post:publish`) 분리 적용
- OpenAPI 문서 자동 생성(`utoipa`) + `/api/v1/docs`
- 레이트리밋, 요청 로깅

**완료 기준:** oh-my-pi(GLM/MiniMax/DeepSeek 등 어떤 모델을 백엔드로 쓰든)에게 "이 글 블로그에 올려줘"라고 시켰을 때, 초안까지는 자동으로 만들지만 명시적 승인 없이는 절대 발행되지 않음을 반복 테스트로 확인.

## Phase 5 — OSS 제품화 (선택, 후순위)

**목표:** 남도 가져다 쓸 수 있는 형태.

- 개인화 요소 설정으로 이관 완료 (5장 §5.7-1)
- `oxipage-starter` 템플릿 저장소 + 원클릭 설치 스크립트
- 확장 레지스트리(curated JSON 인덱스) + `oxipage extension install`
- `Extension` 트레이트를 공개 SDK로 문서화
- (수요 확인 후) WASM 컴포넌트 기반 런타임 로딩 스파이크

**완료 기준:** 제3자가 문서만 보고 자기 Mac/서버에 처음부터 설치해 블로그 확장 하나를 켜는 데까지 성공(직접 도움 없이).

## 우선순위 근거

- Phase 1을 `projects`/`blog`/`links`로 시작한 이유: 포트폴리오·블로그·생태계 링크가 "개발자 개인 홈페이지"의 본질에 가장 가깝고, `movies`/`books`/`scraps`는 외부 API 연동이 있어 상대적으로 후순위로 미뤄도 손해가 적습니다.
- 화려한 `canvas` 로비는 의도적으로 Phase 3까지 미룹니다 — 시그니처 요소이긴 하지만(3장 §3.1), 콘텐츠가 하나도 없는 상태에서 로비 모션부터 완성해봐야 보여줄 게 없습니다.
- OSS 제품화(Phase 5)는 전체가 "선택 사항"이라는 원 요청의 톤을 그대로 반영해 맨 뒤에 두되, 그 방향을 처음부터 설계에 반영해 뒀기 때문에(1장의 트레이트 경계, 5장의 설정 기반 구조) 나중에 급하게 갈아엎을 일은 없도록 했습니다.

## Phase 6 — SSG 전환 (Static Site Generator)

**목표:** 상시 서버 의존형(v1)에서 정적 사이트 생성기(v2)로 전환.

- `BuildExt` 트레이트 추가: `build_pages()`, `build_data()`, `build_search_docs()`
- 9개 확장에 `BuildExt` 구현체 추가 (각 확장의 DB → 정적 HTML + JSON 생성 로직)
- `oxipage build` 명령: rayon 병렬 빌드 파이프라인
- `oxipage deploy` 명령: GitHub Pages 1순위 배포 (git worktree 기반)
- `oxipage query` / `oxipage schema` 명령: AI 에이전트용 직접 DB 조회
- `oxipage cache refresh` 명령: 외부 API 수집 (빌드와 분리)
- React SPA 데이터 레이어: `VITE_DATA_MODE` 분기 (개발: API → 프로덕션: 정적 JSON)
- 배포 모델 변경: launchd/systemd 상시 서버 → `oxipage build && oxipage deploy`
- `oxipage console --preview`: 정적 사이트 로컬 미리보기

**완료 기준:** `oxipage build && oxipage deploy`로 정적 사이트가 GitHub Pages에서 라이브. CLI로 콘텐츠 관리, 빌드, 배포 전부 한 방에 가능.

**설계 문서:** `docs/superpowers/specs/2026-07-28-static-site-generator-design.md`

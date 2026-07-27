# 6장 — 로드맵

그린필드 착수를 위해 "1인 개발 + AI 코딩 에이전트(oh-my-pi 등)" 속도를 전제로 단계를 쪼갰습니다. 각 단계는 그 자체로 배포 가능한 상태를 목표로 합니다 — 완성될 때까지 아무것도 안 켜져 있는 상태로 두지 않습니다.

## Phase 0 — 뼈대

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

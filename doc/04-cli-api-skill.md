# 4장 — CLI · API · 에이전트 스킬

## 4.1 원칙: CLI는 API의 레퍼런스 클라이언트

`oxipage` CLI가 하는 모든 일은 HTTP API 호출입니다. 인증은 폐지되었으므로 토큰이 필요 없습니다(단, `oxipage console`로 로컬 관리 서버를 띄우는 것 자체는 예외 — 이때는 CLI가 곧 서버 프로세스를 기동하는 것). 이렇게 하면:

- API 문서 = CLI 문서 (둘이 어긋날 일이 없음)
- oh-my-pi 같은 에이전트가 CLI를 몰라도 API를 직접 호출해 같은 일을 할 수 있음
- 웹 관리자 UI(있다면)도 같은 API를 씀 — 3개의 프론트엔드(CLI/웹/에이전트)가 API 하나를 공유

## 4.2 인증 (폐지)

정적 사이트 아키텍처에서 인증이 제거되었습니다. 모든 API 라우트가 공개되어 있으며, 관리 서버는 loopback에서만 동작합니다.

## 4.3 명령 체계

```text
# 프로젝트 관리
oxipage init                                   # oxipage.toml 스캐폴딩
oxipage status [--json]                         # 초안/최근 게시물/배포 상태 요약
oxipage console [--port 8787]                     # 로컬 개발 서버
oxipage deploy                                   # 컨테이너 빌드 + Mac mini 배포 (5장)
oxipage backup export [--out FILE]

# 프로필(명함)
oxipage profile edit                             # $EDITOR로 profile.toml 열람/저장
oxipage profile show [--json]

# 프로젝트 포트폴리오
oxipage project add --title-ko "..." --title-en "..." \
    [--desc-ko FILE] [--desc-en FILE] [--tech rust,react] \
    [--screenshot PATH ...] [--link repo=URL --link demo=URL]
oxipage project publish <slug>
oxipage project list [--status active|archived|wip] [--json]

# 블로그
oxipage blog new "제목" [--lang ko|en] [--file DRAFT.md]
oxipage blog publish <slug>
oxipage blog list [--draft] [--json]

# 소설
oxipage novel new "제목"
oxipage novel chapter add <novel-slug> --title "1화" --file ch1.md
oxipage novel chapter publish <novel-slug> <order>

# 영화/시리즈 리뷰
oxipage review movie add --tmdb "<검색어 또는 tmdb id>" \
    --rating 8.5 [--review-file review.md] [--series "해리포터"]
oxipage review movie series create "해리포터" [--rating 9.0] [--review-file r.md]

# 도서 리뷰
oxipage review book add --isbn "<isbn 또는 검색어>" \
    --rating 4.5 [--review-file review.md] [--status reading|completed]

# 스크랩
oxipage scrap add <url> [--source hn|geeknews|manual] [--note "..."]
oxipage scrap queue                              # 백그라운드로 수집된 추천 후보 목록

# 생태계 링크
oxipage link add --title "..." --url "..." [--desc-ko "..."] [--desc-en "..."]

# 활동
oxipage activity sync                            # 즉시 GitHub 동기화 트리거

# 로비/확장
oxipage lobby layout set <extension> --mode canvas|grid|list
oxipage extension list
oxipage extension enable|disable <name>
oxipage extension purge <name> --yes               # 테이블 DROP + 미디어 rm (doc/02 §2.13)
oxipage extension install <name>                 # Phase 5, OSS 레지스트리 (6장)
```

**전역 옵션:** 모든 하위 명령은 `--json`을 지원해 사람 친화적 출력과 기계 친화적 출력을 모두 냅니다. 이건 에이전트(oh-my-pi)가 결과를 안정적으로 파싱하기 위한 요구사항입니다(§4.6).

**초안 우선 원칙(안전장치):** `add`/`new` 계열 명령은 기본적으로 **초안(draft)** 을 만들 뿐 공개하지 않습니다. 공개는 반드시 `publish` 명령이나 `--publish` 플래그로 별도 확인을 거칩니다. 이는 CLI뿐 아니라 에이전트 스킬(§4.6)에서도 동일하게 강제합니다.

## 4.4 설정 파일 `oxipage.toml`

```toml
[site]
name = "내 이름의 Oxipage"
base_url = "https://example.dev"
default_lang = "ko"
languages = ["ko", "en"]

[server]
api_endpoint = "https://example.dev/api/console"

[extensions]
enabled = ["profile", "projects", "blog", "novels", "scraps", "activity", "movies", "books", "links"]

[integrations]
github_username = "myid"
tmdb_api_key_env = "OXIPAGE_TMDB_KEY"
aladin_ttbkey_env = "OXIPAGE_ALADIN_TTBKEY"

[lobby]
default_mode = "grid"

[lobby.overrides]
projects = "canvas"
links = "list"
```

비밀 값(API 키)은 토큰 자체가 아니라 **환경변수 이름**만 설정 파일에 적어, 설정 파일을 git에 커밋해도 안전하도록 합니다.

## 4.5 REST API 설계

- 버전 프리픽스: `/api/console/**`
- 리소스별 CRUD: `GET/POST /api/console/blog/posts`, `GET/PATCH/DELETE /api/console/blog/posts/{slug}` 식으로 확장 id가 그대로 경로 세그먼트가 됨(§1.4 `Extension::id()`와 일치)
- 목록 응답은 커서 기반 페이지네이션: `?cursor=...&limit=20`
- 응답 봉투: 목록은 `{ "data": [...], "meta": { "next_cursor": ... } }`, 단건은 `{ "data": {...} }`
- 에러 포맷: `{ "error": { "code": "validation_error", "message": "...", "field": "title_ko" } }`
- 쓰기 요청은 `Authorization: Bearer <token>` 필수, 스코프 검사(§4.2) 실패 시 403
- **OpenAPI 스펙 자동 생성**: Rust 코드의 라우트/타입 주석에서 `utoipa` 크레이트로 OpenAPI 문서를 생성하고, `/api/console/docs`에 Swagger UI로 서빙 — 사람이 봐도, 에이전트가 스펙을 긁어서 사용법을 익혀도 유용
- 읽기 API는 공개(레이트리밋만 적용), 쓰기 API는 오너 토큰 필요 — 공개 사이트인 만큼 방문자가 리뷰/블로그를 보는 데는 로그인 불필요

## 4.6 oh-my-pi용 스킬 설계

oh-my-pi(omp)는 `SKILL.md` 기반 스킬을 여러 위치에서 읽는데, 그중 omp 고유의 정식 위치는 저장소의 `.agent/skills/` (또는 `.agents/skills/`) 이며, `.claude/skills/`류 기존 관례 위치도 함께 인식합니다. 따라서 Oxipage 저장소 루트에 아래처럼 배치합니다.

```
oxipage/
└── .agent/
    └── skills/
        └── oxipage-cli/
            └── SKILL.md
```

**SKILL.md 초안 (그대로 시작점으로 써도 되는 수준):**

```markdown
---
name: oxipage-cli
description: >
  개인 홈페이지(Oxipage)에 콘텐츠를 발행할 때 사용하는 CLI 스킬입니다.
  블로그 글 작성, 소설 챕터 추가, 프로젝트 포트폴리오 등록, 영화/도서 리뷰
  기록, 해커뉴스·긱뉴스 스크랩, 생태계 링크 추가 요청에 반응하세요.
  "블로그에 올려줘", "이 영화 리뷰 써줘", "이거 스크랩해줘",
  "새 프로젝트 포트폴리오에 추가해줘" 같은 요청이 트리거입니다.
---

# Oxipage CLI 사용법

## 원칙
- 모든 `add`/`new` 명령은 **초안만** 만듭니다. 사용자가 "게시해줘/publish해줘"라고
  명시적으로 말하기 전까지 절대 `--publish`나 `publish` 명령을 쓰지 마세요.
- 출력은 항상 `--json` 플래그를 붙여 파싱하세요.
- 실패 시(특히 403/스코프 부족) 재시도하지 말고 사용자에게 어떤 권한이
  부족한지 그대로 보고하세요.

## 인증(토큰)

- 이 스킬을 쓰려면 oh-my-pi 실행 환경에 Oxipage PAT가 **사전 프로비저닝**되어 있어야 합니다:
  1. 오너가 로컬에서 `oxipage auth token create --label omp-agent --scopes post:write` (에이전트엔 `post:publish`를 주지 않습니다 — 초안 우선 원칙).
  2. 발급받은 평문 토큰을 oh-my-pi 환경변수 `OXIPAGE_TOKEN`으로 주입. 평문은 이때 한 번만 보이므로 즉시 환경에 넣으세요.
  3. CLI는 `OXIPAGE_TOKEN`이 있으면 자동으로 `Authorization: Bearer` 헤더에 붙입니다. 없으면 모든 쓰기 명령이 401로 실패합니다.
- 토큰 만료·스코프 부족(403)이면 **재발급을 사용자에게 요청** — 스킬 스스로 토큰을 발급하거나 권한을 올리지 않습니다.

## 워크플로우 예시

### 블로그 글 작성
1. 사용자의 요청 내용을 마크다운 본문으로 정리해 임시 파일로 저장
2. `oxipage blog new "<제목>" --lang ko --file <임시파일> --json`
3. 결과의 `data.slug`를 사용자에게 알려주고, 게시 여부를 물음
4. 게시 승인 시에만 `oxipage blog publish <slug> --json`

### 영화 리뷰
1. `oxipage review movie add --tmdb "<제목>" --json` 으로 후보 검색
2. 여러 후보가 반환되면 사용자에게 선택지를 보여주고 확인받음
3. 평점/리뷰 텍스트가 없으면 사용자에게 물어봄(임의로 지어내지 않음)
4. 시리즈 언급이 있으면(`--series "<시리즈명>"`) 기존 시리즈 그룹 존재 여부를
   `oxipage review movie series list --json`으로 먼저 확인

### 스크랩
1. URL만 주어지면 `oxipage scrap add <url> --json`으로 메타데이터만 우선 저장
2. 사용자가 코멘트를 함께 주면 `--note` 로 같이 전달
3. 출처(hn/geeknews)를 알 수 없으면 `manual`로 둠

## 하지 말아야 할 것
- 사용자가 요청하지 않은 콘텐츠를 생성해서 게시하지 않기
- 평점·리뷰 내용을 사용자 발화 없이 임의로 창작하지 않기
- `--publish` 없이 만든 초안을 별도 확인 없이 자동으로 발행하지 않기
```

## 4.7 모델 무관성

zai GLM 5.2, MiniMax M3, DeepSeek V4 등 oh-my-pi가 라우팅하는 모델이 무엇이든 CLI 사용법은 동일해야 합니다. 이를 위해:

- CLI 출력은 항상 안정적인 JSON 스키마(`--json`)를 지원 — 모델의 자연어 파싱 능력에 의존하지 않음
- 명령 실패 시 에러 메시지가 기계가 읽어도 다음 행동을 알 수 있을 만큼 구체적(§4.5 에러 포맷과 동일 구조를 CLI stderr에도 반영)
- 위험한 동작(발행, 삭제)은 항상 별도 명령/플래그로 분리해, 어떤 모델이 스킬을 실행하든 "생성"과 "공개"가 우발적으로 한 번에 일어나지 않도록 함

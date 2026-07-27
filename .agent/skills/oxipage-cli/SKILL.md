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
  1. 오너가 로컬에서 `oxipage auth token create --label omp-agent --scope post:write` (에이전트엔 `post:publish`를 주지 않습니다 — 초안 우선 원칙).
  2. 발급받은 평문 토큰을 oh-my-pi 환경변수 `OXIPAGE_TOKEN`으로 주입. 평문은 이때 한 번만 보이므로 즉시 환경에 넣으세요.
  3. CLI는 `OXIPAGE_TOKEN`이 있으면 자동으로 `Authorization: Bearer` 헤더에 붙입니다. 없으면 모든 쓰기 명령이 401로 실패합니다.
- **Phase 1 전환기:** 아직 PAT 체계가 완비되지 않았습니다. 현재는 `OXIPAGE_ADMIN_TOKEN`(서버 측)과 동일한 값을 `OXIPAGE_TOKEN`(에이전트 환경)으로 주입하거나 `oxipage auth token set <token>`으로 credentials 파일에 저장합니다. PAT 스코프 분리는 Phase 4에서 완료 예정입니다.
- 토큰 만료·스코프 부족(403)이면 **재발급을 사용자에게 요청** — 스킬 스스로 토큰을 발급하거나 권한을 올리지 않습니다.

## 현재 지원 명령 (Phase 1-3)

```text
oxipage init                                   # oxipage.toml 스캐폴딩
oxipage status [--json]                         # 서버 상태 요약
oxipage serve [--port 8787]                     # 로컬 개발 서버 기동
oxipage auth token set <token>                  # credentials 저장 (Phase 1)
oxipage blog new "<제목>" [--lang ko|en] [--file DRAFT.md] [--tag t1 --tag t2] [--json]
oxipage blog publish <slug> [--json]
oxipage blog list [--draft] [--lang ko] [--json]
oxipage blog show <slug> [--json]
oxipage blog edit <slug> [--title ...] [--file BODY.md] [--tag ...] [--json]
oxipage blog rm <slug> [--json]
oxipage project add --title-ko "..." --title-en "..." [--desc-ko F] [--desc-en F] \
    [--tech rust,react] [--link repo=URL] [--status wip|active|archived] [--featured] [--publish]
oxipage project publish <slug>
oxipage project list [--status ...]
oxipage project show <slug>
oxipage link add --title "..." --url "..." [--desc-ko ...] [--desc-en ...] [--featured]
oxipage link list
oxipage link rm <id>
oxipage lobby layout set <extension> --mode canvas|grid|list
oxipage lobby config [--json]
```

> Phase 2 확장 명령(novel/review movie/review book/scrap/activity sync)은 각 확장의
> CLI 서브커맨드가 추가될 때 이 문서에 덧붙입니다. 현재는 API/웹으로 접근 가능합니다.

## 워크플로우 예시

### 블로그 글 작성
1. 사용자의 요청 내용을 마크다운 본문으로 정리해 임시 파일로 저장
2. `oxipage blog new "<제목>" --lang ko --file <임시파일> --json`
3. 결과의 `data.slug`를 사용자에게 알려주고, 게시 여부를 물음
4. 게시 승인 시에만 `oxipage blog publish <slug> --json`

### 프로젝트 포트폴리오 등록
1. `oxipage project add --title-ko "..." --title-en "..." --desc-ko FILE --desc-en FILE --tech rust,react --json`
2. 결과 slug 안내. `--publish`는 사용자 명시적 승인 때만.

### 스크랩(링크)
1. URL만 주어지면 `oxipage link add --title "..." --url "..." --json`으로 등록
2. 사용자가 코멘트를 주면 `--desc-ko`/`--desc-en`으로 전달

## 하지 말아야 할 것
- 사용자가 요청하지 않은 콘텐츠를 생성해서 게시하지 않기
- 평점·리뷰 내용을 사용자 발화 없이 임의로 창작하지 않기
- `--publish` 없이 만든 초안을 별도 확인 없이 자동으로 발행하지 않기
- 토큰이 없다고 해서 하드코딩된 가짜 토큰으로 시도하지 않기

# 12장 — Admin Console (관리 GUI)

## 12.1 동기

oxipage는 CLI가 유일한 관리 인터페이스다. 확장 활성/비활성, 게시물 작성·발행, 데이터 조회, 로비 설정 변경 — 모든 작업이 터미널에서 이루어진다. 이는 스크립트·자동화에는 이상적이지만, 시각적 탐색(게시물 목록 스크롤, 스크린샷 미리보기, 확장 상태 한눈에 보기)이나 복잡한 편집(마크다운 에디터, 태그 관리, 테마 미리보기)에는 부적합하다.

이 장은 **로컬 전용 관리 GUI**를 도입한다. 핵심 원칙:

- **호스트 바이너리(`oxipage`)가 도는 로컬에서만 실행.** 원격 접근 불필요, 불허.
- **사이트별 관리.** `~/.config/oxipage/sites.toml`(doc/09)에 등록된 여러 사이트(로컬 self-host, 클라우드 VM, fly.io 등)를 GUI에서 전환하며 각각 관리.
- **서버에 얹지 않는다.** `/admin`을 공개 서버에 마운트하면 0.0.0.0 노출 시 함께 노출되고, 단일 서버 인스턴스만 관리 가능해 "사이트별로"가 성립하지 않는다.

## 12.2 아키텍처 개요

```
┌─────────────────────────────────────────────────────────────────┐
│  oxipage admin (로컬 전용, 127.0.0.1:8788)                      │
│                                                                 │
│  ┌──────────────┐    ┌──────────────────────────────────────┐   │
│  │  Admin SPA   │◄──►│  Admin Backend (axum)                │   │
│  │  (임베드)    │    │                                      │   │
│  │  React 19    │    │  /api/admin/*  ← 로컬 전용 API       │   │
│  │  Tailwind v4 │    │    ├─ /sites     (sites.toml CRUD)   │   │
│  │  OKLCH 토큰  │    │    ├─ /proxy/*   (사이트 프록시)     │   │
│  └──────────────┘    │    └─ /themes    (테마 카탈로그)     │   │
│                      └──────────┬───────────────────────────┘   │
└─────────────────────────────────┼───────────────────────────────┘
                                  │ HTTP (Bearer token)
                    ┌─────────────┼─────────────┐
                    ▼             ▼             ▼
              ┌──────────┐ ┌──────────┐ ┌──────────┐
              │ selfhost │ │ alibaba  │ │ flyio    │
              │ :8787    │ │ cloud VM │ │ fly.dev  │
              └──────────┘ └──────────┘ └──────────┘
```

**데이터 흐름:**

1. 브라우저는 `http://127.0.0.1:8788`의 Admin SPA를 로드.
2. SPA는 모든 API 호출을 **동일 오리진** `http://127.0.0.1:8788/api/admin/*`로 보냄.
3. Admin Backend가 요청을 해석:
   - `/api/admin/sites` → 로컬 `sites.toml` 직접 읽기/쓰기
   - `/api/admin/proxy/{site}/{path...}` → 해당 사이트의 endpoint로 토큰 붙여 전달
   - `/api/admin/themes` → 내장 테마 카탈로그 반환
4. 토큰은 백엔드에서만 취급. 브라우저 JS에는 절대 노출되지 않음.

## 12.3 포트와 바인딩

| 항목 | 값 |
|------|-----|
| 기본 포트 | **8788** (공개 서버 8787의 +1, 직관적) |
| 바인딩 주소 | `127.0.0.1` 고정 (변경 불가 — 보안 경계) |
| 변경 방법 | `--port <N>` flag 또는 `OXIPAGE_ADMIN_PORT` env |
| 충돌 시 | 즉시 에러 + 대안 포트 제안 (`8789`, `8790`, ...) |

## 12.4 보안 모델

로컬 전용이므로 공격 표면이 극히 작지만, 원칙을 명시:

1. **127.0.0.1 고정 바인딩.** `--host` flag 없음. 0.0.0.0 불가.
2. **토큰은 백엔드 측.** SPA는 사이트 토큰을 never see. 프록시 레이어가 `sites.toml`에서 읽어 `Authorization: Bearer` 헤더를 붙임.
3. **프록시 대상 제한.** `{site}` 세그먼트는 `sites.toml`에 등록된 이름만 허용. 임의 URL 프록시 불가 (SSRF 차단).
4. **CSRF 무관.** SameSite 쿠키 없음, 모든 요청은 `Content-Type: application/json` + 커스텀 헤더(`X-Admin-Request: 1`).
5. **CORS 없음.** 동일 오리진(127.0.0.1:8788)에서만 SPA 서빙.

## 12.5 백엔드 — `oxipage-admin` 크레이트

### 구조

```
crates/oxipage-admin/
├── Cargo.toml
└── src/
    ├── lib.rs          # run_admin() 진입점
    ├── proxy.rs        # 사이트 프록시 레이어
    ├── sites_api.rs    # sites.toml CRUD API
    └── themes.rs       # 테마 카탈로그 + 적용 API
```

### 의존성

```toml
[dependencies]
anyhow.workspace = true
axum.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
toml.workspace = true
directories.workspace = true
rust-embed.workspace = true
tower-http = { workspace = true, features = ["cors"] }
tracing.workspace = true
tracing-subscriber.workspace = true
```

`oxipage-cli`의 `sites.rs` 모듈을 **공유 라이브러리로 추출**하지 않고, `oxipage-admin`에서 동일 로직을 재구현한다. 이유:

- `sites.rs`는 130줄, 의존성 제로(serde + toml + directories)
- 추출 시 `oxipage-sites` 크레이트 신설 → 워크스페이스 멤버 +1, 양쪽 의존성 변경
- 재구현 비용 < 추출 비용 (한 번 쓰고 마는 코드 아님 — 양쪽이 독립적으로 진화할 수 있음)

### `run_admin()` 진입점

```rust
pub async fn run_admin(port: u16) -> anyhow::Result<()> {
    // 1. tracing 초기화
    // 2. sites.toml 로드 (없으면 빈 상태로 시작)
    // 3. reqwest::Client 생성 (timeout 30s, TLS 설정)
    // 4. Router 조립
    // 5. 127.0.0.1:{port} 바인딩 — 실패 시 대안 포트 제안
    // 6. axum::serve + graceful shutdown
}
```

### 라우트 테이블

```
GET  /                           → SPA index.html (RustEmbed)
GET  /assets/*                   → SPA 정적 에셋 (RustEmbed)
GET  /api/admin/sites            → 사이트 목록
POST /api/admin/sites            → 사이트 추가
PUT  /api/admin/sites/{name}     → 사이트 수정
DELETE /api/admin/sites/{name}   → 사이트 삭제
GET  /api/admin/sites/active     → 현재 활성 사이트
PUT  /api/admin/sites/active     → 활성 사이트 전환
GET  /api/admin/themes           → 테마 카탈로그
GET  /api/admin/proxy/{site}/*   → 사이트 프록시 (GET/POST/PUT/PATCH/DELETE)
```

## 12.6 사이트 프록시 레이어

### 요청 변환

```
클라이언트:  GET /api/admin/proxy/selfhost/api/v1/extensions
백엔드:      GET {selfhost.endpoint}/api/v1/extensions
             + Authorization: Bearer {selfhost.token}
             + Content-Type: application/json (원본 유지)
```

### 구현

```rust
async fn proxy_handler(
    State(ctx): State<AdminContext>,
    Path((site, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AdminError> {
    // 1. sites.toml에서 site 조회 — 없으면 404
    // 2. endpoint + "/" + path 조립 (경로 순회 검증)
    // 3. reqwest로 전달 (method, headers 필터, body)
    // 4. Authorization: Bearer {token} 추가 (token 없으면 생략)
    // 5. 응답 status/headers/body 그대로 반환
}
```

### 에러 처리

| 상황 | 응답 |
|------|------|
| 사이트 이름 미등록 | 404 `{"error": "site_not_found"}` |
| 사이트 unreachable | 502 `{"error": "upstream_unreachable", "detail": "..."}` |
| 업스트림 4xx/5xx | 그대로 전달 (status + body) |
| 타임아웃 (30s) | 504 `{"error": "upstream_timeout"}` |

## 12.7 테마 시스템 (신규 백엔드 기능)

### 개념

**테마(Theme)** 는 공개 사이트의 시각적 아이덴티티를 결정하는 명명된 프리셋이다. 각 테마는:

- OKLCH 기반 색상 팔레트 (light/dark variant)
- 폰트 스택 조합
- 시그니처 CSS 변수 오버라이드

를 묶는다. 테마는 **사이트별 DB**에 저장되며, 공개 웹이 부팅 시 적용한다.

### 테마 카탈로그 (v1: 4종)

| ID | 이름 | 컨셉 | Light 배경 | Dark 배경 | 악센트 |
|----|------|------|-----------|----------|--------|
| `paper` | Paper (기본값) | 종이와 잉크, 현재 디자인 | `oklch(98.5% 0.004 95)` | `oklch(13% 0.020 265)` | 인디고-바이올렛 290° |
| `midnight` | Midnight | 깊은 밤, 코딩 화면 | `oklch(96% 0.005 250)` | `oklch(10% 0.025 265)` | 시안-블루 230° |
| `sepia` | Sepia | 오래된 책, 따뜻한 독서 | `oklch(96% 0.02 80)` | `oklch(15% 0.015 60)` | 앰버-골드 70° |
| `forest` | Forest | 이끼 낀 돌, 자연 | `oklch(97% 0.01 145)` | `oklch(12% 0.02 155)` | 에메랄드 155° |

### 저장소

각 사이트의 SQLite DB에 `theme_config` 테이블:

```sql
CREATE TABLE IF NOT EXISTS theme_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- 싱글턴 행
    theme_id TEXT NOT NULL DEFAULT 'paper',
    custom_overrides TEXT,                   -- JSON, 미래용 (v1: NULL)
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### API (공개 서버 측, `oxipage-core`에 추가)

```
GET  /api/v1/theme          → 현재 테마 설정 (인증 불요, 공개 웹이 읽음)
PUT  /api/v1/theme          → 테마 변경 (admin 스코프)
```

응답 형식:

```json
{
  "data": {
    "theme_id": "paper",
    "theme": {
      "id": "paper",
      "name": { "ko": "종이", "en": "Paper" },
      "palette": {
        "light": { "canvas": "oklch(98.5% 0.004 95)", "surface": "...", ... },
        "dark": { "canvas": "oklch(13% 0.020 265)", ... }
      },
      "fonts": { "body": "...", "display": "...", "mono": "..." }
    }
  }
}
```

### 공개 웹 적용

`web/src/shared/theme.ts`가 부팅 시 `GET /api/v1/theme`을 호출, 반환된 팔레트를 `document.documentElement.style`에 CSS 변수로 주입. 기존 `data-theme` 토글(light/dark)은 유지 — 테마가 각 variant의 팔레트를 제공하므로.

### Admin GUI에서의 테마 UX

- 테마 카드 4종 그리드, 실시간 미리보기 (미니 공개 페이지 렌더)
- 클릭 → 즉시 `PUT /api/admin/proxy/{site}/api/v1/theme` 호출
- "현재 적용됨" 배지

## 12.8 프론트엔드 — `admin-web/`

### 스택

| 항목 | 선택 | 이유 |
|------|------|------|
| 프레임워크 | React 19 | 기존 `web/`과 동일, 생태계 공유 |
| 빌드 | Vite 7 | 동일 |
| 스타일 | Tailwind v4 + OKLCH 토큰 | 기존 디자인 시스템 재사용 |
| 상태/데이터 | TanStack Query v5 | 동일 |
| 라우팅 | react-router v7 | 동일 |
| 아이콘 | lucide-react | 동일 |
| UI 프리미티브 | Radix UI | 동일 (dialog, dropdown, tabs, tooltip, switch) |

### 디렉토리 구조

```
admin-web/
├── index.html
├── package.json
├── vite.config.ts
├── tsconfig.json
└── src/
    ├── main.tsx
    ├── App.tsx                    # 라우터 + 셸
    ├── shared/
    │   ├── tokens.css             # web/src/shared/tokens.css 복사 + admin 전용 확장
    │   ├── admin.css              # 사이드바, 테이블, 폼 밀도 스타일
    │   ├── api.ts                 # Admin API 클라이언트
    │   └── ui/                    # 재사용 컴포넌트 (Button, Card, Badge, Switch, Table...)
    ├── shell/
    │   ├── AdminShell.tsx         # 사이드바 + 콘텐츠 영역 레이아웃
    │   ├── SiteSwitcher.tsx       # 상단 사이트 셀렉터
    │   └── Sidebar.tsx            # 내비게이션
    ├── dashboard/
    │   └── DashboardPage.tsx      # 개요: 서버 상태, 확장 요약, 최근 게시물
    ├── extensions/
    │   └── ExtensionsPage.tsx     # 확장 목록, 활성/비활성 토글, purge
    ├── content/
    │   ├── BlogListPage.tsx       # 블로그 게시물 테이블 (초안/발행 필터)
    │   ├── BlogEditorPage.tsx     # 마크다운 에디터 (작성/수정)
    │   └── DataBrowserPage.tsx    # 나머지 확장 데이터 테이블 (확장별 탭)
    ├── themes/
    │   └── ThemesPage.tsx         # 테마 카드 그리드 + 미리보기 + 적용
    └── settings/
        ├── SettingsPage.tsx       # 사이트 설정 (이름, URL, 언어)
        └── TokensPage.tsx         # PAT 관리 (생성/목록/폐기)
```

### 라우트

```
/                        → DashboardPage
/extensions              → ExtensionsPage
/content/blog            → BlogListPage
/content/blog/new        → BlogEditorPage (작성)
/content/blog/:slug      → BlogEditorPage (수정)
/content/:extId          → DataBrowserPage (확장별 데이터)
/themes                  → ThemesPage
/settings                → SettingsPage
/settings/tokens         → TokensPage
```

### 디자인 방향

**"조용한 작업실의 관리 데스크"** — 공개 사이트의 paper/ink 미학을 유지하되, 콘솔 밀도:

- **사이드바**: `--p-neutral-950` 배경 (다크 고정), 인디고 악센트 활성 표시
- **콘텐츠 영역**: 라이트/다크 토글 가능, 기본 라이트
- **타이포**: 본문 Pretendard, 수치/코드 SF Mono, 제목 Fraunces (공개 사이트와 동일)
- **밀도**: 테이블 행 높이 40px, 카드 간격 12px — 공개 사이트(여유)보다 조밀
- **상태색**: success/danger 토큰 그대로, 활성/비활성 배지에 사용
- **모션**: `--duration-fast` (120ms) 전환, hover lift 없음 (콘솔은 정적)

### 핵심 화면 설계

#### Dashboard

```
┌─────────────────────────────────────────────────────────┐
│ [사이트: selfhost ▾]              [🌙] [⚙️]            │
├────────┬────────────────────────────────────────────────┤
│        │  서버 상태                                     │
│ 대시보드│  ┌─────────┐ ┌─────────┐ ┌─────────┐         │
│ 확장   │  │ ● 온라인 │ │ 9 확장  │ │ 42 게시물│         │
│ 콘텐츠 │  └─────────┘ └─────────┘ └─────────┘         │
│  └ 블로그│                                             │
│  └ ...  │  최근 게시물                                  │
│ 테마   │  ┌────────────────────────────────────────┐   │
│ 설정   │  │ 제목          상태     수정일          │   │
│        │  │ ...                                    │   │
│        │  └────────────────────────────────────────┘   │
└────────┴────────────────────────────────────────────────┘
```

#### Extensions

```
┌────────────────────────────────────────────────────────┐
│ 확장 관리                                              │
├────────────────────────────────────────────────────────┤
│ 이름          ID         상태      액션                │
│ ─────────────────────────────────────────────────────  │
│ 블로그        blog       ● 활성    [비활성] [Purge]    │
│ 프로젝트      projects   ● 활성    [비활성] [Purge]    │
│ 소설          novels     ○ 비활성  [활성]   [Purge]    │
│ ...                                                    │
├────────────────────────────────────────────────────────┤
│ WASM 확장 설치                                         │
│ [레지스트리에서 설치 ▾]  또는  [파일 업로드]           │
└────────────────────────────────────────────────────────┘
```

#### Blog Editor

```
┌────────────────────────────────────────────────────────┐
│ ← 목록    새 게시물                    [초안 저장] [발행]│
├────────────────────────────────────────────────────────┤
│ 제목: [________________________________]               │
│ 언어: [ko ▾]  태그: [tag1] [tag2] [+]                 │
│                                                        │
│ ┌──────────────────────────────────────────────────┐   │
│ │                                                  │   │
│ │  마크다운 에디터 (textarea, 모노스페이스)         │   │
│ │                                                  │   │
│ │                                                  │   │
│ └──────────────────────────────────────────────────┘   │
│                                                        │
│ ┌──────────────────────────────────────────────────┐   │
│ │  미리보기 (렌더된 HTML)                           │   │
│ └──────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────┘
```

#### Themes

```
┌────────────────────────────────────────────────────────┐
│ 블로그 테마                                            │
├────────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐   │
│ │ [미리보기]│ │ [미리보기]│ │ [미리보기]│ │ [미리보기]│   │
│ │          │ │          │ │          │ │          │   │
│ │  Paper   │ │ Midnight │ │  Sepia   │ │  Forest  │   │
│ │ ✓ 적용됨 │ │          │ │          │ │          │   │
│ └──────────┘ └──────────┘ └──────────┘ └──────────┘   │
└────────────────────────────────────────────────────────┘
```

## 12.9 CLI 통합

### `oxipage admin` 서브커맨드

```
oxipage admin [--port <N>]
```

- `oxipage-cli/src/main.rs`의 `Command` enum에 `Admin { port: Option<u16> }` 추가
- `oxipage-admin::run_admin(port.unwrap_or(8788))` 호출
- `serve`와 동일하게 "HTTP를 거치지 않는 예외" — 로컬 프로세스 직접 기동

### 출력

```
$ oxipage admin
oxipage management console listening on http://127.0.0.1:8788
sites: 3 registered (selfhost*, alibaba, flyio)
open http://127.0.0.1:8788 in your browser
```

`--open` flag (macOS `open`, Linux `xdg-open`)으로 자동 브라우저 오픈.

## 12.10 API 계약 요약

### Admin Backend (127.0.0.1:8788)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/admin/sites` | 사이트 목록 (token 마스킹) |
| POST | `/api/admin/sites` | 사이트 추가 `{name, endpoint, token?}` |
| PUT | `/api/admin/sites/{name}` | 사이트 수정 |
| DELETE | `/api/admin/sites/{name}` | 사이트 삭제 |
| GET | `/api/admin/sites/active` | 현재 활성 사이트 |
| PUT | `/api/admin/sites/active` | 활성 전환 `{name}` |
| GET | `/api/admin/themes` | 테마 카탈로그 (내장) |
| ANY | `/api/admin/proxy/{site}/*` | 사이트 프록시 |

### 공개 서버 추가 (oxipage-core)

| Method | Path | 설명 |
|--------|------|------|
| GET | `/api/v1/theme` | 현재 테마 (공개) |
| PUT | `/api/v1/theme` | 테마 변경 (admin) |

## 12.11 파일 변경 목록

### 신규

| 경로 | 설명 |
|------|------|
| `crates/oxipage-admin/` | Admin Backend 크레이트 |
| `crates/oxipage-admin/src/lib.rs` | `run_admin()` 진입점 |
| `crates/oxipage-admin/src/proxy.rs` | 사이트 프록시 |
| `crates/oxipage-admin/src/sites_api.rs` | sites.toml CRUD |
| `crates/oxipage-admin/src/themes.rs` | 테마 카탈로그 |
| `admin-web/` | Admin SPA (React + Vite) |
| `admin-web/src/**` | 프론트엔드 소스 |

### 변경

| 경로 | 변경 |
|------|------|
| `Cargo.toml` | workspace members에 `oxipage-admin` 추가 |
| `crates/oxipage-cli/Cargo.toml` | `oxipage-admin` 의존성 추가 |
| `crates/oxipage-cli/src/main.rs` | `Command::Admin` 추가 |
| `crates/oxipage-cli/src/commands/mod.rs` | admin dispatch 추가 |
| `crates/oxipage-core/src/http.rs` | `/api/v1/theme` GET/PUT 라우트 추가 |
| `crates/oxipage-core/src/migrate.rs` | `theme_config` 테이블 마이그레이션 |
| `web/src/shared/theme.ts` | 부팅 시 테마 API 호출 + CSS 변수 주입 |

### 미변경

- `oxipage-server` — 공개 서버 로직 변경 없음
- 기존 확장 크레이트 — 라우트/스키마 변경 없음
- `oxipage.toml` 스키마 — 테마는 DB 저장, toml 불변

## 12.12 구현 순서

```
Phase 1: 백엔드 기초
  ├─ oxipage-admin 크레이트 스캐폴드
  ├─ sites.toml CRUD API
  ├─ 사이트 프록시 레이어
  └─ CLI `oxipage admin` 서브커맨드

Phase 2: 테마 시스템
  ├─ theme_config 마이그레이션 (core)
  ├─ /api/v1/theme GET/PUT (core)
  ├─ 테마 카탈로그 (admin)
  └─ 공개 웹 테마 적용 (web/src/shared/theme.ts)

Phase 3: Admin SPA 기초
  ├─ admin-web 스캐폴드 (Vite + React + Tailwind)
  ├─ AdminShell (사이드바 + 콘텐츠)
  ├─ SiteSwitcher
  └─ DashboardPage

Phase 4: 핵심 화면
  ├─ ExtensionsPage (활성/비활성 토글)
  ├─ BlogListPage + BlogEditorPage
  ├─ DataBrowserPage (나머지 확장)
  └─ ThemesPage

Phase 5: 마무리
  ├─ SettingsPage + TokensPage
  ├─ RustEmbed으로 admin-web/dist 임베드
  └─ E2E 스모크 테스트
```

## 12.13 향후 확장 (v2)

본 설계 범위 밖:

1. **실시간 로그 스트리밍**: WebSocket으로 서버 로그를 Admin GUI에 표시
2. **미디어 라이브러리**: `data/media/` 파일 브라우저 + 업로드
3. **테마 커스텀 에디터**: OKLCH 팔레트를 GUI에서 직접 편집
4. **멀티 유저**: Admin GUI 자체 인증 (현재는 로컬 전용이라 불필요)
5. **확장 마켓플레이스 GUI**: 레지스트리 브라우징 + 원클릭 설치
6. **백업 스케줄링 UI**: cron 표현식 편집 + 이력 조회

## 12.14 레퍼런스

- §1.4 확장 레지스트리
- §1.8 인증 (AdminAuth, PAT)
- §2.13 확장 라이프사이클 (enable/disable/purge)
- §3.x 디자인 시스템 (OKLCH 토큰)
- §4.1 CLI는 API의 레퍼런스 클라이언트
- §9.x 멀티 사이트 (Site Profiles)

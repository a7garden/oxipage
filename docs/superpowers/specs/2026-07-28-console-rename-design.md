# Oxipage v2 — Console Rename Design

> 2026-07-28. v2 SSG 전환의 후속. "서버"라는 용어를 "콘솔"로 일관 변경한다.

## 1. 배경

v2 SSG 모델에서는 **공개 사이트를 위한 서버가 없다.** 정적 파일을 GitHub Pages 등에 배포하기 때문. 하지만 현재 코드와 문서는 "서버"라는 단어를 일관되게 사용하고 있어, "어떤 서버?"라는 혼란을 야기한다.

이름 매핑:

| v1 (잘못된) 용어 | v2 (올바른) 용어 | 의미 |
|---|---|---|
| "서버" (public) | **"정적 사이트"** | GitHub Pages가 호스팅 |
| "서버" (admin) | **"콘솔"** | 로컬 관리 도구 |
| `oxipage serve` | **`oxipage console`** | CLI 명령 |
| `oxipage-server` crate | **`oxipage-console` crate** | 라이브러리 |
| `oxipage-admin` crate | (제거) | 콘솔에 통합 |
| `/api/v1/...` | **`/api/console/...`** | HTTP 라우트 |

## 2. 범위

- 크레이트 이름 1개 리네임 (`oxipage-server` → `oxipage-console`)
- 크레이트 1개 제거 (`oxipage-admin` → `oxipage-console`에 통합)
- CLI 명령어 1개 리네임 (`oxipage serve` → `oxipage console`)
- HTTP 라우트 prefix 변경 (`/api/v1` → `/api/console`)
- 모든 import / 의존성 / 문서 / SKILL.md / README / plan / spec 갱신

## 3. 크레이트 재구성

### 3.1 현재 구조

```
crates/
├── oxipage-core/         # 코어 라이브러리 (변경 없음)
├── oxipage-server/        # 관리 HTTP 서버
├── oxipage-admin/         # 별도 admin 콘솔 바이너리
├── oxipage-cli/           # CLI (name = "oxipage")
├── oxipage-wasm/          # WASM 런타임 (변경 없음)
└── oxipage-ext-*/         # 9개 확장 (변경 없음)
```

### 3.2 목표 구조

```
crates/
├── oxipage-core/          # 코어 (변경 없음)
├── oxipage-console/       # 관리 콘솔 — 이전 server + admin 통합
│   ├── src/lib.rs         # run_console(), all_extensions(), all_builders()
│   ├── src/console_web.rs # 이전 admin-web (React SPA)
│   ├── src/main.rs        # 바이너리
│   └── ...
├── oxipage-cli/           # CLI (변경 없음)
├── oxipage-wasm/          # WASM (변경 없음)
└── oxipage-ext-*/         # 9개 확장 (변경 없음)
```

### 3.3 `oxipage-console` 의존성

- `oxipage-core` (코어 라이브러리)
- `axum` (HTTP 서버)
- `rust-embed` (콘솔 SPA 번들)
- 9개 `oxipage-ext-*` (확장)
- `oxipage-wasm` (feature-gated)
- (이전) `oxipage-admin`의 React SPA 번들

## 4. CLI 명령어 변경

### 4.1 `oxipage serve` → `oxipage console`

```
$ oxipage console [--port 8787] [--preview]
```

- `--preview`: `data/out/` 디렉토리 정적 서빙 (이전 `--serve --preview` 동일)
- `--port`: 콘솔 HTTP 포트

### 4.2 도움말 메시지 업데이트

```
$ oxipage console --help
Usage: oxipage console [OPTIONS]

  Start the local management console (admin web UI + API)

Options:
      --port <PORT>        [default: 8787]
      --preview            Serve out/ as static files
      --no-browser         Don't open browser on startup
  -h, --help               Print help
```

### 4.3 변경하지 않는 명령

`build`, `deploy`, `query`, `schema`, `backup`, `cache`, `blog`/`project`/`link` CRUD, `auth`, `init`, `status`, `lobby` — 개념이 명확하므로 그대로 유지.

## 5. HTTP 라우트

### 5.1 `/api/v1` → `/api/console`

```
Before                              After
────────────────────────            ─────────────────────
POST /api/v1/build                  POST /api/console/build
POST /api/v1/cache/refresh          POST /api/console/cache/refresh
GET  /api/v1/lobby/manifest         GET  /api/console/lobby/manifest
GET  /api/v1/lobby/config           GET  /api/console/lobby/config
PUT  /api/v1/lobby/config/{id}      PUT  /api/console/lobby/config/{id}
GET  /api/v1/auth/tokens            GET  /api/console/auth/tokens
POST /api/v1/auth/tokens            POST /api/console/auth/tokens
DELETE /api/v1/auth/tokens/{id}    DELETE /api/console/auth/tokens/{id}
GET  /api/v1/search                 GET  /api/console/search
GET  /api/v1/docs                   GET  /api/console/docs
GET  /api/v1/docs/openapi.json      GET  /api/console/docs/openapi.json
GET  /api/v1/extensions             GET  /api/console/extensions
POST /api/v1/extensions/{id}/enable POST /api/console/extensions/{id}/enable
POST /api/v1/extensions/{id}/disable POST /api/console/extensions/{id}/disable
DELETE /api/v1/extensions/{id}      DELETE /api/console/extensions/{id}
POST /api/v1/extensions/install     POST /api/console/extensions/install
POST /api/v1/backup/snapshot        POST /api/console/backup/snapshot
GET  /api/v1/cli/commands           GET  /api/console/cli/commands
POST /api/v1/cli/exec/{id}/{cmd}    POST /api/console/cli/exec/{id}/{cmd}
GET  /api/v1/theme                  GET  /api/console/theme
PUT  /api/v1/theme                  PUT  /api/console/theme
GET  /api/v1/themes                 GET  /api/console/themes
```

### 5.2 공개 라우트 (변경 없음)

- `/healthz` — load balancer용
- `/setup/*` — setup wizard

### 5.3 자동 redirect

`/api/v1/...` 경로로 들어오는 요청은 `/api/console/...`로 301 redirect (한 번에 마이그레이션 유도):

```rust
async fn api_v1_redirect(...) -> Response {
    let new_path = request.uri().path().replace("/api/v1/", "/api/console/");
    Redirect::permanent(&new_path).into_response()
}
```

## 6. 코드 변경

### 6.1 Crate 리네임 (Cargo workspace)

`Cargo.toml`:
```diff
-    "crates/oxipage-server",
+    "crates/oxipage-console",
```

`crates/oxipage-server/Cargo.toml`:
```diff
-[package]
-name = "oxipage-server"
+[package]
+name = "oxipage-console"
-version = "0.1.0"
```

`crates/oxipage-server/` → `crates/oxipage-console/` (디렉토리 리네임 + 파일 내용 갱신).

### 6.2 Crate 제거 (oxipage-admin)

- `crates/oxipage-admin/` 디렉토리 삭제
- `Cargo.toml`의 `members`에서 제거
- `oxipage-cli/Cargo.toml`의 의존성에서 제거
- `oxipage-console`은 admin-web SPA를 `rust-embed`로 포함

### 6.3 Public API 갱신

```rust
// crates/oxipage-console/src/lib.rs (이전 oxipage-server/src/lib.rs)
pub fn all_extensions() -> Vec<Arc<dyn Extension>> { ... }
pub fn all_builders() -> Vec<Box<dyn BuildExt>> { ... }
pub async fn run_console() -> anyhow::Result<()> { ... }
```

이전 `run_server`는 `#[deprecated]` 어노테이션과 함께 alias로 유지:

```rust
#[deprecated(note = "Use run_console() — server is now the management console")]
pub async fn run_server() -> anyhow::Result<()> { run_console().await }
```

### 6.4 oxipage-cli 변경

- `Command::Serve` → `Command::Console`
- `init_status_serve` 모듈 → `init_console_serve`로 리네임
- `serve()` 함수 → `console()`
- `--preview` 옵션 동일

### 6.5 모든 import 갱신

`grep -r "oxipage_server::"` 후 `oxipage_console::`로 변경.

영향받는 파일:
- `crates/oxipage-cli/Cargo.toml`
- `crates/oxipage-cli/src/commands/build.rs`
- `crates/oxipage-cli/src/commands/backup.rs`
- `crates/oxipage-cli/src/commands/mod.rs`
- 모든 `tests/*.rs`

## 7. 문서 갱신

### 7.1 README.md

"management console" / "console" 용어로 전면 갱신. "server"는 v1 호환성 언급 외에는 제거.

### 7.2 doc/00-overview.md, doc/01-architecture.md

아키텍처 다이어그램과 설명에서 "server" → "console" 또는 "management console"로 갱신.

### 7.3 doc/05-deployment-self-hosting.md

"console" 가이드로 재작성 (이전 self-hosting 가이드 대체).

### 7.4 doc/12-admin-console.md

이미 "admin-console"라는 파일명이지만, v2에서 "console"로 통합되므로 `doc/12-console.md`로 리네임.

### 7.5 SKILL.md

- `oxipage serve` → `oxipage console`
- API endpoint 예시 `/api/v1/...` → `/api/console/...`

### 7.6 설계 문서 / 계획

- `docs/superpowers/specs/2026-07-28-static-site-generator-design.md` — "server" 단어 점검
- `docs/superpowers/plans/2026-07-28-ssg-implementation.md` — 이미 `oxipage-server`로 작성됨, plan 갱신 필요시 갱신
- `docs/superpowers/specs/2026-07-28-console-rename-design.md` (이 문서) — 새

## 8. 작업 영향

| 영향 | 범위 |
|---|---|
| **API breaking change** | `/api/v1/*` → `/api/console/*` (redirect는 제공) |
| **CLI breaking change** | `oxipage serve` → `oxipage console` (alias 제공하지 않음) |
| **Crate import breaking** | `oxipage_server::*` → `oxipage_console::*` (코드 전체 검색으로 처리) |
| **데이터/마이그레이션** | 없음 — DB 스키마 동일 |

## 9. 호환성

- `/api/v1/*` → `/api/console/*` 301 redirect로 호환
- `oxipage serve` 명령어는 완전 제거 (대안 `oxipage console`)
- `oxipage_server` crate의 public 함수는 `#[deprecated]`로 1 minor release 동안 유지
- `oxipage-admin` crate는 완전 제거 (1 minor release 동안 호환 alias 안 둠 — 콘솔에 통합)

## 10. 완료 기준

- `cargo check --workspace` 클린
- `cargo test --workspace` 모든 테스트 통과
- `cargo clippy --workspace --all-targets -- -D warnings` 클린
- `oxipage console` 명령어 정상 기동
- `/api/v1/blog/posts` 요청 시 `/api/console/blog/posts`로 redirect
- 모든 문서에서 "server"가 v1 컨텍스트 외에는 사용되지 않음
- `crates/oxipage-admin/` 디렉토리 완전 삭제
- `crates/oxipage-server/` 디렉토리 `oxipage-console/`로 리네임

## 11. 마이그레이션 가이드 (사용자용)

### 이전 (v1)
```bash
oxipage serve          # 서버 기동
oxipage blog new ...   # HTTP API 호출
```

### 이후 (v2)
```bash
oxipage console        # 관리 콘솔 기동
oxipage blog new ...   # 콘솔 API 호출 (동일, 라우트만 변경)
```

### 외부 API 마이그레이션
```
/api/v1/blog/posts  →  /api/console/blog/posts  (301 redirect)
```

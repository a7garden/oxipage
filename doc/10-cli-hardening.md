> **범위:** CLI hardening (robustness, tests, structure). Missing content-type commands
> are documented in §10.0 but intentionally excluded — each is a feature-design task,
> not a hardening fix.
# 10장 — CLI 하드닝 (Client · 테스트 · 모듈 분할)


## 10.0 범위 결정: hardening vs 새 명령

이 계획은 **기존 코드의 안전장치 공백을 메우는 hardening**에 집중한다. 아래 CLI 미구현 명령들은 **의도적으로 범위 밖**이다:

| 누락 명령군 | 서버 API 상태 | CLI 상태 | 근거 |
|------------|-------------|---------|------|
| `novel new/list/chapter add/publish` | ✅ routes + tests 완비 | ❌ | 신규 feature — 서버는 완성, CLI 명령 + UX 설계 필요 |
| `review movie add/series create` | ✅ routes + tests 완비 | ❌ | 신규 feature |
| `review book add` | ✅ routes + tests 완비 | ❌ | 신규 feature |
| `scrap add/queue/delete` | ✅ routes + tests (api.rs 16KB) | ❌ | 신규 feature |
| `activity sync` | ✅ routes + tests 완비 | ❌ | 신규 feature — CLI는 수동 trigger만 제공 |
| `auth login` (브라우저 OAuth) | ⚠️ PAT 체계 Phase 4 | ❌ stub | Phase 4 의존 — 별도 계획 |
| `oxipage deploy` | N/A | ❌ | Phase 5 의존 |
| `profile edit/show` | N/A | ❌ | 별도 feature |

**서버가 제공하는 HTTP API는 약 39개 명령에 대응.** 현재 CLI는 그중 약 30개를 구현 (77%). 누락된 9개 중 5개는 novels/movies/books/scraps/activity content-type 명령으로, 각각이 자체적인 CLI 서브커맨드 설계를 필요로 한다.

**이 계획이 다루는 것:** resolve chain 테스트, Client timeout/수명주기, commands.rs 분할, OXIPAGE_SITE env 에러 처리, --insecure, E2E smoke test.

**별도 계획이 필요한 것:** 위 표의 모든 누락 명령군 — 각 extension의 API 스펙에 맞춘 CLI 명령 설계.

## 10.1 동기

`oxipage-cli` crate 1623 LOC 중 `sites.rs`(274 LOC)에만 10건의 단위 테스트가 있고, 나머지 4개 모듈(`commands.rs` 1009 LOC, `client.rs`, `credentials.rs`, `output.rs`)은 테스트 커버리지 0%다. 특히 `resolve_endpoint`(7단계 우선순위 체인)와 `Client::request`(HTTP 전송)는 한 단계의 우선순위 오류나 timeout 누락만으로도 명령이 엉뚱한 서버로 발행되거나 CLI가 무한 블록되는 결함으로 이어진다.

동시에 `commands.rs`는 1009줄에 달해 단일 파일이 모든 서브커맨드(blog, project, link, lobby, extension, backup, auth, site)를 담고 있어 변경 충돌 위험이 높다.

## 10.2 대상 문제

| 문제 | 파일 | 심각도 |
|------|------|--------|
| `resolve_endpoint` / `resolve_token` / `resolve_site_name` 무검증 | `commands.rs:275-335` | **높음** — 오발행 가능 |
| `Client::new()` 8회 반복 생성 — 매 호출 `reqwest::Client` 재빌드 | `commands.rs:578,661,705,811,883,924,952,999` | **중간** — 비효율 + timeout 적용 차단 |
| reqwest timeout 미설정 — 원격 hang → CLI 영구 블록 | `client.rs:28-39` | **중간** — 무응답 무한 대기 |
| TLS cert 검증 옵션 없음 — 자체서명 인증서 셀프호스트 불가 | `client.rs:28-39` | **중간** |
| `OXIPAGE_SITE` env가 unknown 사이트를 조용히 무시 → default 폴백 | `commands.rs:264-269` | **낮음** — 침묵 동작, 추적 어려움 |
| `commands.rs` 1009줄 — 단일 파일에 모든 cmd 집중 | `commands.rs` | **낮음** — 유지보수 부채 |

## 10.3 아키텍처 변경

### 10.3.1 Client 수명주기

**Before (현재):**
```
dispatch(cli)
  → endpoint, token resolve
  → match command
      → handler(endpoint: &str, token: &Option<String>)
          → Client::new(endpoint.to_string(), token.clone())?  // 매번 생성
          → client.get("/api/...")
```

**After (변경 후):**
```
dispatch(cli)
  → endpoint, token resolve
  → client = Client::new(endpoint, token)?  // 단 한 번 생성
  → match command
      → handler(client: &Client)             // 재사용
          → client.get("/api/...")
```

변경 영향: 모든 서브커맨드 핸들러 시그니처가 `(client: &Client, ...)` 로 바뀐다. `require_token()` 호출 위치도 핸들러 내부에서 dispatch로 이전한다.

### 10.3.2 Client 빌드 설정

```rust
// client.rs:28-39 — 변경 후
pub fn new(endpoint: String, token: Option<String>, insecure: bool) -> anyhow::Result<Self> {
    let endpoint = endpoint.trim_end_matches('/').to_string();
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("oxipage-cli/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60));
    if insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let http = builder.build().context("failed to build HTTP client")?;
    Ok(Client { endpoint, token, http })
}
```

### 10.3.3 `--insecure` 플래그

`main.rs` `Cli` struct에 추가:
```rust
#[arg(long, global = true, env = "OXIPAGE_TLS_INSECURE")]
pub insecure: bool,
```

`dispatch()`에서 `Client::new(endpoint, token, cli.insecure)?` 로 전달. TLS 검증 스킵은 `--insecure` 또는 `OXIPAGE_TLS_INSECURE=1` env로만 활성화되며, 기본값은 검증 유지.

### 10.3.4 `OXIPAGE_SITE` env unknown 처리

`resolve_site_name()` 에서 OXIPAGE_SITE env에 지정된 사이트가 존재하지 않으면, default_site로 조용히 폴백하지 않고 **에러**를 반환한다. flag와 동일한 수준의 명시적 의도로 취급한다.

```rust
// commands.rs — resolve_site_name() OXIPAGE_SITE 분기 변경
// Before:
if let Ok(env) = std::env::var("OXIPAGE_SITE") {
    if !env.is_empty() && sites_file.exists(&env) {
        return Ok(sites_file.resolve_name(None));
    }
}

// After:
if let Ok(env) = std::env::var("OXIPAGE_SITE") {
    if !env.is_empty() {
        if sites_file.exists(&env) {
            return Ok(sites_file.resolve_name(None));
        }
        anyhow::bail!("site '{env}' (from OXIPAGE_SITE env) not found — use `oxipage site add` to create it");
    }
}
```

### 10.3.5 commands.rs 분할 구조

```
crates/oxipage-cli/src/
  commands/               # 신규 디렉토리
    mod.rs                # dispatch, resolve_* 함수, require_token
    site.rs               # SiteCommand enum + handler functions
    blog.rs               # BlogCommand enum + handler
    project.rs            # ProjectCommand enum + handler
    link.rs               # LinkCommand enum + handler
    lobby.rs              # LobbyCommand enum + handler
    extension.rs          # ExtensionCommand enum + handler
    backup.rs             # BackupCommand enum + handler
    auth.rs               # AuthCommand + TokenCommand enum + handler
    init_status_serve.rs  # init, status, serve 함수 + DEFAULT_TOML
```

각 서브모듈이 자신의 `#[derive(Subcommand)]` enum을 소유한다. `main.rs`의 `Command` enum은 각 모듈의 enum을 `commands::blog::BlogCommand` 식으로 참조한다(현재와 동일 패턴).

## 10.4 CLI `--insecure` / `--site` 완성 명세

변경 후 전체 global flags:

```
--endpoint <url>        OXIPAGE_ENDPOINT       API server base URL
--token <tok>           OXIPAGE_TOKEN          Bearer token
--site <name>           OXIPAGE_SITE           Active site profile
--insecure              OXIPAGE_TLS_INSECURE   Accept invalid TLS certs
--json                                        JSON output mode
--config <path>         OXIPAGE_CONFIG         oxipage.toml path
```

## 10.5 구현 계획

총 7개 Task. **의존성**: Task 1 → Task 2 → 나머지는 독립적으로 parallel 배치 가능. Task 7(통합 검증)은 전체 완료 후.

---

### Task 1: Client 빌드 하드닝 (timeout + --insecure + 수명주기)

**Files:**
- Modify: `crates/oxipage-cli/src/client.rs:28-39`
- Modify: `crates/oxipage-cli/src/main.rs` — `--insecure` flag 추가
- Modify: `crates/oxipage-cli/src/commands.rs:174-247` — dispatch() 시그니처

**Interfaces:**
- Consumes: 현행 Client::new 시그니처
- Produces:
  - `Client::new(endpoint: String, token: Option<String>, insecure: bool) -> anyhow::Result<Self>`
  - `Cli.insecure: bool`
  - `dispatch(cli: Cli) -> anyhow::Result<()>` — 내부에서 Client 생성, `&Client`로 핸들러 호출

- [ ] **Step 1: Client::new에 timeout + insecure 추가**

```rust
// client.rs
use std::time::Duration;

pub fn new(endpoint: String, token: Option<String>, insecure: bool) -> anyhow::Result<Self> {
    let endpoint = endpoint.trim_end_matches('/').to_string();
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("oxipage-cli/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60));
    if insecure {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let http = builder.build()
        .context("failed to build HTTP client")?;
    Ok(Client { endpoint, token, http })
}
```

- [ ] **Step 2: main.rs에 --insecure flag 추가**

```rust
// main.rs — Cli struct 내
#[arg(long, global = true, env = "OXIPAGE_TLS_INSECURE")]
pub insecure: bool,
```

- [ ] **Step 3: dispatch()에서 Client 생성 후 핸들러에 &Client 전달**

```rust
// commands.rs — dispatch() 변경
pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let out = Output::new(cli.json);
    let sites_file = sites::SitesFile::load();
    let site_name = resolve_site_name(cli.site.as_deref(), &sites_file)?;
    let endpoint = resolve_endpoint(cli.endpoint.clone(), site_name, &sites_file, cli.config.as_deref())?;
    let token = resolve_token(cli.token.clone(), site_name, &sites_file)?;
    let client = Client::new(endpoint, token, cli.insecure)?;

    if let Command::Site(c) = &cli.command {
        return dispatch_site(c, &out, &sites_file, site_name).await;
    }

    match cli.command {
        Command::Init => init(&out, cli.config.as_deref()),
        Command::Status => status(&out, &client).await,
        Command::Serve { port } => serve(port, cli.config.as_deref()).await,
        Command::Auth(c) => auth(c, &out, &client).await,
        Command::Blog(c) => blog(c, &out, &client).await,
        // ... 나머지 동일 패턴 (endpoint, token → client)
        Command::Site(_) => unreachable!(),
    }
}
```

- [ ] **Step 4: 모든 핸들러 시그니처를 `(out, &client)` 로 변경**

각 핸들러에서 `let client = Client::new(...)` 라인을 제거하고, 파라미터를 `(out: &Output, client: &Client)`로 교체한다. `require_token(&client)` 호출은 유지(쓰기 명령).

변경 대상 함수 (정확한 시그니처):
```rust
async fn status(out: &Output, client: &Client) -> anyhow::Result<()>
async fn auth(c: AuthCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn blog(c: BlogCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn project(c: ProjectCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn link(c: LinkCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn lobby(c: LobbyCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn extension(c: ExtensionCommand, out: &Output, client: &Client) -> anyhow::Result<()>
async fn backup(c: BackupCommand, out: &Output, client: &Client) -> anyhow::Result<()>
```

- [ ] **Step 5: cargo build -p oxipage-cli → clean compile 확인**

- [ ] **Step 6: Commit**

```bash
git add crates/oxipage-cli/src/client.rs crates/oxipage-cli/src/main.rs crates/oxipage-cli/src/commands.rs
git commit -m "refactor(cli): single Client instance with timeout + --insecure flag"
```

---

### Task 2: resolve_* 단위 테스트

**Files:**
- Create: `crates/oxipage-cli/src/commands/test_resolve.rs` (또는 commands.rs 하단 `#[cfg(test)] mod tests`)
- Modify: `crates/oxipage-cli/src/commands.rs:264-269` — OXIPAGE_SITE env unknown → error

**Interfaces:**
- Consumes: `resolve_endpoint`, `resolve_token`, `resolve_site_name` (Task 1 이후 시그니처)
- Produces: 테스트 커버리지 (최소 15 test case)

- [ ] **Step 1: OXIPAGE_SITE unknown → error 수정**

```rust
// commands.rs:264-272 — resolve_site_name() OXIPAGE_SITE 분기
if let Ok(env) = std::env::var("OXIPAGE_SITE") {
    if !env.is_empty() {
        if sites_file.exists(&env) {
            return Ok(Some(&env)); // 직접 반환 — resolve_name 재호출 제거
        }
        anyhow::bail!(
            "site '{env}' (from OXIPAGE_SITE env) not found — use `oxipage site add` to create it"
        );
    }
}
```

- [ ] **Step 2: resolve_endpoint 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SitesFile;

    fn empty_sites() -> SitesFile { SitesFile::default() }

    #[test]
    fn test_endpoint_cli_flag_wins() {
        let sites = empty_sites();
        let result = resolve_endpoint(
            Some("https://custom.example.com".into()),
            None,
            &sites,
            None,
        ).unwrap();
        assert_eq!(result, "https://custom.example.com");
    }

    #[test]
    fn test_endpoint_site_over_config() {
        let mut sites = SitesFile::default();
        sites.sites.insert("prod".into(), crate::sites::SiteEntry {
            endpoint: "https://prod.example.com".into(),
            token: None,
        });
        let result = resolve_endpoint(
            None,
            Some("prod"),
            &sites,
            None, // no config file
        ).unwrap();
        assert_eq!(result, "https://prod.example.com");
    }

    #[test]
    fn test_endpoint_oxipage_env() {
        let sites = empty_sites();
        // OXIPAGE_ENDPOINT env → fallback
        std::env::set_var("OXIPAGE_ENDPOINT", "https://env.example.com");
        let result = resolve_endpoint(None, None, &sites, None).unwrap();
        assert_eq!(result, "https://env.example.com");
        std::env::remove_var("OXIPAGE_ENDPOINT");
    }

    #[test]
    fn test_endpoint_fallback_to_127() {
        let sites = empty_sites();
        // No flag, no site, no env, no config → default
        let result = resolve_endpoint(None, None, &sites, None).unwrap();
        assert_eq!(result, "http://127.0.0.1:8787");
    }

    #[test]
    fn test_endpoint_cli_flag_overrides_site() {
        let mut sites = SitesFile::default();
        sites.sites.insert("prod".into(), crate::sites::SiteEntry {
            endpoint: "https://prod.example.com".into(),
            token: None,
        });
        let result = resolve_endpoint(
            Some("https://override.example.com".into()),
            Some("prod"), // site says prod, but --endpoint wins
            &sites,
            None,
        ).unwrap();
        assert_eq!(result, "https://override.example.com");
    }
}
```

- [ ] **Step 3: resolve_token + token fallthrough 테스트**

```rust
#[test]
fn test_token_cli_flag_wins() {
    let sites = empty_sites();
    let result = resolve_token(Some("tok_cli".into()), None, &sites).unwrap();
    assert_eq!(result, Some("tok_cli".into()));
}

#[test]
fn test_token_site_token_used() {
    let mut sites = SitesFile::default();
    sites.sites.insert("prod".into(), crate::sites::SiteEntry {
        endpoint: "https://prod.example.com".into(),
        token: Some("tok_site".into()),
    });
    let result = resolve_token(None, Some("prod"), &sites).unwrap();
    assert_eq!(result, Some("tok_site".into()));
}

#[test]
fn test_token_site_no_token_falls_through_to_env() {
    let mut sites = SitesFile::default();
    sites.sites.insert("flyio".into(), crate::sites::SiteEntry {
        endpoint: "https://oxipage.fly.dev".into(),
        token: None, // no token → fallthrough
    });
    std::env::set_var("OXIPAGE_TOKEN", "tok_env");
    let result = resolve_token(None, Some("flyio"), &sites).unwrap();
    assert_eq!(result, Some("tok_env".into()));
    std::env::remove_var("OXIPAGE_TOKEN");
}

#[test]
fn test_token_none_when_nothing_configured() {
    let sites = empty_sites();
    // Clear any env leakage
    std::env::remove_var("OXIPAGE_TOKEN");
    let result = resolve_token(None, None, &sites).unwrap();
    assert_eq!(result, None);
}
```

- [ ] **Step 4: resolve_site_name 테스트**

```rust
#[test]
fn test_site_name_flag_valid() {
    let mut sites = SitesFile::default();
    sites.sites.insert("prod".into(), crate::sites::SiteEntry {
        endpoint: "url".into(), token: None,
    });
    let result = resolve_site_name(Some("prod"), &sites).unwrap();
    assert_eq!(result, Some("prod"));
}

#[test]
fn test_site_name_flag_unknown_errors() {
    let sites = empty_sites();
    let result = resolve_site_name(Some("nonexistent"), &sites);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}
```

- [ ] **Step 5: `cargo test -p oxipage-cli` → 23+ 통과 확인 (기존 10 + 신규 15+)**

- [ ] **Step 6: Commit**

```bash
git add crates/oxipage-cli/src/commands.rs
git commit -m "fix(cli): OXIPAGE_SITE unknown → error; add resolve chain unit tests (15+ cases)"
```

---

### Task 3: commands.rs → commands/ 분할

**Files:**
- Create: `crates/oxipage-cli/src/commands/mod.rs`
- Create: `crates/oxipage-cli/src/commands/site.rs`
- Create: `crates/oxipage-cli/src/commands/blog.rs`
- Create: `crates/oxipage-cli/src/commands/project.rs`
- Create: `crates/oxipage-cli/src/commands/link.rs`
- Create: `crates/oxipage-cli/src/commands/lobby.rs`
- Create: `crates/oxipage-cli/src/commands/extension.rs`
- Create: `crates/oxipage-cli/src/commands/backup.rs`
- Create: `crates/oxipage-cli/src/commands/auth.rs`
- Create: `crates/oxipage-cli/src/commands/init_status_serve.rs`
- Modify: `crates/oxipage-cli/src/main.rs` — 모듈 경로 변경
- Delete: `crates/oxipage-cli/src/commands.rs` (분할 완료 후)

**Interfaces:**
- `commands/mod.rs`가 `PubCommand` enum, `dispatch()`, `resolve_*` 함수, `require_token()`을 소유
- 각 서브모듈은 `PubCommand` enum variant 하나와 해당 핸들러 함수를 노출
- 기존 `main.rs`의 `Command` enum import는 `commands::<Type>` 그대로 유지

- [ ] **Step 1: commands.rs를 commands/ 디렉토리로 rename**

```bash
mkdir -p crates/oxipage-cli/src/commands
git mv crates/oxipage-cli/src/commands.rs crates/oxipage-cli/src/commands/mod.rs
```

- [ ] **Step 2: site.rs 추출**

`commands/mod.rs` 에서 `SiteCommand` enum, `dispatch_site()`, `site_list`, `site_show`, `site_use`, `site_add`, `site_edit`, `site_rm` 함수를 `commands/site.rs` 로 이동.

`commands/mod.rs`에 추가: `mod site; pub use site::SiteCommand;`

- [ ] **Step 3: blog.rs 추출**

`BlogCommand` enum + `blog()` 함수 → `commands/blog.rs`.

- [ ] **Step 4: project.rs 추출**

`ProjectCommand` enum + `project()` 함수 + `parse_link_pairs()` 헬퍼 → `commands/project.rs`.

- [ ] **Step 5: link.rs 추출**

`LinkCommand` enum + `link()` 함수 → `commands/link.rs`.

- [ ] **Step 6: lobby.rs 추출**

`LobbyCommand` enum + `lobby()` 함수 → `commands/lobby.rs`.

- [ ] **Step 7: extension.rs 추출**

`ExtensionCommand` enum + `extension()` 함수 → `commands/extension.rs`.

- [ ] **Step 8: backup.rs 추출**

`BackupCommand` enum + `backup()` 함수 → `commands/backup.rs`.

- [ ] **Step 9: auth.rs 추출**

`AuthCommand` + `TokenCommand` enums + `auth()` 함수 → `commands/auth.rs`.

- [ ] **Step 10: init_status_serve.rs 추출**

`init()`, `status()`, `serve()` 함수 + `DEFAULT_TOML` 상수 → `commands/init_status_serve.rs`.

- [ ] **Step 11: mod.rs 정리**

`mod.rs` 에 잔류하는 것: `Output`, `Client`, `sites`, `Cli`, `Command` imports, `dispatch()`, `resolve_*`, `require_token()`.

- [ ] **Step 12: `cargo build -p oxipage-cli` → clean compile 확인**

- [ ] **Step 13: `cargo test -p oxipage-cli` → 기존 23개 모두 통과 확인**

- [ ] **Step 14: Commit**

```bash
git add -A crates/oxipage-cli/src/commands/
git add crates/oxipage-cli/src/main.rs
git commit -m "refactor(cli): split commands.rs into commands/ modules (site, blog, project, link, lobby, extension, backup, auth, init_status_serve)"
```

---

### Task 4: Client::request 통합 테스트

**Files:**
- Create: `crates/oxipage-cli/tests/client_tests.rs`

**Interfaces:**
- Consumes: `Client` (Task 1 시그니처)
- Produces: HTTP-level integration tests

테스트는 실제 HTTP 서버를 띄우지 않고 `Client::new`의 validation + `ApiError` 구조체 round-trip을 검증한다. (full HTTP mock은 scope 밖 — oxipage-core의 test server 기반 통합 테스트로 전환 고려)

- [ ] **Step 1: Client 생성 basic test**

```rust
// crates/oxipage-cli/tests/client_tests.rs
use oxipage_cli::client::Client;

#[test]
fn test_client_endpoint_trimmed() {
    let client = Client::new(
        "http://localhost:8787/".into(),
        Some("tok".into()),
        false,
    ).unwrap();
    // endpoint trailing slash trimmed
    assert_eq!(client.endpoint(), "http://localhost:8787");
}

#[test]
fn test_client_has_token() {
    let client = Client::new(
        "http://localhost:8787".into(),
        Some("tok".into()),
        false,
    ).unwrap();
    assert!(client.has_token());
}

#[test]
fn test_client_no_token() {
    let client = Client::new(
        "http://localhost:8787".into(),
        None,
        false,
    ).unwrap();
    assert!(!client.has_token());
}
```

- [ ] **Step 2: ApiError Display test**

```rust
#[test]
fn test_api_error_display() {
    let err = oxipage_cli::client::ApiError {
        status: 404,
        code: "not_found".into(),
        message: "post not found".into(),
        field: Some("slug".into()),
    };
    let s = err.to_string();
    assert!(s.contains("404"));
    assert!(s.contains("not_found"));
    assert!(s.contains("post not found"));
    assert!(s.contains("field=slug"));
}
```

- [ ] **Step 3: `cargo test -p oxipage-cli` → 27+ 통과 확인 (23 + 4 신규)**

- [ ] **Step 4: Commit**

```bash
git add crates/oxipage-cli/tests/
git commit -m "test(cli): add Client construction and ApiError tests"
```

---

### Task 5: export public symbols for testing

**Files:**
- Modify: `crates/oxipage-cli/src/main.rs` — `pub use` 노출
- Modify: `crates/oxipage-cli/src/client.rs` — `ApiError` public 확인

**Interfaces:**
- Produces: `oxipage_cli::client::Client`, `oxipage_cli::client::ApiError` 외부 접근 가능

- [ ] **Step 1: main.rs에 re-export 추가**

```rust
// main.rs
pub use client;
```

`Client` 와 `ApiError`는 이미 `pub` 이므로 `pub use` 로 통합 테스트에서 접근 가능.

- [ ] **Step 2: cargo test → tests/client_tests.rs 컴파일 확인**

- [ ] **Step 3: Commit**

```bash
git add crates/oxipage-cli/src/main.rs
git commit -m "refactor(cli): expose client module for integration tests"
```

---

### Task 6: 전체 E2E smoke test

**Files:**
- Create: `crates/oxipage-cli/tests/smoke.rs`

**Interfaces:**
- Consumes: 빌드된 `oxipage` 바이너리
- Produces: E2E 동작 확인

실행할 `oxipage` 바이너리를 찾기 위해 `cargo build -p oxipage-cli` 의 출력물 경로를 사용한다. 실제 HTTP 서버를 띄우지 않고 `--help`, `--version`, `site` 서브커맨드만 검증.

- [ ] **Step 1: CLI help/version smoke test 작성**

```rust
// crates/oxipage-cli/tests/smoke.rs
use std::process::Command;

fn oxipage() -> Command {
    Command::new(env!("CARGO_BIN_EXE_oxipage"))
}

#[test]
fn test_help() {
    let out = oxipage().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--site"));
    assert!(stdout.contains("--insecure"));
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("site"));
    assert!(stdout.contains("blog"));
}

#[test]
fn test_site_list_empty() {
    let out = oxipage().arg("site").arg("list").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("no sites configured") || stdout.is_empty());
}

#[test]
fn test_site_add_list_rm_flow() {
    // Clean up any residual test data
    let config_dir = dirs::config_dir().unwrap().join("oxipage");
    let sites_file = config_dir.join("sites.toml");
    let _ = std::fs::remove_file(&sites_file);

    // Add
    let out = oxipage()
        .args(["site", "add", "test-site", "--endpoint", "http://localhost:9999"])
        .output().unwrap();
    assert!(out.status.success());

    // List
    let out = oxipage().arg("site").arg("list").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("test-site"));

    // Rm
    let out = oxipage()
        .args(["site", "rm", "test-site"])
        .output().unwrap();
    assert!(out.status.success());
}
```

- [ ] **Step 2: `cargo test -p oxipage-cli` → E2E 포함 전체 통과 확인**

- [ ] **Step 3: Commit**

```bash
git add crates/oxipage-cli/tests/smoke.rs
git commit -m "test(cli): add E2E smoke tests (help, site add/list/rm flow)"
```

---

### Task 7: 최종 검증 + 사이트 테스트 추가 (OXIPAGE_SITE env E2E)

**Files:**
- Modify: `crates/oxipage-cli/tests/smoke.rs` — env 테스트 추가

- [ ] **Step 1: OXIPAGE_SITE env unknown → error E2E 검증**

```rust
#[test]
fn test_site_flag_unknown_errors() {
    let out = oxipage()
        .args(["--site", "nonexistent-xyz", "site", "list"])
        .output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not found"));
    assert!(stderr.contains("nonexistent-xyz"));
}
```

- [ ] **Step 2: OXIPAGE_SITE env unknown → error 검증**

```rust
#[test]
fn test_oxipage_site_env_unknown_errors() {
    let out = oxipage()
        .env("OXIPAGE_SITE", "nonexistent-env")
        .arg("site").arg("list")
        .output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXIPAGE_SITE"));
    assert!(stderr.contains("not found"));
}
```

- [ ] **Step 3: `cargo test -p oxipage-cli` → 전체 30+ 통과 확인**

- [ ] **Step 4: `cargo build -p oxipage-cli` → clean, no warnings 확인**

- [ ] **Step 5: Commit**

```bash
git add crates/oxipage-cli/tests/smoke.rs
git commit -m "test(cli): add OXIPAGE_SITE env error E2E test"
```

---

## 10.6 의존성 그래프

```
Task 1 (Client 하드닝)
  └─ Task 2 (resolve_* 테스트)
       ├─ Task 3 (commands/ 분할)
       ├─ Task 4 (Client 통합 테스트)
       ├─ Task 5 (pub export)
       └─ Task 6 (E2E smoke)
            └─ Task 7 (최종 검증)
```

Task 4,5,6은 Task 2 이후 병렬 실행 가능하지만, commands/ 분할(Task 3)은 파일 대규모 이동이므로 충돌을 피하기 위해 **순차 실행을 권장**한다.

## 10.7 결과물

| metric | before | after |
|--------|--------|-------|
| `commands.rs` LOC | 1009 | 0 (mod.rs ~120, 서브모듈 ~120avg) |
| `Client::new` 호출 | 8회/명령 | 1회/명령 |
| reqwest timeout | 없음 | connect 10s, body 60s |
| `--insecure` | 없음 | `OXIPAGE_TLS_INSECURE` / `--insecure` |
| `OXIPAGE_SITE` unknown | silent fallback | error exit 1 |
| 테스트 건수 | 10 | 30+ |
| 테스트 커버 모듈 | sites.rs only | resolve chain, Client, E2E |
| `#[allow(dead_code)]` | 2건 (client::endpoint, client::post) | post 제거 가능성 검토 |

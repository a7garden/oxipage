# 11장 — CLI 확장성 (Extension 기반 CLI 명령)

## 11.1 문제

현재 CLI는 모든 서브커맨드가 `oxipage-cli` 바이너리에 **하드코딩**되어 있다.

```rust
// main.rs — 확장이 추가돼도 CLI 명령은 따로 추가해야 함
pub enum Command {
    Blog(commands::BlogCommand),     // 수동 추가
    Project(commands::ProjectCommand), // 수동 추가
    Auth(commands::AuthCommand),     // 수동 추가
    // novels? 없음. CLI 바이너리를 직접 수정해야 추가 가능.
    // movies? 없음.
    // books? 없음.
    // scraps? 없음.
    // activity? 없음.
}
```

반면 서버 측 `Extension` trait은 **확장이 자신의 기능을 등록**하는 표준화된 인터페이스를 제공한다:

| 기능 | Extension trait 메서드 | 상태 |
|------|----------------------|------|
| HTTP 라우트 | `fn routes() -> Router<AppState>` | 각 확장이 자유롭게 등록 |
| DB 스키마 | `fn migrations() -> Vec<Migration>` | 각 확장이 자유롭게 등록 |
| 백그라운드 잡 | `fn background_jobs() -> Vec<ScheduledJob>` | 각 확장이 자유롭게 등록 |
| 로비 카드 | `fn lobby_summary() -> Option<LobbyCard>` | 각 확장이 자유롭게 등록 |
| **CLI 명령** | **없음** | **oxipage-cli 바이너리에서 하드코딩, 확장이 등록 불가** |

novels/movies/books/scraps/activity 확장의 서버 API(`routes.rs` + `repo.rs` + SQL migrations)는 모두 완성되어 있지만, **CLI 명령이 없는 이유는 시스템적 설계 결함이 아닌 사람이 하드코딩을 추가하지 않았기 때문**이다. 이는 확장 시스템의 일관성을 깨는 구조적 문제다.

## 11.2 설계

### 11.2.1 Extension trait에 `cli_commands()` 추가

기존 등록 패턴(`routes()`, `migrations()`)과 동일한 인터페이스:

```rust
// oxipage-core/src/extension.rs

/// CLI 서브커맨드 하나의 정의.
pub struct CliCommand {
    /// 명령 이름 (예: "novel"). 확장 id와 동일할 필요는 없지만 관례상 일치 권장.
    pub name: &'static str,
    /// `oxipage novel --help` 상단에 표시될 설명.
    pub about: &'static str,
    /// 이 명령의 하위 서브커맨드들.
    pub subcommands: Vec<CliSubcommand>,
}

/// 단일 서브커맨드 (예: "oxipage novel new").
pub struct CliSubcommand {
    pub name: &'static str,
    pub about: &'static str,
    /// 위치 인자.
    pub args: Vec<CliArg>,
    /// 핸들러 — 모든 인자를 `&str → &str` 맵으로 받아 HTTP 호출 수행.
    pub handler: CliHandler,
}

pub struct CliArg {
    /// "--slug" 또는 "--title-ko" 등.
    pub long: &'static str,
    pub short: Option<char>,
    pub help: &'static str,
    pub required: bool,
}

/// 핸들러 타입: (인자 맵, HTTP 클라이언트) → Result.
pub type CliHandler =
    Arc<dyn Fn(&BTreeMap<&'static str, String>, &Client) -> BoxFuture<'_, anyhow::Result<()>>
        + Send + Sync>;
```

`Extension` trait에 기본 구현(빈 vec)으로 추가:

```rust
pub trait Extension: Send + Sync {
    // ...기존 메서드들...

    /// 이 확장이 CLI에 등록할 서브커맨드.
    /// 기본 구현: 빈 vec (CLI 명령이 없는 확장).
    fn cli_commands(&self) -> Vec<CliCommand> {
        Vec::new()
    }
}
```

### 11.2.2 CLI 측: catch-all `Dynamic` variant + raw args

clap derive와 builder를 같은 트리에서 섞으려면 모든 정적 명령까지 builder로 이전해야 하는 함정이 있다. 대신 **하나의 catch-all variant**로 동적 명령을 받는다:

```rust
// main.rs — 정적 명령은 derive 그대로 유지, 동적 명령은 raw args로 수신
pub enum Command {
    // 기존 정적 명령 (변경 없음)
    Init,
    Status,
    Serve { port: Option<u16> },
    Auth(commands::AuthCommand),
    Site(commands::SiteCommand),
    Blog(commands::BlogCommand),
    // ...나머지 정적 명령...

    /// 확장이 등록한 동적 명령. `oxipage novel new --title "X" --genre "g"` →
    /// `Dynamic { name: "novel", sub: "new", args: ["--title","X","--genre","g"] }`.
    /// clap derive가 매칭하지 못한 명령을 raw args 배열로 잡는다.
    #[clap(external_subcommand)]
    Dynamic(Vec<String>),
}
```

`#[clap(external_subcommand)]`는 derive가 알지 못하는 서브커맨드를 `Vec<String>`으로 포착하는 clap 기능이다. `oxipage novel new --title "X"` → `Dynamic(["novel", "new", "--title", "X"])`.

dispatch에서:

```rust
pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let client = Client::new(/* ... */)?;

    match cli.command {
        Command::Init => init(...),
        // ...기존 정적 명령...

        Command::Dynamic(ref args) => {
            // args = ["novel", "new", "--title", "X", "--genre", "g"]
            if args.is_empty() {
                anyhow::bail!("missing dynamic command name");
            }
            let ext_name = &args[0];
            let sub_name = args.get(1).ok_or_else(|| {
                anyhow::anyhow!("missing subcommand for extension '{ext_name}'")
            })?;
            // 나머지 args[2..]는 --key value 쌍으로 파싱
            let parsed = parse_dynamic_args(&args[2..]);

            // resolve extension: compile-time list + runtime discovery fallback
            let registry = resolve_command_registry().await?;
            let cmd = registry.lookup(ext_name, sub_name)?;
            (cmd.handler)(&parsed, &client).await
        }
    }
}
```

### 11.2.3 명령 레지스트리: 컴파일 타임 + 런타임 디스커버리

컴파일 확장은 빌드 타임에 `all_extensions()`로 알 수 있지만, WASM 확장은 서버 런타임에서만 존재한다. **한쪽만 택하지 않고 두 경로를 병합한다:**

```
resolve_command_registry():
  1. 컴파일 확장: oxipage_server::all_extensions().iter().flat_map(cli_commands)
     → local cache, online/offline 둘 다 작동
  2. 런타임 디스커버리: GET /api/v1/cli/commands
     → 서버가 WASM 확장을 포함한 전체 CLI 명령 정의 반환
  3. merge: 컴파일 목록 + 디스커버리 목록 (중복 제거, 뒤가 우선)
```

````rust
/// 서버가 /api/v1/cli/commands 에서 반환하는 형식.
/// 각 Extension의 routes()에서 이 엔드포인트를 자동으로 내보낸다.
#[derive(Serialize, Deserialize)]
pub struct CliCommandManifest {
    pub extensions: Vec<CliCommandSpec>,
}

#[derive(Serialize, Deserialize)]
pub struct CliCommandSpec {
    pub extension_id: String,
    pub name: String,
    pub about: String,
    pub subcommands: Vec<CliSubcommandSpec>,
}

#[derive(Serialize, Deserialize)]
pub struct CliSubcommandSpec {
    pub name: String,
    pub about: String,
    pub args: Vec<CliArgSpec>,
}

#[derive(Serialize, Deserialize)]
pub struct CliArgSpec {
    pub long: String,
    pub short: Option<char>,
    pub help: String,
    pub required: bool,
}
````

서버 측: core `build_app()`가 `/api/v1/cli/commands` 엔드포인트를 자동 마운트. 모든 활성 확장의 `cli_commands()`를 수집해 `CliCommandManifest`로 응답.

```rust
// oxipage-core/src/http.rs — build_app() 내부
Router::new()
    .route("/api/v1/cli/commands", get(cli_commands_handler))
    // ...기존 라우트...

async fn cli_commands_handler(State(state): State<Arc<AppState>>) -> Json<CliCommandManifest> {
    let extensions: Vec<_> = state.registry.iter()
        .filter(|e| e.runtime.enabled)
        .flat_map(|e| e.extension.cli_commands())
        .map(|cmd| CliCommandSpec { /* ... */ })
        .collect();
    Json(CliCommandManifest { extensions })
}
```

CLI 측 해상(resolution) 순서:

```
1. --endpoint flag / OXIPAGE_ENDPOINT / site / default → endpoint
2. 컴파일 확장 목록 (all_extensions) → offline 가능
3. GET {endpoint}/api/v1/cli/commands → WASM 확장 포함
   실패 시 컴파일 목록만으로 진행 (server offline tolerant)
4. merge 후 lookup
```

### 11.2.4 clap `external_subcommand`가 제공하는 UX

`#[clap(external_subcommand)]`를 사용한 동적 명령의 장점:

| 항목 | 효과 |
|------|------|
| `--help` | 정적 명령만 나열. "Run `oxipage help <name>` for more..." 유도 |
| `oxipage help novel` | Dynamic args로 진입 → resolve 후 인자 안내 |
| `oxipage novel` | args = `["novel"]` → "missing subcommand" 에러 |
| `oxipage novel new --title "X" --genre g` | 정상 파싱, handler 호출 |

derive와 builder를 섞지 않음. 정적 명령은 derive 유지, 동적 명령은 `Vec<String>`으로 원시 수신 후 수동 파싱. clap 내부와 충돌 없음.

## 11.3 장점

| 관점 | 효과 |
|------|------|
| **확장 시스템 일관성** | `routes()`, `migrations()`과 동일한 등록 패턴. "설치하면 CLI 명령도 자동" |
| **기존 확장과의 역호환성** | 기본 구현이 빈 vec이므로 기존 확장에 영향 없음 |
| **정적 명령 공존** | `#[clap(external_subcommand)]` 하나만 추가. blog/project/link 등 derive 기반 명령은 그대로 |
| **WASM 확장 CLI 명령** | 서버 `/api/v1/cli/commands` 엔드포인트로 런타임 확장의 명령도 자동 노출 |
| **오프라인 대응** | 서버 미기동 시 컴파일 확장 목록으로 폴백. `oxipage --help`는 서버 없이도 동적 명령 나열 가능 |
| **테스트 용이성** | 각 확장의 CLI 핸들러를 확장 자체의 crate에서 단위 테스트 (`Client` mock 주입) |

## 11.4 단점 및 대응

| 문제 | 대응 |
|------|------|
| **인자 타입 안전성 손실** | 모든 인자가 `&str`. 핸들러에서 파싱 필요하지만 보통 2-5개 인자로 체감 부담 적음 |
| **WASM 확장의 핸들러** | WASM 확장은 CLI 프로세스에서 Rust 코드를 실행할 수 없음. `CliHandler`가 `None`인 명령은 서버에 `/api/v1/cli/exec/{ext_id}/{subcommand}`로 위임 (dispatch에서 분기) |
| **중첩 서브커맨드** | 1-depth로 시작. `novel chapter add` → chapter를 별도 `CliCommand`로 분리. 필요 시 `CliSubcommand`에 `sub_subcommands: Vec<CliSubcommand>` 추가 |
| **디스커버리 지연** | 첫 동적 명령 실행 시 `GET /api/v1/cli/commands` 1회. 이후 세션 내 캐시 |
| **BoxFuture** | async 핸들러를 `Arc<dyn Fn>` 안에 넣으려면 `BoxFuture` 필요. `async_trait`와 동일한 패턴 |

## 11.5 구현 계획

### Phase 1: 코어 인터페이스 + 서버 엔드포인트

**파일:** `crates/oxipage-core/src/extension.rs`, `crates/oxipage-core/src/http.rs`

- `CliCommand`, `CliSubcommand`, `CliArg`, `CliHandler` 타입 정의
- `Extension::cli_commands()` 메서드 추가 (기본 구현: 빈 vec)
- 서버 `build_app()`에 `GET /api/v1/cli/commands` 엔드포인트 추가 — 모든 활성 확장의 `cli_commands()` 수집
- 핸들러가 `None`인 명령 감지: WASM 확장은 `POST /api/v1/cli/exec/{ext_id}/{subcommand}` 서버 위임

### Phase 2: CLI 측 `external_subcommand` + 디스커버리

**파일:** `crates/oxipage-cli/src/main.rs`, `crates/oxipage-cli/src/commands/mod.rs`

- `Command::Dynamic(Vec<String>)` variant + `#[clap(external_subcommand)]` 추가
- `resolve_command_registry()`: 컴파일 목록 + `GET /api/v1/cli/commands` merge (오프라인 폴백)
- dispatch()에서 raw args 파싱 → registry lookup → handler (또는 server proxy)
- `parse_dynamic_args(&[String]) -> BTreeMap<String, String>` 헬퍼

### Phase 3: 확장 마이그레이션

**파일:** 각 `crates/oxipage-ext-*/src/lib.rs`

```yaml
novels:   novel new, novel list, novel chapter add
movies:   review movie add, series create
books:    review book add
scraps:   scrap add, scrap queue, scrap delete
activity: activity sync
```

각 확장이 `fn cli_commands()` 구현. 기존 blog/project/link 등 정적 명령은 Phase 5에서 이전 가능(선택).

### Phase 4: 핸들러 공통 헬퍼

**파일:** `crates/oxipage-core/src/cli.rs` (신규)

- 인자 역직렬화 헬퍼 (`args_from_map::<T: Deserialize>()`)
- HTTP 응답 포맷팅 헬퍼 (output.rs와 유사)
- 에러 메시지 포맷팅

### Phase 5 (선택): 기존 정적 명령 이전

blog, project, link, lobby, extension, backup 등 기존 derive 기반 명령을 점진적으로 `cli_commands()` 기반으로 이전. 최종적으로 `Command::Dynamic`만 남고 정적 variant는 제거.

## 11.6 레퍼런스

- doc/01 §1.4 Extension trait 설계 원칙
- doc/04 §4.1 CLI는 API의 레퍼런스 클라이언트
- doc/04 §4.3 명령 체계
- clap `external_subcommand`: <https://docs.rs/clap/latest/clap/_derive/index.html#external-subcommands>

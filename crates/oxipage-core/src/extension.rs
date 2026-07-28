use crate::scheduler::ScheduledJob;
use crate::state::AppState;
use async_trait::async_trait;
use axum::Router;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Ko => "ko",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "ko" => Some(Lang::Ko),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCardItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCard {
    pub id: String,
    pub items: Vec<LobbyCardItem>,
}

/// SSR 스냅샷을 생성할 공개 페이지 (doc/01 §1.6).
#[derive(Debug, Clone)]
pub struct PageSpec {
    /// 단일 페이지 경로 (예: "/blog/hello-rust").
    pub path: String,
    /// 확장 내 문서 식별자 (slug 등).
    pub doc_id: String,
}

/// CLI 핸들러 트레이트 — 인자 맵을 받아 HTTP 호출로 명령을 실행한다.
pub trait CliHandler: Send + Sync {
    /// 인자 맵 (--key value 쌍)을 받아 CLI 명령을 실행한다.
    fn run(&self, args: BTreeMap<String, String>, client: &crate::client::Client)
        -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;
}
/// CLI 서브커맨드 하나의 정의. 확장이 `cli_commands()`로 반환한다.
#[derive(Clone)]
pub struct CliCommand {
    /// 명령 이름 (예: "novels"). 확장 id와 동일할 필요는 없지만 관례상 일치 권장.
    pub name: &'static str,
    /// `oxipage novels --help` 상단에 표시될 설명.
    pub about: &'static str,
    /// 이 명령의 하위 서브커맨드들.
    pub subcommands: Vec<CliSubcommand>,
}

/// 단일 서브커맨드 (예: "oxipage novels new").
#[derive(Clone)]
pub struct CliSubcommand {
    pub name: &'static str,
    pub about: &'static str,
    /// 위치 인자.
    pub args: Vec<CliArg>,
    /// 핸들러. None인 경우 서버 위임 (`POST /api/console/cli/exec/{name}/{subcommand}`).
    pub handler: Option<Arc<dyn CliHandler>>,
}

#[derive(Debug, Clone)]
pub struct CliArg {
    /// "--slug" 또는 "--title-ko" 등.
    pub long: &'static str,
    pub short: Option<char>,
    pub help: &'static str,
    pub required: bool,
}

// ───────────────────────── 서버 매니페스트 (doc/11 §11.2.3) ─────────────────────────

/// 서버 `/api/console/cli/commands` 응답 형식.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliCommandManifest {
    pub extensions: Vec<CliCommandSpec>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliCommandSpec {
    pub extension_id: String,
    pub name: String,
    pub about: String,
    pub subcommands: Vec<CliSubcommandSpec>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliSubcommandSpec {
    pub name: String,
    pub about: String,
    pub args: Vec<CliArgSpec>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CliArgSpec {
    pub long: String,
    pub short: Option<char>,
    pub help: String,
    pub required: bool,
}

#[async_trait]
pub trait Extension: Send + Sync {
    /// 고유 식별자. oxipage.toml의 enabled 목록, API 경로 프리픽스, 로비 매니페스트 키로 재사용.
    fn id(&self) -> &str;

    fn display_name(&self, lang: Lang) -> String;

    /// 이 확장이 소유한 SQLite 마이그레이션 (독립 네임스페이스 테이블).
    fn migrations(&self) -> Vec<Migration>;

    /// `/api/console/{id}/**` 하위에 마운트될 라우터.
    fn routes(&self) -> Router<AppState>;

    /// 로비 카드에 표시할 요약 데이터 (최근 글 3개, 활동 스파크라인 등).
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard>;

    /// 서버 부팅 시 1회 호출 (싱글턴 시드 등).
    async fn on_startup(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }

    /// 확장 비활성화 시 호출. 기본 구현은 `search_documents`에서 해당 확장의
    /// 모든 행을 즉시 동기 삭제한다 (doc/02 §2.13). DB/미디어 정리가 필요하면 override.
    async fn on_disable(&self, ctx: &AppState) -> anyhow::Result<()> {
        crate::search::delete_extension(&ctx.db, self.id()).await?;
        Ok(())
    }

    /// 백그라운드 잡 (GitHub 폴링, 외부 캐시 갱신 등). 기본 없음.
    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        Vec::new()
    }

    /// SSR 스냅샷이 필요한 공개 경로들. 기본 없음.
    fn public_pages(&self) -> Vec<PageSpec> {
        Vec::new()
    }

    /// 이 확장이 소유한 데이터 테이블 이름 (purge 시 DROP 대상). FTS 색인과
    /// 미디어 디렉토리(`data/media/{id}/`)는 코어가 별도로 정리한다.
    fn table_names(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// 런타임 적재(WASM) 확장이 동적 라우트를 제공하면 Some.
    /// 컴파일 확장은 None — 정적 `Router`를 반환하므로 라우트가 빌드 타임에 확정된다.
    /// WASM 확장은 라우트를 넘길 수 없으므로, 이 메서드로 동적 디스패치를 제공한다.
    /// 코어 `build_app`은 네스팅 루프에서 `route_dispatcher()`가 Some인 확장을 건너뛰고,
    /// 폴백 핸들러가 요청 시점에 디스패치한다.
    fn route_dispatcher(&self) -> Option<&dyn RouteDispatcher> {
        None
    }

    /// 이 확장이 CLI에 등록할 서브커맨드. 기본 구현: 빈 vec (CLI 명령이 없는 확장).
    fn cli_commands(&self) -> Vec<CliCommand> {
        Vec::new()
    }

    /// setup 마법사가 이 확장의 활성화 후 사용자에게 보여줄 step.
    /// `None`이면 이 확장은 자기 step이 없다(대부분의 확장).
    ///
    /// 코어의 setup 마법사가 이 메서드를 호출해 동적으로 step을 조립한다 —
    /// 확장이 활성화돼 있을 때만 노출되며, 코어는 확장의 도메인 필드를 모른다.
    fn setup_wizard_step(&self) -> Option<SetupStep> {
        None
    }

    /// 이 확장이 사용할 외부 API 키 메타. setup_status 응답에 노출되어
    /// 마법사가 동적으로 키 입력란을 만든다. 실제 키 값은 `save_external_key`로 수신.
    /// 기본 구현: 빈 vec (외부 키가 없는 확장).
    fn external_api_keys(&self) -> Vec<ExternalApiKey> {
        Vec::new()
    }

    /// 외부 API 키 값을 저장. 코어가 `external_api_keys()`에서 모은 id로 디스패치한다.
    ///
    /// **기본 구현:** `external_api_keys()`를 순회해 `key_id`와 일치하는 키를 찾고
    /// `env_var`가 등록돼 있으면 `std::env::set_var`로 process env에 보존한다.
    /// `scope == ExtensionConfig`이면 `extension_state.config` JSON에도 기록한다.
    /// 자기 도메인별 추가 검증/저장이 필요하면 확장이 override할 수 있다.
    async fn save_external_key(
        &self,
        ctx: &AppState,
        key_id: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        for k in self.external_api_keys() {
            if k.id == key_id {
                // SAFETY: process env 오염이지만 setup wizard는 단일 사용자 환경에서만
                // 동작하며, 이 시그니처는 v1 SSG 모델에서 "한 사용자가 한 사이트를
                // 로컬에서 설정"하는 경로에 한정된다. 다른 env 변경 경로는 env_override
                // 함수로 분리할 것.
                unsafe {
                    std::env::set_var(k.env_var, value);
                }
                if matches!(k.scope, ExternalKeyScope::ExtensionConfig) {
                    persist_extension_config(ctx, self.id(), k.env_var, value).await?;
                }
                return Ok(());
            }
        }
        Ok(())
    }

    /// setup 완료 시점에 시드할 샘플 데이터 (예: 환영 글).
    /// 활성 확장에만 호출되며, 실패해도 setup 완료 진행(best-effort).
    async fn seed_sample_data(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}

// ───────────────────────── Setup 위저드 훅 타입 ─────────────────────────
//
// setup 마법사가 확장의 자기-도메인 데이터(프로필 필드, 환영 글, API 키 등)를
// 동적으로 조립/저장하기 위한 트레이트 경계. 코어는 이 타입들의 외형만 알고
// 실제 SQL/데이터는 각 확장이 자기 `SetupStep::save_handler`와
// `save_external_key`/`seed_sample_data` 안에서 다룬다.

/// setup wizard 한 step의 선언적 정의.
/// 코어가 step 라우팅 + 폼 디스패치를 담당하고, 이 구조체가 UI 필드와 저장 콜백을 표현.
/// 클라이언트로 보낼 때는 `save_handler`만 빼고 직렬화된다(`ExtensionStepInfo` 참고).
#[derive(Clone)]
pub struct SetupStep {
    /// step 식별자. URL `/api/console/setup/extension-step/{id}`의 `{id}`로 사용된다.
    /// 확장에서 유일해야 한다(코어는 첫 번째로 매칭되는 step만 사용).
    pub id: &'static str,
    pub title_ko: &'static str,
    pub title_en: &'static str,
    pub description_ko: &'static str,
    pub description_en: &'static str,
    pub fields: Vec<SetupField>,
    /// form JSON을 받아 자기 DB에 저장하는 핸들러.
    pub save_handler: Arc<dyn SetupSaveHandler>,
}

/// 클라이언트에 직렬화되는 step 정보 (save_handler 제외).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtensionStepInfo {
    pub id: String,
    pub title_ko: String,
    pub title_en: String,
    pub description_ko: String,
    pub description_en: String,
    pub fields: Vec<SetupField>,
}

impl ExtensionStepInfo {
    pub fn from_step(step: &SetupStep) -> Self {
        Self {
            id: step.id.to_string(),
            title_ko: step.title_ko.to_string(),
            title_en: step.title_en.to_string(),
            description_ko: step.description_ko.to_string(),
            description_en: step.description_en.to_string(),
            fields: step.fields.clone(),
        }
    }
}

/// form 한 필드.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetupField {
    pub name: &'static str,
    pub label_ko: &'static str,
    pub label_en: &'static str,
    pub kind: SetupFieldKind,
    pub required: bool,
    pub placeholder_ko: Option<&'static str>,
    pub placeholder_en: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SetupFieldKind {
    Text,
    Textarea,
    Url,
}

/// 코어가 form JSON을 받아 위임. 확장이 자기 DB에 쓴다.
/// form의 모든 값은 문자열이다 — `serde_json::Value::String`으로 들어온다.
#[async_trait]
pub trait SetupSaveHandler: Send + Sync {
    async fn save(
        &self,
        ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()>;
}

/// 외부 API 키 한 줄. setup_status 응답에 노출되어 마법사가 동적으로
/// 입력란을 만들고, save 시 확장이 자기 도메인 위치에 저장한다.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExternalApiKey {
    pub id: &'static str,
    pub label_ko: &'static str,
    pub label_en: &'static str,
    pub env_var: &'static str,
    pub required: bool,
    pub scope: ExternalKeyScope,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalKeyScope {
    /// process env에만 set (현재 IntegrationsConfig가 env를 직접 읽는 패턴).
    EnvOnly,
    /// env + `extension_state.config` JSON 둘 다.
    ExtensionConfig,
}

/// `extension_state.config` JSON에 한 키를 upsert. save_external_key 기본 impl이 사용.
/// 기존에 같은 키가 있으면 덮어쓰고, JSON이 깨져 있으면 빈 dict로 시작한다.
pub(crate) async fn persist_extension_config(
    ctx: &AppState,
    ext_id: &str,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT config FROM extension_state WHERE extension_id = ?1")
            .bind(ext_id)
            .fetch_optional(&ctx.db)
            .await?;
    let mut config: serde_json::Map<String, serde_json::Value> = match row {
        Some((Some(s),)) => serde_json::from_str(&s).unwrap_or_default(),
        _ => serde_json::Map::new(),
    };
    config.insert(key.to_string(), serde_json::Value::String(value.to_string()));
    let serialized = serde_json::to_string(&config)?;
    sqlx::query(
        "INSERT INTO extension_state (extension_id, enabled, purged, config)
         VALUES (?1, 0, 0, ?2)
         ON CONFLICT(extension_id) DO UPDATE SET config = ?2",
    )
    .bind(ext_id)
    .bind(&serialized)
    .execute(&ctx.db)
    .await?;
    Ok(())
}


// ───────────────────────── 동적 라우트 디스패치 (WASM) ─────────────────────────

/// WASM 확장의 단일 라우트 선언.
#[derive(Debug, Clone)]
pub struct RouteSpec {
    /// HTTP 메서드 ("GET", "POST", "PUT", "DELETE", "PATCH").
    pub method: String,
    /// 확장 내 상대 경로 ("/info", "/items/{id}").
    pub path: String,
}

/// WASM 확장의 라우트 응답.
#[derive(Debug)]
pub struct RouteResponse {
    pub status: u16,
    /// 응답 본문 (보통 JSON UTF-8).
    pub body: Vec<u8>,
}

/// 런타임 적재 확장이 HTTP 요청을 처리하는 인터페이스.
/// `Extension::route_dispatcher()`가 반환하면, 코어 폴백 핸들러가 이 trait으로
/// 요청을 위임한다. 컴파일 확장은 구현하지 않는다.
#[async_trait]
pub trait RouteDispatcher: Send + Sync {
    /// 라우트 매니페스트 (load 시점 추출).
    fn route_specs(&self) -> &[RouteSpec];

    /// 요청 디스패치. method/path/body 를 WASM 모듈에 전달하고 응답을 반환한다.
    /// path 는 확장 prefix 이후의 경로 (예: 확장 id 가 "wasm-demo" 이고
    /// 요청이 "/api/console/wasm-demo/info" 이면 path="/info").
    async fn dispatch(
        &self,
        method: &str,
        path: &str,
        body: Vec<u8>,
        ctx: &AppState,
    ) -> RouteResponse;
}

// ───────────────────────── WASM 로더 (core → wasm crate) ─────────────────────────

/// `.wasm` 파일에서 `Extension` 트레이트 객체를 생성하는 팩토리.
/// 코어(`oxipage-core`)는 wasmtime 에 의존하지 않으므로, 실제 로딩은 이 trait의
/// 구현체(`oxipage-wasm`)가 담당한다. 서버가 `--features wasm` 으로 빌드되었을 때
/// `AppState.wasm_loader` 에 주입되어 install 엔드포인트의 라이브 활성화에 쓰인다.
pub trait WasmLoader: Send + Sync {
    fn load(&self, path: &std::path::Path) -> anyhow::Result<Arc<dyn Extension>>;
}

/// 공통 데이터 봉투 helpers — 확장 routes에서 재사용.
#[derive(Debug, serde::Serialize)]
pub struct DataEnvelope<T: serde::Serialize> {
    pub data: T,
}

#[derive(Debug, serde::Serialize)]
pub struct ListEnvelope<T: serde::Serialize> {
    pub data: Vec<T>,
    pub meta: ListMeta,
}

#[derive(Debug, serde::Serialize, Default)]
pub struct ListMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

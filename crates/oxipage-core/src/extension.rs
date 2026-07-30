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
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &crate::client::Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>;
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
    /// Now returns `Router` (no shared state) — handlers use
    /// `Extension<SiteScopedDb>` instead of `State<AppState>`.
    fn routes(&self) -> Router;

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

    /// setup 마법사에서 이 확장이 소유할 서브-위자드 (0..N step).
    /// `None`이면 이 확장은 위자드에 등장하지 않는다 (대부분의 확장).
    ///
    /// 코어의 setup 마법사가 이 메서드를 호출해 동적으로 step을 조립한다 —
    /// 확장이 활성화돼 있을 때만 노출되며, 코어는 확장의 도메인 필드를 모른다.
    /// (Phase 1: 단수 setup_wizard_step → 복수 ExtensionWizard 전환.)
    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        None
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
// `seed_sample_data` 안에서 다룬다.

/// setup wizard 한 step의 선언적 정의.
/// 코어가 step 라우팅 + 폼 디스패치를 담당하고, 이 구조체가 UI 필드와 저장 콜백을 표현.
/// 클라이언트로 보낼 때는 `save_handler`만 빼고 직렬화된다(`ExtensionStepInfo` 참고).
#[derive(Clone)]
pub struct SetupStep {
    pub id: &'static str,
    pub title_ko: &'static str,
    pub title_en: &'static str,
    pub description_ko: &'static str,
    pub description_en: &'static str,
    pub fields: Vec<SetupField>,
    /// form JSON을 받아 자기 DB에 저장하는 핸들러.
    pub save_handler: Arc<dyn SetupSaveHandler>,
    /// 클라이언트에 공개되는 pre-fill 힌트. 예: `{"display_name": "site_name"}`
    /// — wizard가 사이트 컨텍스트에서 값을 가져와 채운다.
    /// 키는 field.name, 값은 PrefillSource. 직렬화는 `ExtensionStepInfo`를 통해.
    pub prefill: BTreeMap<&'static str, PrefillSource>,
    /// step 표시 조건. None = 항상 표시. 클라이언트가 평가 (Phase 3).
    pub visible_when: Option<VisibilityRule>,
}

/// prefill 값의 출처. 확장이 자기 의미에 맞는 출처를 선언한다.
#[derive(Debug, Clone, Copy)]
pub enum PrefillSource {
    /// 사이트 1단계에서 사용자가 입력한 사이트 이름.
    SiteName,
}

/// step의 표시 조건. None = 항상 표시. 클라이언트가 직렬화된 규칙을 평가한다 (Phase 3).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VisibilityRule {
    FieldNotEmpty {
        step_id: &'static str,
        field: &'static str,
    },
    FieldEquals {
        step_id: &'static str,
        field: &'static str,
        value: &'static str,
    },
    All(Vec<VisibilityRule>),
    Any(Vec<VisibilityRule>),
}

/// 한 확장이 소유한 서브-위자드 (0..N step). 빈 steps vec = 참여 안 함.
pub struct ExtensionWizard {
    pub steps: Vec<SetupStep>,
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
    /// 필드 pre-fill 매핑. wizard가 사이트 컨텍스트에서 값을 가져와 채운다.
    /// 예: `{"display_name": "site_name"}` → site_name을 display_name에 주입.
    /// 키는 field.name, 값은 출처 식별자.
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub prefill: BTreeMap<String, String>,
    /// step 표시 조건 (클라이언트 평가). None = 항상 표시.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<VisibilityRule>,
    /// fields 가 비어있으면 action step (버튼만). 클라이언트 렌더 분기용.
    pub is_action: bool,
}

impl ExtensionStepInfo {
    pub fn from_step(step: &SetupStep) -> Self {
        let prefill = step
            .prefill
            .iter()
            .map(|(field, source)| {
                let source = match source {
                    PrefillSource::SiteName => "site_name",
                };
                ((*field).to_string(), source.to_string())
            })
            .collect();
        Self {
            id: step.id.to_string(),
            title_ko: step.title_ko.to_string(),
            title_en: step.title_en.to_string(),
            description_ko: step.description_ko.to_string(),
            description_en: step.description_en.to_string(),
            fields: step.fields.clone(),
            prefill,
            visible_when: step.visible_when.clone(),
            is_action: step.fields.is_empty(),
        }
    }
}

/// form 한 필드.
///
/// **직렬화 형태:** `kind` 필드를 flatten해서 클라이언트가 `field.type`로 직접 읽는다.
/// 예: `{"name":"bio_ko","label_ko":"…","type":"textarea","required":false}`.
/// flatten이 없으면 `kind:{"type":"textarea"}`가 되어 클라이언트 `field.type`이 항상 undefined.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SetupField {
    pub name: &'static str,
    pub label_ko: &'static str,
    pub label_en: &'static str,
    #[serde(flatten)]
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
    Secret,
}

/// 코어가 form JSON을 받아 위임. 확장이 자기 DB에 쓴다.
/// form의 모든 값은 문자열이다 — `serde_json::Value::String`으로 들어온다.
#[async_trait]
pub trait SetupSaveHandler: Send + Sync {
    async fn save(
        &self,
        ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome>;
}

/// step 저장 결과. 후속 step 의 visible_when/prefill 평가에 쓰이는 값을 클라이언트에 노출.
/// 코어는 이 값을 그냥 전달만 한다 — 평가는 클라이언트가 한다.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StepOutcome {
    pub values: serde_json::Map<String, serde_json::Value>,
}

impl StepOutcome {
    /// 폼 step 기본 동작: 입력받은 form 값을 그대로 outcome 으로 노출.
    pub fn from_form(form: &serde_json::Map<String, serde_json::Value>) -> Self {
        StepOutcome {
            values: form.clone(),
        }
    }
}

/// `extension_state.config` JSON에 한 키를 upsert. 확장의 setup_wizard 키-step save 핸들러가 사용.
/// 기존에 같은 키가 있으면 덮어쓰고, JSON이 깨져 있으면 빈 dict로 시작한다.
pub async fn persist_extension_config(
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
    config.insert(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
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

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
    /// 핸들러. None인 경우 서버 위임 (`POST /api/v1/cli/exec/{name}/{subcommand}`).
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

/// 서버 `/api/v1/cli/commands` 응답 형식.
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

    /// `/api/v1/{id}/**` 하위에 마운트될 라우터.
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
    /// 요청이 "/api/v1/wasm-demo/info" 이면 path="/info").
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

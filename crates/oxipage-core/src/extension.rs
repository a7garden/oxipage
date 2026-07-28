use crate::scheduler::ScheduledJob;
use crate::state::AppState;
use async_trait::async_trait;
use axum::Router;
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

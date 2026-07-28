//! Admin sub-API (sites/themes) — 이전 oxipage-admin crate 통합 (v2).
//!
//! Sites 관리, 테마 카탈로그 등 admin SPA의 백엔드 API.
//! v2 SSG: 사이트 프록시 제거. CLI는 로컬 DB/파일을 직접 다룬다 (oxipage query/schema/build/deploy).

pub mod sites_api;
pub mod themes;

use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use directories::ProjectDirs;
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Admin 콘솔 전역 상태.
#[derive(Clone)]
pub struct AdminContext {
    /// 사이트 목록 (`~/.config/oxipage/sites.toml`)
    pub sites_path: PathBuf,
    /// 멀티사이트 활성 사이트
    pub active_site: Arc<RwLock<Option<String>>>,
}

/// Admin 에러 타입.
#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("internal error: {0}")]
    Internal(anyhow::Error),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AdminError::NotFound(_) => (axum::http::StatusCode::NOT_FOUND, "not_found"),
            AdminError::BadRequest(_) => (axum::http::StatusCode::BAD_REQUEST, "bad_request"),
            AdminError::Internal(_) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let msg = self.to_string();
        (
            status,
            Json(serde_json::json!({
                "error": { "code": code, "message": msg }
            })),
        )
            .into_response()
    }
}

/// 임베드된 Admin SPA 빌드 아티팩트.
#[derive(RustEmbed)]
#[folder = "embedded-spa"]
struct AdminAssets;

/// Admin 콘솔 서버를 127.0.0.1:{port}에 기동한다.
/// (v1 호환: `oxipage_console::run_admin` 별칭)
pub async fn run_admin(port: u16) -> anyhow::Result<()> {
    let sites_path = sites_path()?;
    let ctx = AdminContext {
        sites_path,
        active_site: Arc::new(RwLock::new(None)),
    };

    let app = build_admin_router(ctx);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "oxipage admin console starting");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_admin_router(ctx: AdminContext) -> Router {
    Router::new()
        // sites CRUD
        .route("/api/admin/sites", get(sites_api::list).post(sites_api::add))
        .route(
            "/api/admin/sites/active",
            get(sites_api::get_active).put(sites_api::set_active),
        )
        .route(
            "/api/admin/sites/{name}",
            get(sites_api::update).delete(sites_api::delete),
        )
        // themes
        .route("/api/admin/themes", get(themes::catalog_handler))
        // SPA static (vite build)
        .fallback(static_handler)
        .with_state(ctx)
}

/// SPA 정적 파일 서빙. `/assets/*`는 Vite 빌드 에셋, 그 외 경로는 `index.html`로 폴백.
async fn static_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(asset) = serve_asset(path) {
        return asset;
    }
    if let Some(index) = AdminAssets::get("index.html") {
        return Response::builder()
            .header("content-type", "text/html; charset=utf-8")
            .body(axum::body::Body::from(index.data.into_owned()))
            .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    }
    Response::new(axum::body::Body::empty())
}

fn serve_asset(path: &str) -> Option<Response> {
    let asset = AdminAssets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .header("content-type", mime.to_string())
        .body(axum::body::Body::from(asset.data.into_owned()))
        .ok()
}

/// ~/.config/oxipage/sites.toml 경로 결정.
fn sites_path() -> anyhow::Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "oxipage", "oxipage")
        .ok_or_else(|| anyhow::anyhow!("could not determine project directories"))?;
    let dir = dirs.config_dir();
    std::fs::create_dir_all(dir)?;
    Ok(dir.join("sites.toml"))
}

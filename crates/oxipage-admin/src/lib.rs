//! Oxipage Admin Console — 로컬 전용 관리 GUI 백엔드 (doc/12).
//!
//! `oxipage admin` CLI 서브커맨드가 기동하는 로컬 서버. 127.0.0.1:{port}에 바인딩되어
//! 관리 SPA를 서빙하고, 사이트 프로필(`~/.config/oxipage/sites.toml`) 관리 API + 사이트
//! 프록시를 제공한다.

mod proxy;
mod sites_api;
pub mod themes;

use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::path::PathBuf;

/// 임베드된 Admin SPA 빌드 아티팩트 (`admin-web/dist`).
#[derive(RustEmbed)]
#[folder = "../../admin-web/dist"]
struct AdminAssets;

/// Admin 콘솔 전역 상태.
#[derive(Clone)]
pub(crate) struct AdminContext {
    /// HTTP 클라이언트 (proxy용). 연결 재사용.
    client: reqwest::Client,
    /// `SitesFile`의 경로 (`~/.config/oxipage/sites.toml`).
    sites_path: PathBuf,
}

/// Admin 에러 타입.
#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("upstream error: {0}")]
    Upstream(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AdminError::NotFound(m) => (axum::http::StatusCode::NOT_FOUND, m.clone()),
            AdminError::BadRequest(m) => (axum::http::StatusCode::BAD_REQUEST, m.clone()),
            AdminError::Upstream(m) => (axum::http::StatusCode::BAD_GATEWAY, m.clone()),
            AdminError::Internal(e) => {
                tracing::error!("admin internal error: {e}");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_string(),
                )
            }
        };
        (status, Json(serde_json::json!({"error": msg}))).into_response()
    }
}

/// Admin 콘솔 서버를 127.0.0.1:{port}에 기동한다.
pub async fn run_admin(port: u16) -> anyhow::Result<()> {
    let sites_path = sites_path()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(false)
        .build()?;

    let ctx = AdminContext {
        client,
        sites_path,
    };

    let app = build_router(ctx);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot bind to 127.0.0.1:{port} — {e}. Try a different port with --port <N>"
        )
    })?;
    tracing::info!("oxipage admin console on http://127.0.0.1:{port}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn build_router(ctx: AdminContext) -> Router {
    // Admin API routes — same origin as the SPA
    let api = Router::new()
        .route("/sites", get(sites_api::list).post(sites_api::add))
        .route(
            "/sites/{name}",
            axum::routing::put(sites_api::update).delete(sites_api::delete),
        )
        .route("/sites/active", get(sites_api::get_active).put(sites_api::set_active))
        .route("/themes", get(themes::catalog_handler))
        .route("/proxy/{site}/{*path}", any(proxy::proxy_handler));

    Router::new()
        .nest("/api/admin", api)
        .fallback(static_handler)
        .with_state(ctx)
}

/// SPA 정적 파일 서빙. `/assets/*`는 Vite 빌드 에셋, 그 외 경로는 `index.html`로 폴백.
async fn static_handler(uri: axum::http::Uri) -> axum::response::Response {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() || path == "index.html" {
        return serve_asset("index.html");
    }
    if path.starts_with("assets/") {
        return serve_asset(path);
    }
    // SPA fallback — 모든 미등록 경로는 index.html
    serve_asset("index.html")
}

fn serve_asset(path: &str) -> axum::response::Response {
    match AdminAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            )
                .into_response()
        }
        None => {
            // 개발 중 admin-web/dist가 없을 때 — placeholder 응답
            (
                axum::http::StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/html; charset=utf-8",
                )],
                "<html><body><h1>Oxipage Admin</h1><p>Admin SPA not built yet. Run <code>cd admin-web && bun install && bun run build</code></p></body></html>".as_bytes(),
            )
                .into_response()
        }
    }
}

/// ~/.config/oxipage/sites.toml 경로 결정.
fn sites_path() -> anyhow::Result<PathBuf> {
    let dir = directories::ProjectDirs::from("", "", "oxipage")
        .ok_or_else(|| anyhow::anyhow!("cannot determine config directory"))?
        .config_dir()
        .to_path_buf();
    Ok(dir.join("sites.toml"))
}

async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

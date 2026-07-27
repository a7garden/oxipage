//! Oxipage 서버 진입점 라이브러리. CLI의 `serve` 서브커맨드가 동일한 진입점을
//! 호출한다 (doc/04 §4.1 — `serve`는 "CLI가 서버 프로세스를 기동하는 예외").
//!
//! 이 크레이트는 모든 확장을 정적 링크하며, 여기서 확장 레지스트리를 조립한다.
//! 새 확장 추가 시 `all_extensions()`에 한 줄 추가하고 Cargo.toml 의존성을 추가.

use oxipage_core::config::Config;
use oxipage_core::extension::Extension;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// 활성화된 모든 확장을 정적 링크에서 조립.
/// oxipage.toml의 `[extensions].enabled`가 비어 있으면 전부 활성화.
pub fn all_extensions() -> Vec<Arc<dyn Extension>> {
    vec![
        Arc::new(oxipage_ext_profile::ProfileExtension),
        Arc::new(oxipage_ext_blog::BlogExtension),
        Arc::new(oxipage_ext_projects::ProjectsExtension),
        Arc::new(oxipage_ext_links::LinksExtension),
        Arc::new(oxipage_ext_novels::NovelsExtension),
        Arc::new(oxipage_ext_movies::MoviesExtension),
        Arc::new(oxipage_ext_books::BooksExtension),
        Arc::new(oxipage_ext_scraps::ScrapsExtension),
        Arc::new(oxipage_ext_activity::ActivityExtension),
    ]
}

pub async fn run_server() -> anyhow::Result<()> {
    run_server_with_extensions(all_extensions()).await
}

/// 테스트/커스텀 빌드용 진입점 — 확장 목록을 주입받는다.
pub async fn run_server_with_extensions(all: Vec<Arc<dyn Extension>>) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("oxipage.toml"));
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        tracing::warn!(path = %config_path.display(), "config file not found; using defaults");
        let mut cfg = Config::default();
        cfg.apply_env_overrides();
        cfg
    };
    let config = Arc::new(config);

    let enabled: Vec<Arc<dyn Extension>> = if config.extensions.enabled.is_empty() {
        all
    } else {
        all.into_iter()
            .filter(|e| config.extensions.enabled.iter().any(|id| id == e.id()))
            .collect()
    };
    let registry = Arc::new(ExtensionRegistry::new(enabled));

    let db_path = config.server.data_dir.join("oxipage.db");
    let db = oxipage_core::db::connect(&db_path).await?;
    registry.run_migrations(&db).await?;

    let admin_token: Option<Arc<str>> = std::env::var("OXIPAGE_ADMIN_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(Arc::from);
    if admin_token.is_none() {
        tracing::warn!(
            "OXIPAGE_ADMIN_TOKEN is not set; write APIs will return 503 admin_not_configured"
        );
    }

    let state = AppState {
        db,
        config: config.clone(),
        admin_token: admin_token.clone(),
        registry: registry.clone(),
    };
    for ext in registry.iter() {
        ext.on_startup(&state).await?;
    }

    let app = oxipage_core::http::build_app(state);
    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("oxipage listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}

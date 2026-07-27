use oxipage_core::config::Config;
use oxipage_core::extension::Extension;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use oxipage_ext_profile::ProfileExtension;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
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

    // Phase 0: 컴파일 타임 확장 목록. enabled 비어있으면 전부 활성.
    let all: Vec<Arc<dyn Extension>> = vec![Arc::new(ProfileExtension)];
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

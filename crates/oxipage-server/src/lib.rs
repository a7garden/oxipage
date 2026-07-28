//! Oxipage 서버 진입점 라이브러리. CLI의 `serve` 서브커맨드가 동일한 진입점을
//! 호출한다 (doc/04 §4.1 — `serve`는 "CLI가 서버 프로세스를 기동하는 예외").
//!
//! 이 크레이트는 모든 확장을 정적 링크하며, 여기서 확장 레지스트리를 조립한다.
//! 런타임 탑재/제거는 DB `extension_state` 기반 (doc/02 §2.13). 새 확장 추가 시
//! `all_extensions()`에 한 줄 추가하고 Cargo.toml 의존성을 추가.

use oxipage_core::builder::BuildExt;
use oxipage_core::config::Config;
use oxipage_core::extension::Extension;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

/// 컴파일된 모든 확장을 정적 링크에서 조립. registry는 항상 전부를 들고 가며,
/// 런타임 활성/비활성은 DB `extension_state`가 결정한다 (doc/02 §2.13).
/// `oxipage.toml`의 `[extensions].enabled`는 첫 부팅 시드로만 쓰인다.
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

/// 컴파일된 모든 확장의 BuildExt 인스턴스.
pub fn all_builders() -> Vec<Box<dyn BuildExt>> {
    vec![
        Box::new(oxipage_ext_profile::ProfileExtension),
        Box::new(oxipage_ext_blog::BlogExtension),
        Box::new(oxipage_ext_projects::ProjectsExtension),
        Box::new(oxipage_ext_links::LinksExtension),
        Box::new(oxipage_ext_novels::NovelsExtension),
        Box::new(oxipage_ext_movies::MoviesExtension),
        Box::new(oxipage_ext_books::BooksExtension),
        Box::new(oxipage_ext_scraps::ScrapsExtension),
        Box::new(oxipage_ext_activity::ActivityExtension),
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

    // 런타임 WASM 확장 적재 (doc/08 §8.4, feature gate). data/extensions/*.wasm 을
    // 스캔해 정적 확장 목록에 추가한다. 라우트가 없으므로 lobby 카드만 기여한다.
    // 빈 디렉토리/누락이면 load_all_from_dir 이 빈 vec 를 반환한다.
    let all = {
        #[cfg(feature = "wasm")]
        {
            let mut all = all;
            all.extend(oxipage_wasm::load_all_from_dir(
                &config.server.data_dir.join("extensions"),
            ));
            all
        }
        #[cfg(not(feature = "wasm"))]
        { all }
    };

    // 단일 진실 소스 (doc/02 §2.13): 모든 컴파일 확장이 registry에 들어가 라우트까지 항상
    // 마운트된다. toml [extensions].enabled는 첫 부팅 시드로만 쓰이고 이후엔 DB가 결정.
    let registry = Arc::new(ExtensionRegistry::new(all));
    let toml_enabled = config.extensions.enabled.clone();

    let db_path = config.server.data_dir.join("oxipage.db");
    let db = oxipage_core::db::connect(&db_path).await?;
    registry.run_migrations(&db, &toml_enabled).await?;

    let admin_token: Option<Arc<str>> = std::env::var("OXIPAGE_ADMIN_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(Arc::from);
    if admin_token.is_none() {
        tracing::warn!(
            "OXIPAGE_ADMIN_TOKEN is not set; write APIs will return 503 admin_not_configured"
        );
    }

    // 첫 부팅 감지 — setup 마법사로 브라우저 오픈 (doc/13)
    if oxipage_core::setup::is_setup_needed(&db).await {
        let url = format!("http://{}:{}/setup", config.server.host, config.server.port);
        tracing::info!("first boot detected — opening setup wizard at {url}");
        open_browser(&url);
    }

    let wasm_loader: Option<Arc<dyn oxipage_core::extension::WasmLoader>> = {
        #[cfg(feature = "wasm")]
        {
            Some(Arc::new(oxipage_wasm::WasmLoaderImpl))
        }
        #[cfg(not(feature = "wasm"))]
        {
            None
        }
    };
    let state = AppState {
        db,
        config: config.clone(),
        admin_token: admin_token.clone(),
        registry: registry.clone(),
        wasm_loader,
        site_override: Arc::new(RwLock::new(None)),
        builders: Arc::new(all_builders()),
    };
    for ext in registry.iter() {
        let status = registry.status_of(ext.id()).await;
        let active = status.map(|s| s.active()).unwrap_or(false);
        if active {
            ext.on_startup(&state).await?;
        } else if status.map(|s| !s.purged).unwrap_or(false) {
            // doc/02 §2.13 안전망: toml 비활성화로 시드된 확장의 FTS 색인을 즉시 정리.
            if let Err(e) = ext.on_disable(&state).await {
                tracing::warn!(extension = ext.id(), error = %e, "on_disable failed at boot");
            }
        }
    }

    // 백그라운드 잡 스케줄러 (doc/01 §1.9). 활성 확장의 background_jobs()를
    // 수집해 cron 드라이버로 spawn한다. run(&self, &AppState) 시그니처로
    // job body가 DB pool/config에 접근한다.
    let mut scheduler = oxipage_core::scheduler::Scheduler::new();
    for ext in registry.iter() {
        if registry.is_active(ext.id()).await {
            for job in ext.background_jobs() {
                scheduler.register(job);
            }
        }
    }
    scheduler.spawn_all(state.clone());

    let app = oxipage_core::http::build_app(state);
    let addr = SocketAddr::new(config.server.host.parse()?, config.server.port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("oxipage listening on http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// 플랫폼별 기본 브라우저로 URL 오픈 (실패 시 경고만, 서버는 계속)
fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd).arg(url).spawn() {
        tracing::warn!("could not open browser: {e}");
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    // systemd/launchd는 SIGTERM을 보낸다 (doc/05 §5.2). ctrl_c(SIGINT)만 잡으면
    // `systemctl stop` 시 드레인 없이 즉시 종료되므로 SIGTERM도 함께 대기한다.
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                let _ = s.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

//! 비-API `/preview/*` 404 가드 + canonical preview 경로 불변 (design §6).

use axum::body::Body;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use oxibuilder_console::operations::SiteOperationGuard;
use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::config::Config;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::sites::SitesFile;
use oxibuilder_core::state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile", "blog"]
"#
    )
}

fn create_site_dir(name: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::with_prefix(format!("oxibuilder-followup-{name}-")).unwrap();
    let toml_path = dir.path().join("oxibuilder.toml");
    std::fs::write(&toml_path, minimal_toml(name)).unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}

async fn build_console_app() -> axum::Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry: Arc::new(ExtensionRegistry::new(vec![])),
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };

    let (_dir, path) = create_site_dir("Test");
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path);
    let guard = Arc::new(SiteOperationGuard::new());
    let site_registry = Arc::new(SiteRegistry::new(sf, guard).await.unwrap());
    oxibuilder_console::build_console_app(state, site_registry)
}

#[tokio::test]
async fn non_api_preview_paths_return_404_not_admin_html() {
    let app = build_console_app().await;
    for uri in [
        "/preview/nope/",
        "/preview/nope",
        "/preview/blog/whatever/x",
    ] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "uri={uri}");
        let bytes = to_bytes(resp.into_body(), 4096).await.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains("admin"),
            "uri={uri} must not serve admin.html: {text}"
        );
    }
}

#[tokio::test]
async fn api_preview_canonical_path_unaffected() {
    let app = build_console_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/console/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // 등록 사이트 + 무빌드 → 424 build_required (기존 동작 불변).
    assert_eq!(resp.status(), StatusCode::FAILED_DEPENDENCY);
}

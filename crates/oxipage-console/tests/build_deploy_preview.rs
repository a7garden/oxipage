//! Tests for the build/deploy/preview route handlers.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use oxipage_core::sites::SitesFile;
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
    let dir = TempDir::with_prefix(format!("oxipage-t9-{name}-")).unwrap();
    let toml_path = dir.path().join("oxipage.toml");
    std::fs::write(&toml_path, minimal_toml(name)).unwrap();
    let p = dir.path().to_path_buf();
    (dir, p)
}

async fn build_test_app() -> Router {
    let (_dir, path) = create_site_dir("Test");
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path);
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf, Default::default(), Default::default()).await.unwrap());
    build_console_router(registry)
}


#[tokio::test]
async fn preview_endpoint_returns_404_for_missing_out_dir() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/index.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

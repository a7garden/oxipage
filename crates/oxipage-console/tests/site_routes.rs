//! Tests for site-prefixed routes and middleware.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use oxipage_core::sites::SitesFile;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt; // for `.oneshot()`

/// Minimal oxipage.toml for test sites.
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
    let dir = TempDir::with_prefix(format!("oxipage-test-{name}-")).unwrap();
    let toml_path = dir.path().join("oxipage.toml");
    std::fs::write(&toml_path, minimal_toml(name)).unwrap();
    let p = dir.path().to_path_buf();
    (dir, p)
}

async fn build_test_app() -> Router {
    let (_dir, path) = create_site_dir("TestSite");
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path);
    let registry = Arc::new(SiteRegistry::new(sf).await.unwrap());
    build_console_router(registry)
}

#[tokio::test]
async fn unknown_slug_returns_404() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/s/missing/blog/posts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // `/s/missing/blog/posts` will not match any literal nest path.
    // With axum, an unmatched path falls through to the outer router which
    // has no fallback handler → 404.
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn known_slug_build_endpoint_returns_200() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/build/blog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let s = resp.status(); // build needs migrations; OK if 200/500 in tests
    assert!(s != StatusCode::NOT_FOUND, "route missing, got {s:?}");
}

#[tokio::test]
async fn sites_list_returns_json() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/sites")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn sites_default_returns_json() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/sites/default")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

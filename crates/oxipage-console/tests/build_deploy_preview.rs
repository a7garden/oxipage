//! Tests for the build/deploy/preview route handlers.

use axum::Router;
use axum::body::{Body, to_bytes};
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
    let registry = Arc::new(
        SiteRegistry::new(sf, Default::default(), Default::default())
            .await
            .unwrap(),
    );
    build_console_router(registry)
}

/// Site with a populated `out/` (manifest + index.html carrying the
/// manifest-derived `/repo/` base — the handler must override it).
async fn build_test_app_with_out() -> (TempDir, Router) {
    let (dir, path) = create_site_dir("Test");
    let out_dir = path.join("data").join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(
        out_dir.join(".oxipage-build.json"),
        r#"{"build_id":"b1","deployment_base":"/repo/","theme_id":"paper","asset_revision":"abc","built_at":"2026-07-31T10:00:00Z"}"#,
    )
    .unwrap();
    std::fs::write(
        out_dir.join("index.html"),
        "<!DOCTYPE html><html><head><base href=\"/repo/\"></head><body>x</body></html>",
    )
    .unwrap();

    let mut sf = SitesFile::default();
    sf.add("blog".into(), path);
    sf.set_default("blog");
    let registry = Arc::new(
        SiteRegistry::new(sf, Default::default(), Default::default())
            .await
            .unwrap(),
    );
    (dir, build_console_router(registry))
}

#[tokio::test]
async fn preview_endpoint_returns_424_when_manifest_missing() {
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
    assert_eq!(resp.status(), StatusCode::FAILED_DEPENDENCY);
}

#[tokio::test]
async fn preview_root_serves_index_and_rewrites_base() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("/api/console/preview/blog/"),
        "base not rewritten: {text}"
    );
    assert!(!text.contains("/repo/"), "old base leaked: {text}");
    assert_eq!(
        app.oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .headers()
        .get("cache-control")
        .unwrap(),
        "no-store"
    );
}

#[tokio::test]
async fn preview_rejects_traversal() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/../etc/passwd")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn preview_serves_spa_fallback_for_missing_route() {
    let (dir, app) = build_test_app_with_out().await;
    let out_dir = dir.path().join("data").join("out");
    std::fs::write(
        out_dir.join("404.html"),
        "<!DOCTYPE html><html><head><base href=\"/repo/\"></head><body>404</body></html>",
    )
    .unwrap();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/blog/some-post")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("/api/console/preview/blog/"),
        "base not rewritten: {text}"
    );
}

#[tokio::test]
async fn preview_redirects_no_wildcard_to_trailing_slash() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/api/console/preview/blog/"
    );
}

#[tokio::test]
async fn preview_emits_build_metadata_headers() {
    let (_dir, app) = build_test_app_with_out().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/preview/blog/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let h = resp.headers();
    assert_eq!(h.get("x-oxipage-build-id").unwrap(), "b1");
    assert_eq!(h.get("x-oxipage-build-theme").unwrap(), "paper");
    assert_eq!(h.get("x-oxipage-build-asset-revision").unwrap(), "abc");
    assert_eq!(h.get("x-oxipage-build-deployment-base").unwrap(), "/repo/");
}

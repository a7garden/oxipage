//! Tests for the create-site handler.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use oxipage_core::sites::SitesFile;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

async fn body_string(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn create_site_handler_seeds_toml_and_returns_slug() {
    let tmp = TempDir::with_prefix("oxipage-create-").unwrap();
    let target = tmp.path().join("blog");

    let registry = Arc::new(SiteRegistry::new(SitesFile::default()).await.unwrap());
    let app = build_console_router(registry);

    let body = serde_json::json!({ "path": target.to_str().unwrap() });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/setup/create-site")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = resp.status();
    let body = body_string(resp).await;
    assert_eq!(status, StatusCode::OK, "create-site failed: {body}");
    assert!(target.join("oxipage.toml").exists(), "oxipage.toml should exist");
}

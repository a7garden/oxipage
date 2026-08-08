//! Tests for the global `/mounts` CRUD endpoints.
//!
//! Mounts are config-driven: `oxibuilder.toml` is the single source of truth,
//! and the endpoints patch the raw toml doc (preserving comments/formatting and
//! the verbatim `source`) then atomically write + reload. These tests exercise
//! the round-trip against a real registered site.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxibuilder_console::router::build_console_router;
use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::sites::SitesFile;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn toml_minimal() -> &'static str {
    r#"[site]
name="Site"
base_url="http://127.0.0.1:8787/"

[server]
host="127.0.0.1"
port=8787
data_dir="data"

[extensions]
enabled=["profile"]
"#
}

async fn app_with_site() -> (TempDir, PathBuf, Router) {
    let dir = TempDir::with_prefix("oxibuilder-mounts-").unwrap();
    let path = dir.path().to_path_buf();
    std::fs::write(path.join("oxibuilder.toml"), toml_minimal()).unwrap();
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path.clone());
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf, Default::default()).await.unwrap());
    (dir, path, build_console_router(registry))
}

async fn send(app: Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(request).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn mount_crud_round_trip_preserves_raw_source() {
    let (_dir, path, app) = app_with_site().await;

    // Initially empty.
    let (s, v) = send(app.clone(), "GET", "/mounts", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["data"]["mounts"].as_array().unwrap().is_empty());

    // Add a mount with a RELATIVE source.
    let (s, v) = send(
        app.clone(),
        "POST",
        "/mounts",
        Some(json!({
            "id": "portfolio",
            "source": "../portfolio",
            "path": "portfolio",
            "title_ko": "포트폴리오",
            "title_en": "Portfolio",
            "description": "Hand-crafted",
            "icon": null,
            "open_in_new_tab": false,
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let mounts = v["data"]["mounts"].as_array().unwrap();
    assert_eq!(mounts.len(), 1);
    // Raw source preserved — NOT the build-resolved absolute path.
    assert_eq!(mounts[0]["source"], "../portfolio");
    assert_eq!(mounts[0]["id"], "portfolio");

    // The on-disk toml has the [[mounts]] entry verbatim (relative source).
    let on_disk = std::fs::read_to_string(path.join("oxibuilder.toml")).unwrap();
    assert!(on_disk.contains("[[mounts]]"));
    assert!(on_disk.contains("source = \"../portfolio\""));

    // Remove it.
    let (s, v) = send(app.clone(), "DELETE", "/mounts/portfolio", None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["data"]["mounts"].as_array().unwrap().is_empty());

    // On-disk no longer carries the mount.
    let on_disk = std::fs::read_to_string(path.join("oxibuilder.toml")).unwrap();
    assert!(!on_disk.contains("[[mounts]]"));
}

#[tokio::test]
async fn mount_add_rejects_reserved_prefix() {
    let (_dir, _path, app) = app_with_site().await;
    let (s, _v) = send(
        app,
        "POST",
        "/mounts",
        Some(json!({
            "id": "a", "source": "../x", "path": "assets",
            "title_ko": "k", "title_en": "e",
        })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mount_add_rejects_duplicate_id() {
    let (_dir, _path, app) = app_with_site().await;
    let payload = json!({
        "id": "portfolio", "source": "../portfolio", "path": "portfolio",
        "title_ko": "k", "title_en": "e",
    });
    let (s, _) = send(app.clone(), "POST", "/mounts", Some(payload.clone())).await;
    assert_eq!(s, StatusCode::OK);
    // Same id again → rejected by validate_mounts.
    let (s, _) = send(app, "POST", "/mounts", Some(payload)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn mount_rm_unknown_is_404() {
    let (_dir, _path, app) = app_with_site().await;
    let (s, _) = send(app, "DELETE", "/mounts/nope", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mounts_list_when_no_site_is_404() {
    // Empty registry — no default site to resolve.
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);
    let (s, _) = send(app, "GET", "/mounts", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mount_list_surfaces_resolved_source_for_auto_detected_dir() {
    let (dir, _path, app) = app_with_site().await;

    // External project root living next to the config: <dir>/extproj/dist/index.html
    let dist = dir.path().join("extproj").join("dist");
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "x").unwrap();

    // Add a mount whose raw source is the project root (not the dist).
    let (s, _v) = send(
        app.clone(),
        "POST",
        "/mounts",
        Some(json!({
            "id": "ext",
            "source": "extproj",
            "path": "ext",
            "title_ko": "k",
            "title_en": "e",
        })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, v) = send(app.clone(), "GET", "/mounts", None).await;
    assert_eq!(s, StatusCode::OK);
    let mounts = v["data"]["mounts"].as_array().unwrap();
    assert_eq!(mounts.len(), 1);
    // Raw source is preserved verbatim.
    assert_eq!(mounts[0]["source"], "extproj");
    // Resolved source points at the auto-detected dist dir (absolute).
    let resolved = mounts[0]["resolved_source"].as_str().unwrap();
    assert!(
        resolved.ends_with("extproj/dist"),
        "expected resolved source under extproj/dist, got {resolved}"
    );
    assert!(
        std::path::Path::new(resolved).is_absolute(),
        "should be absolute"
    );
}

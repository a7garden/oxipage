//! Tests for `PUT /s/{slug}/config` deploy patch — preservation of unknown
//! and startup-immutable keys, and rejection of invalid targets (no on-disk
//! mutation).

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use oxipage_core::sites::SitesFile;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn toml_with_server_and_custom() -> &'static str {
    r#"[site]
name="Site"
base_url="https://old.invalid/"

[server]
port=9123
data_dir="private-data"

[custom]
keep="yes"
"#
}

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

fn create_site_dir(name: &str, toml_text: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::with_prefix(format!("oxipage-cfg-{name}-")).unwrap();
    let toml_path = dir.path().join("oxipage.toml");
    std::fs::write(&toml_path, toml_text).unwrap();
    let p = dir.path().to_path_buf();
    (dir, p)
}

async fn site_router_with_toml(toml_text: &str) -> (TempDir, PathBuf, Router) {
    let (dir, path) = create_site_dir("cfg", toml_text);
    let mut sf = SitesFile::default();
    sf.add("blog".into(), path.clone());
    sf.set_default("blog");
    let registry = Arc::new(
        SiteRegistry::new(sf, Default::default(), Default::default())
            .await
            .unwrap(),
    );
    (dir, path, build_console_router(registry))
}

async fn site_router() -> (TempDir, PathBuf, Router) {
    site_router_with_toml(toml_minimal()).await
}

async fn put_json(app: Router, uri: &str, body: serde_json::Value) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn get_json(app: Router, uri: &str) -> serde_json::Value {
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn deploy_patch_preserves_server_and_unknown_keys() {
    let (_dir, project_dir, app) = site_router_with_toml(toml_with_server_and_custom()).await;
    let response = put_json(
        app,
        "/s/blog/config",
        json!({
            "deploy": { "github_pages": { "owner": "a7garden", "repo": "notes", "branch": "gh-pages" } }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let saved: toml::Value =
        toml::from_str(&std::fs::read_to_string(project_dir.join("oxipage.toml")).unwrap()).unwrap();
    assert_eq!(saved["server"]["port"].as_integer(), Some(9123));
    assert_eq!(saved["custom"]["keep"].as_str(), Some("yes"));
    assert_eq!(
        saved["deploy"]["github_pages"]["owner"].as_str(),
        Some("a7garden")
    );
    assert_eq!(
        saved["deploy"]["github_pages"]["repo"].as_str(),
        Some("notes")
    );
}

#[tokio::test]
async fn invalid_target_is_not_persisted() {
    let (_dir, project_dir, app) = site_router().await;
    let path = project_dir.join("oxipage.toml");
    let before = std::fs::read_to_string(&path).unwrap();
    let response = put_json(
        app,
        "/s/blog/config",
        json!({
            "deploy": { "github_pages": { "owner": "bad/name", "repo": "notes", "branch": "gh-pages" } }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}

#[tokio::test]
async fn deploy_response_includes_pages_url_and_base_path() {
    let (_dir, _project_dir, app) = site_router().await;
    let put = put_json(
        app,
        "/s/blog/config",
        json!({
            "deploy": { "github_pages": { "owner": "a7garden", "repo": "notes", "branch": "pages/v1" } }
        }),
    )
    .await;
    assert_eq!(put.status(), StatusCode::OK);
    let body = to_bytes(put.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let target = &v["data"]["deploy"]["github_pages"];
    assert_eq!(target["owner"].as_str(), Some("a7garden"));
    assert_eq!(target["repo"].as_str(), Some("notes"));
    assert_eq!(target["branch"].as_str(), Some("pages/v1"));
    assert_eq!(
        target["pages_url"].as_str(),
        Some("https://a7garden.github.io/notes/")
    );
    assert_eq!(target["base_path"].as_str(), Some("/notes/"));
}

#[tokio::test]
async fn get_config_returns_deploy_block() {
    // The site is loaded WITH the deploy section present in its toml, so the
    // load-time settings snapshot carries the target.
    let (_dir, _project_dir, app) = site_router_with_toml(
        r#"[site]
name="Site"
base_url="http://127.0.0.1:8787/"

[server]
host="127.0.0.1"
port=8787
data_dir="data"

[deploy.github_pages]
owner="a7garden"
repo="notes"
"#,
    )
    .await;
    let v = get_json(app, "/s/blog/config").await;
    let target = &v["data"]["deploy"]["github_pages"];
    assert_eq!(target["owner"].as_str(), Some("a7garden"));
    assert_eq!(target["repo"].as_str(), Some("notes"));
    assert_eq!(target["branch"].as_str(), Some("gh-pages"));
}

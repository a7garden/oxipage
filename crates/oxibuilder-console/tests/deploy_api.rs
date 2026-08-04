//! Tests for the deploy history + preflight + current-operation APIs.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxibuilder_console::operations::SiteOperationKind;
use oxibuilder_console::router::build_console_router;
use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::sites::SitesFile;
use sqlx::SqlitePool;
use std::sync::Arc;
use tempfile::TempDir;
use tower::util::ServiceExt;

fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787/"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"
"#
    )
}

async fn site_router() -> (TempDir, SqlitePool, Arc<SiteRegistry>, Router) {
    let dir = TempDir::with_prefix("oxibuilder-deploy-").unwrap();
    std::fs::write(dir.path().join("oxibuilder.toml"), minimal_toml("Test")).unwrap();
    let mut sf = SitesFile::default();
    sf.add("blog".into(), dir.path().to_path_buf());
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf, Default::default()).await.unwrap());
    let db = registry.ctx_for("blog").await.unwrap().db.clone();
    let app = build_console_router(registry.clone());
    (dir, db, registry, app)
}

async fn configured_site() -> (TempDir, SqlitePool, Arc<SiteRegistry>, Router) {
    let dir = TempDir::with_prefix("oxibuilder-deploy-").unwrap();
    std::fs::write(
        dir.path().join("oxibuilder.toml"),
        r#"[site]
name="Test"
base_url="https://o.github.io/r/"

[server]
host="127.0.0.1"
port=8787
data_dir="data"

[deploy.github_pages]
owner="o"
repo="r"
"#,
    )
    .unwrap();
    let mut sf = SitesFile::default();
    sf.add("blog".into(), dir.path().to_path_buf());
    sf.set_default("blog");
    let registry = Arc::new(SiteRegistry::new(sf, Default::default()).await.unwrap());
    let db = registry.ctx_for("blog").await.unwrap().db.clone();
    let app = build_console_router(registry.clone());
    (dir, db, registry, app)
}

fn write_manifest(out_dir: &std::path::Path, base: &str, theme: &str) {
    std::fs::create_dir_all(out_dir).unwrap();
    std::fs::write(
        out_dir.join(".oxibuilder-build.json"),
        format!(
            r#"{{"build_id":"b1","deployment_base":"{base}","theme_id":"{theme}","asset_revision":"a","built_at":"2026-07-31T00:00:00Z"}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        out_dir.join("index.html"),
        "<html><head><base href=\"/\"></head></html>",
    )
    .unwrap();
    std::fs::create_dir_all(out_dir.join("assets")).unwrap();
}

async fn get_json(app: Router, uri: &str) -> serde_json::Value {
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET {uri}");
    let bytes = to_bytes(resp.into_body(), 256 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn insert_deploy(db: &SqlitePool, run_id: &str, status: &str) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS deploy_log(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL UNIQUE,
            build_id TEXT NOT NULL,
            target TEXT NOT NULL,
            owner TEXT NOT NULL,
            repo TEXT NOT NULL,
            branch TEXT NOT NULL,
            base_path TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN('running','deployed','unchanged','failed')),
            url TEXT,
            commit_sha TEXT,
            error_code TEXT,
            error TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT
        )",
    )
    .execute(db)
    .await
    .unwrap();
    let _ = sqlx::query(
        "INSERT INTO deploy_log (run_id, build_id, target, owner, repo, branch, base_path, status, started_at)
         VALUES (?1, 'b', 'https://o.github.io/r/', 'o', 'r', 'gh-pages', '/', ?2, '2026-07-31T00:00:00Z')",
    )
    .bind(run_id)
    .bind(status)
    .execute(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn deploys_are_newest_first_and_limited() {
    let (_dir, db, _reg, app) = site_router().await;
    insert_deploy(&db, "r1", "deployed").await;
    insert_deploy(&db, "r2", "unchanged").await;
    let j = get_json(app, "/s/blog/deploys?limit=1").await;
    assert_eq!(j["data"].as_array().unwrap().len(), 1);
    assert_eq!(j["data"][0]["run_id"], "r2");
}

#[tokio::test]
async fn current_returns_common_run() {
    let (_dir, _db, reg, app) = site_router().await;
    reg.operation_guard
        .try_start("blog", "b7", SiteOperationKind::Build)
        .unwrap();
    let j = get_json(app, "/s/blog/operations/current").await;
    assert_eq!(j["data"]["run_id"], "b7");
    assert_eq!(j["data"]["kind"], "build");
}

#[tokio::test]
async fn preflight_reports_stale_base_and_theme() {
    let (dir, _db, _reg, app) = configured_site().await;
    let out_dir = dir.path().join("data").join("out");
    write_manifest(&out_dir, "/wrong/", "midnight");
    let j = get_json(app, "/s/blog/deploy/preflight").await;
    let codes: Vec<&str> = j["data"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["code"].as_str().unwrap())
        .collect();
    assert!(
        codes.contains(&"stale_build_base"),
        "expected stale_build_base, got {codes:?}"
    );
    assert!(
        codes.contains(&"stale_build_theme"),
        "expected stale_build_theme, got {codes:?}"
    );
}

#[tokio::test]
async fn deploy_post_rejects_preflight_failure_with_424() {
    let (dir, _db, _reg, app) = configured_site().await;
    let out_dir = dir.path().join("data").join("out");
    write_manifest(&out_dir, "/wrong/", "paper");
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/s/blog/deploy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FAILED_DEPENDENCY);
}

#[tokio::test]
async fn stats_uses_latest_deploy() {
    let (_dir, db, _reg, app) = site_router().await;
    insert_deploy(&db, "old", "deployed").await;
    insert_deploy(&db, "new", "unchanged").await;
    let j = get_json(app, "/s/blog/stats").await;
    assert_eq!(j["data"]["last_deploy"]["run_id"], "new");
}

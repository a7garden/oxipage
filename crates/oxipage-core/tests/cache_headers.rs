//! Tests for static asset cache policy.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_core::http::build_app;
use oxipage_core::state::AppState;
use std::sync::Arc;
use tower::util::ServiceExt;

async fn build_test_app() -> axum::Router {
    use oxipage_core::config::Config;
    use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
    use oxipage_core::registry::ExtensionRegistry;

    struct DummyExt;
    #[async_trait::async_trait]
    impl Extension for DummyExt {
        fn id(&self) -> &'static str { "dummy" }
        fn display_name(&self, l: Lang) -> String { "Dummy".into() }
        fn migrations(&self) -> Vec<Migration> {
            vec![Migration { version: 1, name: "init",
                sql: "CREATE TABLE IF NOT EXISTS dummy_t (id INTEGER PRIMARY KEY)" }]
        }
        fn table_names(&self) -> Vec<&'static str> { vec!["dummy_t"] }
        fn routes(&self) -> axum::Router { axum::Router::new() }
        async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> { None }
    }

    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry,
        wasm_loader: None,
        site_override: Arc::new(tokio::sync::RwLock::new(None)),
        builders: Arc::new(vec![]),
    };
    oxipage_core::http::build_app(state)
}

#[tokio::test]
async fn admin_html_has_no_cache_header() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/sites").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp.headers().get("cache-control").unwrap().to_str().unwrap();
    assert!(cc.contains("no-cache"), "cache-control was: {cc}");
}

#[tokio::test]
async fn hashed_asset_has_immutable_cache() {
    let app = build_test_app().await;
    // Extract the hashed JS asset URI from the embedded admin.html so the
    // test is robust to hash changes across builds.
    let html = oxipage_core::http::spa_index_html().unwrap_or_default();
    // Find the hashed `/assets/<name>-<hash>.js` script specifically; the
    // console entry also carries non-hashed boot scripts (theme-boot.js) that
    // are intentionally `no-cache` and must not be selected here.
    let asset = html
        .split("src=\"")
        .map(|s| s.split('"').next().unwrap_or(""))
        .find(|u| u.starts_with("/assets/") && u.ends_with(".js"))
        .expect("admin.html must reference a hashed /assets/ script");
    let resp = app
        .oneshot(
            Request::builder()
                .uri(asset)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "asset {asset} not found");
    let cc = resp.headers().get("cache-control").unwrap().to_str().unwrap();
    assert!(cc.contains("immutable"), "cache-control was: {cc}");
}

#[tokio::test]
async fn admin_html_has_revision_meta_and_header() {
    let app = build_test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/sites").body(Body::empty()).unwrap())
        .await
        .unwrap();
    // Capture the header BEFORE consuming the body — `into_body()` moves the
    // response and drops the headers.
    let header_rev = resp
        .headers()
        .get("X-Oxipage-SPA-Revision")
        .expect("X-Oxipage-SPA-Revision header must be set on admin.html")
        .to_str()
        .expect("X-Oxipage-SPA-Revision must be ASCII")
        .to_owned();
    assert!(!header_rev.is_empty(), "X-Oxipage-SPA-Revision must be non-empty");
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    let meta_rev = body
        .split("name=\"oxipage-spa-revision\" content=\"")
        .nth(1)
        .and_then(|s| s.split('\"').next())
        .expect("admin.html must carry the oxipage-spa-revision meta tag");
    assert_eq!(
        meta_rev, header_rev,
        "meta tag content and X-Oxipage-SPA-Revision header must agree"
    );
}
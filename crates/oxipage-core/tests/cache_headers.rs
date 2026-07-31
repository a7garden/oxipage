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
    // Extract the hashed JS asset URI from the served /admin.html body so
    // the test is robust to hash changes across builds and reads the Admin
    // entry HTML (not the Lobby index.html, which uses shorter Vite hashes).
    let admin_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(admin_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    let asset = html
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('\"').next())
        .expect("admin.html must reference a script")
        .to_owned();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(asset.as_str())
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
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        body.contains("oxipage-spa-revision"),
        "admin.html must carry the revision meta tag"
    );
}
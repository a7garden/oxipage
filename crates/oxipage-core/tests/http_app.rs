use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxipage_core::config::Config;
use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

struct DummyExt;

#[async_trait::async_trait]
impl Extension for DummyExt {
    fn id(&self) -> &'static str {
        "dummy"
    }
    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "더미".to_string(),
            Lang::En => "Dummy".to_string(),
        }
    }
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }
    fn routes(&self) -> axum::Router<AppState> {
        use axum::routing::get;
        axum::Router::new().route(
            "/",
            get(|| async { axum::Json(serde_json::json!({"data": {"ok": true}})) }),
        )
    }
    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }
}

async fn test_app() -> axum::Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: None,
        registry,
    };
    oxipage_core::http::build_app(state)
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn healthz_returns_ok() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains("\"ok\""));
}

#[tokio::test]
async fn manifest_lists_enabled_extensions() {
    let app = test_app().await;
    let res = app
        .oneshot(
            Request::get("/api/v1/lobby/manifest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.contains("\"id\":\"dummy\""));
    assert!(body.contains("\"ko\":\"더미\""));
    assert!(body.contains("\"en\":\"Dummy\""));
    assert!(body.contains("\"default_lang\":\"ko\""));
}

#[tokio::test]
async fn extension_routes_are_mounted_under_api_v1() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/api/v1/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_string(res).await.contains("\"ok\":true"));
}

#[tokio::test]
async fn spa_fallback_serves_index_html_for_unknown_paths() {
    let app = test_app().await;
    let res = app
        .oneshot(
            Request::get("/some/unknown/path")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.to_lowercase().contains("<!doctype html"));
}

#[tokio::test]
async fn unknown_api_path_returns_404_json() {
    let app = test_app().await;
    let res = app
        .oneshot(Request::get("/api/v1/nope/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_string(res).await;
    assert!(body.contains("\"error\""));
}

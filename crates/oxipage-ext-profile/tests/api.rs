use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use oxipage_ext_profile::ProfileExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ProfileExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        registry: registry.clone(),
        wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    for e in registry.iter() {
        e.on_startup(&state).await.unwrap();
    }
    let ext_router = registry.find("profile").unwrap().routes();
    Router::new()
        .nest("/api/v1/profile", ext_router)
        .with_state(state)
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn get_profile_returns_seeded_singleton() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(Request::get("/api/v1/profile").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["display_name"], "Oxipage"); // Config::default().site.name
}

#[tokio::test]
async fn put_roundtrip_updates_profile() {
    let app = test_app(Some("tok")).await;
    let payload = r#"{
        "display_name": "김개발",
        "tagline_ko": "밤에 코드를 짜는 사람",
        "tagline_en": "codes at night",
        "email": "me@example.dev",
        "github_username": "myid",
        "education": [{"institution": "SNU", "degree": "BS", "field": "CS", "start_year": 2018, "end_year": 2022}],
        "custom_links": [{"label": "Blog", "url": "https://blog.example.dev", "icon": null}]
    }"#;
    let res = app
        .clone()
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .header("authorization", "Bearer tok")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["display_name"], "김개발");
    assert_eq!(json["data"]["education"][0]["institution"], "SNU");

    let res = app
        .oneshot(Request::get("/api/v1/profile").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"]["tagline_en"], "codes at night");
    assert_eq!(json["data"]["github_username"], "myid");
    assert_eq!(
        json["data"]["custom_links"][0]["url"],
        "https://blog.example.dev"
    );
}

#[tokio::test]
async fn put_with_empty_display_name_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::put("/api/v1/profile")
                .header("content-type", "application/json")
                .header("authorization", "Bearer tok")
                .body(Body::from(r#"{"display_name":"  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "validation_error");
    assert_eq!(json["error"]["field"], "display_name");
}

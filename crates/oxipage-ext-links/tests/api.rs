use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::{AppState, SiteScopedDb};
use oxipage_ext_links::LinksExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(LinksExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(Config::default()),
        registry: registry.clone(),
        wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    for e in registry.iter() {
        e.on_startup(&state).await.unwrap();
    }
    let ext_router = registry.find("links").unwrap().routes();
    Router::new()
        .nest("/api/console/links", ext_router)
        .layer(Extension(SiteScopedDb { db: pool }))
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[tokio::test]
async fn create_with_empty_title_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/links")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"title":"  ","url":"https://y"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_invalid_url_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/links")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"title":"x","url":"ftp://y"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_show_patch_delete_and_featured_filter() {
    let app = test_app(Some("tok")).await;
    // 2개 생성 (1번째 featured, order=1)
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/links")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"First","url":"https://a.example","display_order":1,"featured":true}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let id1 = json["data"]["id"].as_i64().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/links")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"Second","url":"https://b.example","display_order":2}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let id2 = json["data"]["id"].as_i64().unwrap();

    // 전체 목록 2개, order 순
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/links")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let arr = json["data"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["title"], "First");

    // featured 필터 — 1개
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/links?featured=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    // show
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/links/{id1}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // patch
    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/console/links/{id2}"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"title":"Second Updated"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["title"], "Second Updated");

    // delete id1
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/console/links/{id1}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 삭제 후 총 1개
    let res = app
        .oneshot(
            Request::get("/api/console/links")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

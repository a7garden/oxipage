use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::{AppState, SiteScopedDb};
use oxipage_ext_novels::NovelsExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(NovelsExtension)]));
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
    let r = registry.find("novels").unwrap().routes();
    Router::new()
        .nest("/api/console/novels", r)
        .layer(Extension(SiteScopedDb {
            db: pool,
            settings: std::sync::Arc::new(tokio::sync::RwLock::new(
                oxipage_core::site_paths::MutableSiteSettings::from_config(
                    &oxipage_core::config::Config::default(),
                )
            )),
        }))
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn bearer(t: &str) -> String {
    format!("Bearer {t}")
}

#[tokio::test]
async fn novel_create_publish_with_chapter_charcount() {
    let app = test_app(Some("tok")).await;
    // 소설 초안 생성
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/novels")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"빛의 이야기","status":"ongoing"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();
    assert!(json["data"]["published_at"].is_null());

    // 챕터 추가 — char_count 자동 계산 (공백 제외)
    let body = "안녕하세요 세계 1234"; // 공백 제외: 안녕하세요5 + 세계2 + 1234 = 11자
    let expected_cc: i64 = 11;
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/novels/{slug}/chapters"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(format!(
                    r##"{{"chapter_order":1,"title":"1화","body":"{body}"}}"##
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let json = body_json(res).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["char_count"], expected_cc);

    // 발행
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/novels/{slug}/chapters/1/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // show chapter — 발행본
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/novels/{slug}/chapters/1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["body"], body);

    // 챕터 목록 (발행본만)
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/novels/{slug}/chapters"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

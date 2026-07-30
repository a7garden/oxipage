use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::SiteScopedDb;
use oxipage_ext_blog::BlogExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(BlogExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    // Blog extension's on_startup still needs AppState, create minimally
    let state = oxipage_core::state::AppState {
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
    let ext_router = registry.find("blog").unwrap().routes();
    Router::new()
        .nest("/api/console/blog", ext_router)
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
            Request::post("/api/console/blog")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r#"{"title":"  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_invalid_lang_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/blog")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r#"{"title":"x","lang":"ja"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn draft_create_then_publish_flow() {
    let app = test_app(Some("tok")).await;
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/blog")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"Hello Rust","body":"# body","lang":"en","tags":["rust"]}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();
    assert!(json["data"]["published_at"].is_null());

    // 초안은 공개 show에서 404
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/blog/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 발행본 목록은 비어 있음
    let res = app
        .clone()
        .oneshot(Request::get("/api/console/blog").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 0);

    // 초안 목록에는 1개
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/blog?draft=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    // 발행
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/blog/{slug}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert!(json["data"]["published_at"].is_string());

    // 이제 show가 200
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/blog/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 발행본 목록에 1개
    let res = app
        .oneshot(Request::get("/api/console/blog").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn fts_index_on_publish() {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(BlogExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();

    let _draft = oxipage_ext_blog::repo::create(
        &pool,
        &oxipage_ext_blog::model::BlogPostInput {
            title: "Rust Ownership".into(),
            body: "Ownership borrowing lifetime".into(),
            lang: "en".into(),
            tags: vec![],
            translation_group_id: None,
            slug: Some("rust-ownership".into()),
        },
        "rust-ownership",
    )
    .await
    .unwrap();
    let post = oxipage_ext_blog::repo::publish(&pool, "rust-ownership")
        .await
        .unwrap();
    oxipage_core::search::upsert(
        &pool,
        "blog",
        &post.slug,
        &post.title,
        &post.body,
        Some(&post.lang),
        post.published_at.as_deref(),
    )
    .await
    .unwrap();

    let hits = oxipage_core::search::search(&pool, "ownership", None, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].doc_id, "rust-ownership");
    let hits = oxipage_core::search::search(&pool, "ownership", None, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].doc_id, "rust-ownership");
}

/// SSR 스냅샷 호출은 best-effort이라 file 시스템 검증은 회피하고,
/// publish가 200 응답으로 정상 완료되는지만 확인한다.
/// (실제 file 생성은 `snapshot::write_snapshot_for` 단위 테스트가 보장).
#[tokio::test]
async fn publish_does_not_block_on_ssr_failure() {
    let app = test_app(Some("tok")).await;
    // 초안 생성
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/blog")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"Snapshot Test","body":"body content here","lang":"ko"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();

    // SSR 보조 호출이 실패(예: index.html 미임베드)해도 publish API는 200을 반환해야 한다.
    let res = app
        .oneshot(
            Request::post(format!("/api/console/blog/{slug}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert!(json["data"]["published_at"].is_string());
}

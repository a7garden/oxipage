use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::{AppState, SiteScopedDb};
use oxipage_ext_books::BooksExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(BooksExtension)]));
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
    let r = registry.find("books").unwrap().routes();
    Router::new()
        .nest("/api/console/books", r)
        .layer(Extension(SiteScopedDb {
            db: pool,
            settings: std::sync::Arc::new(tokio::sync::RwLock::new(
                oxipage_core::site_paths::MutableSiteSettings::from_config(
                    &oxipage_core::config::Config::default(),
                ),
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

// ─── Acceptance tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn manual_book_create_publish_show() {
    let app = test_app(Some("tok")).await;
    // create (rating 7)
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/books")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"프로젝트 헤일메리","author":"앤드류 위어","rating":7,"review_ko":"좋은 책"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let id = json["data"]["id"].as_i64().unwrap();
    assert_eq!(json["data"]["rating"], 7);
    assert_eq!(json["data"]["status"], "wishlist");
    assert!(json["data"]["published_at"].is_null());

    // show 전: 미발행은 404
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/books/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // publish
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/books/{id}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // show — 발행본
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/books/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["id"], id);
    assert_eq!(json["data"]["title"], "프로젝트 헤일메리");
    assert_eq!(json["data"]["rating"], 7);
    assert!(json["data"]["published_at"].is_string());

    // list 에 포함
    let res = app
        .oneshot(
            Request::get("/api/console/books")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let arr = json["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id);
}

#[tokio::test]
async fn rating_out_of_range_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/books")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"title":"x","rating":11}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn invalid_status_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/books")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"title":"x","rating":5,"status":"weird"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // ENV_LOCK은 의도된 전역 env 격리 가드
async fn external_search_no_aladin_key_is_503() {
    // `OXIPAGE_ALADIN_TTBKEY`가 unset이면 503. 테스트 환경에서 unset 가정.
    // 다른 테스트와 env 변수를 공유하므로 cargo test는 보통 unset 상태로 시작한다.
    // 격리를 위해 mutex로 보호.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[allow(clippy::await_holding_lock)] // ENV_LOCK은 의도된 전역 env 격리 가드
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prior = std::env::var("OXIPAGE_ALADIN_TTBKEY").ok();
    unsafe {
        std::env::remove_var("OXIPAGE_ALADIN_TTBKEY");
    }
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::get("/api/console/books/search?q=rust")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    if let Some(v) = prior {
        unsafe {
            std::env::set_var("OXIPAGE_ALADIN_TTBKEY", v);
        }
    }
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "book_search_disabled");
}

// ─── Additional coverage ─────────────────────────────────────────────────────

#[tokio::test]
async fn status_filter_and_patch_and_delete() {
    let app = test_app(Some("tok")).await;
    // 2개 생성, 1개는 reading
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/books")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"Book A","rating":6,"status":"reading"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let id_a = body_json(res).await["data"]["id"].as_i64().unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/books")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title":"Book B","rating":9,"status":"completed"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let id_b = body_json(res).await["data"]["id"].as_i64().unwrap();

    // 둘 다 publish
    for id in [id_a, id_b] {
        let res = app
            .clone()
            .oneshot(
                Request::post(format!("/api/console/books/{id}/publish"))
                    .header(AUTHORIZATION, bearer("tok"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // status filter
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/books?status=reading")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let arr = json["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id_a);

    // 잘못된 status → 422
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/books?status=nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // patch (rating 8, review_en 채움)
    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/console/books/{id_b}"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"rating":8,"review_en":"Solid sci-fi."}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["rating"], 8);
    assert_eq!(json["data"]["review_en"], "Solid sci-fi.");

    // delete
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/console/books/{id_a}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 다시 list — 1개
    let res = app
        .oneshot(
            Request::get("/api/console/books")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn show_unknown_id_is_404() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::get("/api/console/books/9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

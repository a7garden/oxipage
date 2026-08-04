use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxibuilder_core::config::Config;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::{AppState, SiteScopedDb};
use oxibuilder_ext_scraps::ScrapsExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ScrapsExtension)]));
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
    let r = registry.find("scraps").unwrap().routes();
    Router::new()
        .nest("/api/console/scraps", r)
        .layer(Extension(SiteScopedDb {
            db: pool,
            settings: std::sync::Arc::new(tokio::sync::RwLock::new(
                oxibuilder_core::site_paths::MutableSiteSettings::from_config(
                    &oxibuilder_core::config::Config::default(),
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

#[tokio::test]
async fn create_invalid_source_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/scraps")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"source_url":"https://x.example","title":"x","source":"reddit"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_empty_title_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/scraps")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"source_url":"https://x.example","title":"   "}"##,
                ))
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
            Request::post("/api/console/scraps")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"source_url":"ftp://x","title":"x"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn manual_create_list_show_patch_flow() {
    let app = test_app(Some("tok")).await;

    // manual 추가 — 즉시 발행본
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/scraps")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"source_url":"https://a.example","title":"First","note_ko":"안녕"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let id = json["data"]["id"].as_i64().unwrap();
    assert_eq!(json["data"]["source"], "manual");
    assert!(json["data"]["published_at"].is_string());
    assert_eq!(json["data"]["note_ko"], "안녕");

    // list — 1개
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    // show — 단건
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/scraps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["id"], id);
    assert_eq!(json["data"]["title"], "First");

    // patch — note 갱신
    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/console/scraps/{id}"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"note_en":"hello","tags":["rust","weekly"]}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["note_en"], "hello");
    assert_eq!(json["data"]["tags"].as_array().unwrap().len(), 2);

    // patch 이후 GET /{id} 로 note_en/tags 가 영구 반영됐는지 검증 (DB round-trip)
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/scraps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["note_en"], "hello");
    let tags = json["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|v| v == "rust"));
    assert!(tags.iter().any(|v| v == "weekly"));
    // delete — 단건 삭제
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/console/scraps/{id}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 삭제 후 비어 있음
    let res = app
        .oneshot(
            Request::get("/api/console/scraps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn queue_publish_and_source_filter_flow() {
    let app = test_app(Some("tok")).await;
    let state = app.clone();
    // 직접 풀을 얻기 어려우니 registry에서 잡을 통해 마이그레이션만 적용된
    // 메모리 풀을 만들어 큐 row를 미리 insert 한다 (test 격리).
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ScrapsExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();

    // 큐 후보 2개 insert (hackernews 1, geeknews 1) + 수동 발행 1개
    let hn = oxibuilder_ext_scraps::repo::upsert_queue_item(
        &pool,
        "hackernews",
        "111",
        "https://hn.example/a",
        "HN Story",
        None,
    )
    .await
    .unwrap();
    let gn = oxibuilder_ext_scraps::repo::upsert_queue_item(
        &pool,
        "geeknews",
        "222",
        "https://gn.example/b",
        "GN Story",
        None,
    )
    .await
    .unwrap();
    assert!(hn.published_at.is_none());
    assert!(gn.published_at.is_none());

    // 별도 app — admin 토큰으로 핸들러 검증
    let state_app = AppState {
        db: pool.clone(),
        config: Arc::new(Config::default()),
        registry: registry.clone(),
        wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    for e in registry.iter() {
        e.on_startup(&state_app).await.unwrap();
    }
    let r = registry.find("scraps").unwrap().routes();
    let app = Router::new()
        .nest("/api/console/scraps", r)
        .layer(Extension(SiteScopedDb {
            db: pool,
            settings: std::sync::Arc::new(tokio::sync::RwLock::new(
                oxibuilder_core::site_paths::MutableSiteSettings::from_config(
                    &oxibuilder_core::config::Config::default(),
                ),
            )),
        }));

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps/queue")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 2);

    // queue source 필터
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps/queue?source=geeknews")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let arr = json["data"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["source"], "geeknews");

    // 큐 row는 일반 list에 안 나옴
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 0);

    // 큐 row 공개 show는 404 (미발행)
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/scraps/{}", hn.id))
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
            Request::post(format!("/api/console/scraps/{}/publish", hn.id))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert!(json["data"]["published_at"].is_string());

    // publish 후 list — 1개
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    // publish 후 show 200
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/scraps/{}", hn.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // queue — 1개 남음 (geeknews)
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps/queue")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);

    // publish 없는 id → 404
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/scraps/9999/publish")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 이미 발행된 row를 다시 publish 시도 → 404 (UPDATE ... WHERE published_at IS NULL)
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/scraps/{}/publish", hn.id))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 상태를 다시 사용하지 않도록 unused 경고 회피
    let _ = state;
}

#[tokio::test]
async fn list_source_filter() {
    let app = test_app(Some("tok")).await;
    // manual 항목 2개 추가 (source 필터는 published 만 받음)
    for (title, url) in [("A", "https://a.example"), ("B", "https://b.example")] {
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/console/scraps")
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION, bearer("tok"))
                    .body(Body::from(format!(
                        r##"{{"source_url":"{url}","title":"{title}"}}"##
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps?source=manual")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 2);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/scraps?source=hackernews")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn patch_note_contract_404_and_persistence() {
    let app = test_app(Some("tok")).await;

    // seed one manual scrap
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/scraps")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"source_url":"https://a.example","title":"Seed","note_ko":"원본"}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let id = json["data"]["id"].as_i64().unwrap();

    // PATCH unknown id → 404
    let res = app
        .clone()
        .oneshot(
            Request::patch("/api/console/scraps/9999")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r##"{"note_ko":"x"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // PATCH note_ko + tags → 200, 응답에 반영
    let res = app
        .clone()
        .oneshot(
            Request::patch(format!("/api/console/scraps/{id}"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"note_ko":"수정","note_en":"updated","tags":["a","b"]}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["note_ko"], "수정");
    assert_eq!(json["data"]["note_en"], "updated");
    let tags = json["data"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);

    // GET /{id} 로 DB 에 영구 반영 확인
    let res = app
        .oneshot(
            Request::get(format!("/api/console/scraps/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["note_ko"], "수정");
    assert_eq!(json["data"]["note_en"], "updated");
    let tags = json["data"]["tags"].as_array().unwrap();
    assert!(tags.iter().any(|v| v == "a"));
    assert!(tags.iter().any(|v| v == "b"));
}

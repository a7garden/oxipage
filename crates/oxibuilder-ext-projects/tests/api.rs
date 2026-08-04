use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::Extension;
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxibuilder_core::config::Config;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::{AppState, SiteScopedDb};
use oxibuilder_ext_projects::ProjectsExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(_admin_token: Option<&str>) -> Router {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ProjectsExtension)]));
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
    let ext_router = registry.find("projects").unwrap().routes();
    Router::new()
        .nest("/api/console/projects", ext_router)
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

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[tokio::test]
async fn create_both_titles_null_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/projects")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r#"{"status":"wip"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_invalid_status_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/console/projects")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(r#"{"title_en":"x","status":"live"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn draft_create_publish_with_screenshots() {
    let app = test_app(Some("tok")).await;
    // 초안 생성
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/console/projects")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::from(
                    r##"{"title_ko":"내 프로젝트","title_en":"My Project","status":"wip","tech_stack":["rust"]}"##,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();
    assert!(json["data"]["published_at"].is_null());

    // 초안은 show 404
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/projects/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 스크린샷 2개 추가
    for (i, alt) in ["main", "detail"].iter().enumerate() {
        let body = format!(r##"{{"url":"https://x/{i}.png","alt_en":"{alt}"}}"##);
        let res = app
            .clone()
            .oneshot(
                Request::post(format!("/api/console/projects/{slug}/screenshots"))
                    .header("content-type", "application/json")
                    .header(AUTHORIZATION, bearer("tok"))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "screenshot add");
    }

    // 발행
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/console/projects/{slug}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // show에 screenshots 2개
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/console/projects/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["title_en"], "My Project");

    // 스크린샷 하나 삭제
    let sid = json["data"]["screenshots"][0]["id"].as_i64().unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/console/projects/{slug}/screenshots/{sid}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 이제 1개
    let res = app
        .oneshot(
            Request::get(format!("/api/console/projects/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"]["screenshots"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn fts_index_on_publish() {
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ProjectsExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();

    oxibuilder_ext_projects::repo::create(
        &pool,
        &oxibuilder_ext_projects::model::ProjectInput {
            title_ko: None,
            title_en: Some("Oxibuilder".into()),
            description_ko: None,
            description_en: Some("Personal site engine".into()),
            tech_stack: vec!["rust".into()],
            status: "wip".into(),
            started_at: None,
            ended_at: None,
            links: serde_json::json!({}),
            featured: false,
            slug: Some("oxibuilder".into()),
        },
        "oxibuilder",
    )
    .await
    .unwrap();
    let project = oxibuilder_ext_projects::repo::publish(&pool, "oxibuilder")
        .await
        .unwrap();
    let title = project
        .title_en
        .clone()
        .or(project.title_ko.clone())
        .unwrap_or_default();
    let body = project
        .description_en
        .clone()
        .or(project.description_ko.clone())
        .unwrap_or_default();
    oxibuilder_core::search::upsert(
        &pool,
        "projects",
        &project.slug,
        &title,
        &body,
        Some("en"),
        project.published_at.as_deref(),
    )
    .await
    .unwrap();

    let hits = oxibuilder_core::search::search(&pool, "oxibuilder", None, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].extension_id, "projects");
}

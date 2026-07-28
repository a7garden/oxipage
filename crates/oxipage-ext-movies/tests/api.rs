use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header::AUTHORIZATION};
use oxipage_core::config::Config;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use oxipage_ext_movies::MoviesExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(MoviesExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: admin_token.map(Arc::<str>::from),
        registry: registry.clone(),
        wasm_loader: None,
    };
    for e in registry.iter() {
        e.on_startup(&state).await.unwrap();
    }
    let ext_router = registry.find("movies").unwrap().routes();
    Router::new()
        .nest("/api/v1/movies", ext_router)
        .with_state(state)
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn json_body(s: &str) -> Body {
    Body::from(s.to_string())
}

// ─── 인증 / 검증 ───

#[tokio::test]
async fn create_without_token_is_401() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .body(json_body(r#"{"media_type":"movie","title":"X","rating":8}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rating_11_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"movie","title":"X","rating":11}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "validation_error");
    assert_eq!(json["error"]["field"], "rating");
}

#[tokio::test]
async fn rating_negative_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"movie","title":"X","rating":-1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn invalid_media_type_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"anime","title":"X","rating":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn no_title_no_tmdb_id_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"movie","rating":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ─── CRUD + 발행 ───

#[tokio::test]
async fn manual_create_publish_show_flow() {
    let app = test_app(Some("tok")).await;

    // 1) 초안 생성 (rating 8)
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(
                    r#"{
                        "media_type": "movie",
                        "title": "Parasite",
                        "release_year": 2019,
                        "watched_at": "2024-05-01",
                        "rating": 8,
                        "review_ko": "봉준호 최고",
                        "rewatch": false
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();
    assert_eq!(json["data"]["rating"].as_i64().unwrap(), 8);
    assert!(json["data"]["published_at"].is_null());

    // 2) 초안은 공개 show에서 404
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/movies/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // 3) 발행본 목록은 비어있음
    let res = app
        .clone()
        .oneshot(Request::get("/api/v1/movies").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 0);

    // 4) publish
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/movies/{slug}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert!(json["data"]["published_at"].is_string());

    // 5) 발행본 show는 200
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/movies/{slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["title"], "Parasite");
    assert_eq!(json["data"]["rating"].as_i64().unwrap(), 8);

    // 6) 발행본 목록에 1개
    let res = app
        .oneshot(Request::get("/api/v1/movies").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn patch_updates_rating() {
    let app = test_app(Some("tok")).await;
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"movie","title":"Old","rating":3}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();

    let res = app
        .oneshot(
            Request::patch(format!("/api/v1/movies/{slug}"))
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"rating":9}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["rating"].as_i64().unwrap(), 9);
}

#[tokio::test]
async fn delete_removes_entry() {
    let app = test_app(Some("tok")).await;
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"media_type":"movie","title":"Doomed","rating":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    let slug = json["data"]["slug"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/v1/movies/{slug}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::delete(format!("/api/v1/movies/{slug}"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// ─── SeriesGroup ───

#[tokio::test]
async fn create_group_and_attach_movie() {
    let app = test_app(Some("tok")).await;

    // 1) 시리즈 생성
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/movies/series")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(
                    r#"{
                        "title_ko": "해리포터",
                        "title_en": "Harry Potter",
                        "group_rating": 8
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let group_slug = json["data"]["slug"].as_str().unwrap().to_string();
    let group_id = json["data"]["id"].as_i64().unwrap();
    assert_eq!(json["data"]["group_rating"].as_i64().unwrap(), 8);

    // 2) 시리즈에 영화 등록
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/v1/movies")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(&format!(
                    r#"{{
                        "media_type": "movie",
                        "title": "해리포터와 마법사의 돌",
                        "release_year": 2001,
                        "rating": 8,
                        "series_group_id": {group_id},
                        "series_order": 1
                    }}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["series_group_id"].as_i64().unwrap(), group_id);
    assert_eq!(json["data"]["series_order"].as_i64().unwrap(), 1);
    let movie_slug = json["data"]["slug"].as_str().unwrap().to_string();

    // 3) 영화 publish
    let res = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/movies/{movie_slug}/publish"))
                .header(AUTHORIZATION, bearer("tok"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4) 시리즈 단건 → entries 포함
    let res = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/movies/series/{group_slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["data"]["title_ko"], "해리포터");
    assert_eq!(json["data"]["entries"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"]["entries"][0]["title"], "해리포터와 마법사의 돌");

    // 5) list?series_group= 필터
    let res = app
        .oneshot(
            Request::get(format!("/api/v1/movies?series_group={group_slug}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"][0]["title"], "해리포터와 마법사의 돌");
}

#[tokio::test]
async fn group_with_no_titles_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies/series")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"cover_image": "x.png"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn group_rating_out_of_range_is_422() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/movies/series")
                .header("content-type", "application/json")
                .header(AUTHORIZATION, bearer("tok"))
                .body(json_body(r#"{"title_ko":"X","group_rating":15}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ─── TMDB search ───

#[tokio::test]
async fn tmdb_search_disabled_when_no_key() {
    // 환경에 키가 이미 설정돼 있으면 이 테스트는 skip — 실제 fetch는 mocking하지 않는다.
    if std::env::var("OXIPAGE_TMDB_KEY").is_ok() {
        eprintln!("OXIPAGE_TMDB_KEY is set; skipping tmdb_disabled assertion");
        return;
    }
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::get("/api/v1/movies/search?q=inception")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let json = body_json(res).await;
    assert_eq!(json["error"]["code"], "tmdb_disabled");
}

// ─── FTS ───

#[tokio::test]
async fn fts_index_on_publish() {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(MoviesExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();

    use oxipage_ext_movies::model::{MovieEntryInput, MovieEntryPatch};
    use oxipage_ext_movies::repo;

    let input = MovieEntryInput {
        tmdb_id: None,
        media_type: "movie".into(),
        title: Some("Inception".into()),
        poster_path: None,
        release_year: Some(2010),
        watched_at: Some("2024-01-01".into()),
        rating: 9,
        review_ko: None,
        review_en: Some("Dreams within dreams".into()),
        rewatch: false,
        series_group_id: None,
        series_order: None,
        slug: Some("inception".into()),
    };
    let _ = repo::create_entry(
        &pool,
        &input,
        "inception",
        input.tmdb_id,
        input.title.clone().unwrap(),
        input.poster_path.clone(),
        input.release_year,
    )
    .await
    .unwrap();

    // publish → FTS upsert
    let published = repo::publish_entry(&pool, "inception").await.unwrap();
    oxipage_core::search::upsert(
        &pool,
        "movies",
        &published.slug,
        &published.title,
        published.review_en.as_deref().unwrap_or(""),
        Some("en"),
        published.published_at.as_deref(),
    )
    .await
    .unwrap();

    let hits = oxipage_core::search::search(&pool, "dreams", None, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].doc_id, "inception");
    assert_eq!(hits[0].extension_id, "movies");

    // PATCH 도 동작하는지 (rating 0~10 범위).
    let patch = MovieEntryPatch {
        rating: Some(8),
        ..Default::default()
    };
    let updated = repo::update_entry(&pool, "inception", &patch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.rating, 8);
}

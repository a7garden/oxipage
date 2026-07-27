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

// ─── PAT 스코프 강제 통합 테스트 (doc/01 §1.8, doc/04 §4.2) ───

use oxipage_core::auth::sha256_hex;
use sqlx::SqlitePool;

async fn pat_setup(admin_token: Option<&str>) -> (axum::Router, SqlitePool) {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool).await.unwrap();
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(Config::default()),
        admin_token: admin_token.map(Arc::<str>::from),
        registry,
    };
    let app = oxipage_core::http::build_app(state);
    (app, pool)
}

async fn seed_pat(pool: &SqlitePool, label: &str, scopes: &[&str]) -> String {
    let plain = format!("oxp_test_{label}");
    let hash = sha256_hex(plain.as_bytes());
    let prefix: String = plain.chars().take(12).collect();
    sqlx::query(
        "INSERT INTO auth_token (label, token_hash, token_prefix, scopes)
         VALUES (?, ?, ?, ?)",
    )
    .bind(label)
    .bind(&hash)
    .bind(&prefix)
    .bind(serde_json::to_string(scopes).unwrap())
    .execute(pool)
    .await
    .unwrap();
    plain
}

async fn put_lobby_config(app: axum::Router, token: &str) -> axum::response::Response {
    app.oneshot(
        Request::put("/api/v1/lobby/config/dummy")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"display_mode":"grid"}"#))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn post_create_pat(app: axum::Router, token: &str) -> axum::response::Response {
    app.oneshot(
        Request::post("/api/v1/auth/tokens")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"label":"x","scopes":["read"]}"#))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn pat_read_only_rejected_on_write_route() {
    // read 전용 PAT는 AdminAuth 진입 단계에서 403.
    let (app, pool) = pat_setup(Some("root")).await;
    let pat = seed_pat(&pool, "reader", &["read"]).await;
    let res = put_lobby_config(app, &pat).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pat_write_can_write_but_not_admin_action() {
    let (app, pool) = pat_setup(Some("root")).await;
    let pat = seed_pat(&pool, "writer", &["post:write"]).await;
    // 쓰기 라우트(lobby config)는 통과.
    let res = put_lobby_config(app.clone(), &pat).await;
    assert_eq!(res.status(), StatusCode::OK);
    // 토큰 관리(admin 스코프)는 거부.
    let res = post_create_pat(app, &pat).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pat_admin_can_manage_tokens() {
    let (app, pool) = pat_setup(Some("root")).await;
    let pat = seed_pat(&pool, "admin_tok", &["admin"]).await;
    let res = post_create_pat(app, &pat).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.contains("data"), "expected data envelope: {body}");
}

#[tokio::test]
async fn pat_with_neither_admin_nor_write_is_rejected_even_with_valid_token() {
    // read-only PAT는 publish-style(admin) 라우트는 물론 일반 쓰기도 불가.
    let (app, pool) = pat_setup(Some("root")).await;
    let pat = seed_pat(&pool, "reader2", &["read"]).await;
    let res = post_create_pat(app, &pat).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

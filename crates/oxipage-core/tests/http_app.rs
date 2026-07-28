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
        vec![Migration {
            version: 1,
            name: "init",
            sql: "CREATE TABLE IF NOT EXISTS dummy_t (id INTEGER PRIMARY KEY)",
        }]
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["dummy_t"]
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
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: None,
        registry,
                wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
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
            Request::get("/api/console/lobby/manifest")
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
        .oneshot(Request::get("/api/console/dummy").body(Body::empty()).unwrap())
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
        .oneshot(Request::get("/api/console/nope/").body(Body::empty()).unwrap())
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
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(Config::default()),
        admin_token: admin_token.map(Arc::<str>::from),
        registry,
                wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
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
        Request::put("/api/console/lobby/config/dummy")
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
        Request::post("/api/console/auth/tokens")
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

// ─── extension lifecycle (doc/02 §2.13, doc/04 §4.3) ───

async fn admin_app() -> axum::Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: Some(Arc::from("test-admin-token")),
        registry,
                wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    oxipage_core::http::build_app(state)
}

fn admin_req(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", "Bearer test-admin-token")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn extensions_list_returns_admin_state() {
    let app = admin_app().await;
    let res = app.oneshot(admin_req("GET", "/api/console/extensions")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.contains("dummy"), "body: {body}");
}

#[tokio::test]
async fn disable_gates_extension_routes() {
    let app = admin_app().await;
    let before = app
        .clone()
        .oneshot(Request::get("/api/console/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);
    let res = app
        .clone()
        .oneshot(admin_req("POST", "/api/console/extensions/dummy/disable"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let gated = app
        .oneshot(Request::get("/api/console/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(gated.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn enable_restores_extension_routes() {
    let app = admin_app().await;
    app.clone()
        .oneshot(admin_req("POST", "/api/console/extensions/dummy/disable"))
        .await
        .unwrap();
    let res = app
        .clone()
        .oneshot(admin_req("POST", "/api/console/extensions/dummy/enable"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let after = app
        .oneshot(Request::get("/api/console/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::OK);
}

#[tokio::test]
async fn purge_marks_extension_and_gates_routes() {
    let app = admin_app().await;
    let res = app
        .clone()
        .oneshot(admin_req("DELETE", "/api/console/extensions/dummy"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res).await;
    assert!(body.contains("\"purged\":true"), "body: {body}");
    let gated = app
        .oneshot(Request::get("/api/console/dummy").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(gated.status(), StatusCode::NOT_FOUND);
}

// ─── wasm runtime install (doc/08 §8.4) ───

#[tokio::test]
async fn install_writes_wasm_and_registers_state() {
    // data_dir 을 임시 디렉토리로 격리 — 테스트가 리포지토리 data/ 를 더럽히지 않게.
    let data_dir =
        std::env::temp_dir().join(format!("oxipage-install-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data_dir);
    std::fs::create_dir_all(&data_dir).unwrap();

    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(DummyExt)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let mut config = Config::default();
    config.server.data_dir = data_dir.clone();
    let state = AppState {
        db: pool.clone(),
        config: Arc::new(config),
        admin_token: Some(Arc::from("test-admin-token")),
        registry,
                wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    let app = oxipage_core::http::build_app(state);

    // 하이픈이 포함된 이름("wasm-demo")이 is_safe_extension_name 을 통과하는지 검증.
    let req = Request::builder()
        .method("POST")
        .uri("/api/console/extensions/install")
        .header("authorization", "Bearer test-admin-token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"wasm-demo"}"#))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "install should succeed");
    let body = body_string(res).await;
    assert!(body.contains("wasm-demo"), "body: {body}");
    assert!(body.contains("\"bytes\""), "body should report byte count: {body}");

    // 1. .wasm 파일이 data/extensions/<name>.wasm 에 쓰였는지.
    let wasm_path = data_dir.join("extensions").join("wasm-demo.wasm");
    assert!(wasm_path.exists(), "wasm artifact should be written at {}", wasm_path.display());
    let meta = std::fs::metadata(&wasm_path).unwrap();
    assert!(meta.len() > 100, "wasm artifact non-trivial size, got {}", meta.len());

    // 2. extension_state 행이 enabled=0 으로 추가됐는지 (부팅 시 적재 대기).
    let (enabled,): (i64,) =
        sqlx::query_as("SELECT enabled FROM extension_state WHERE extension_id = ?")
            .bind("wasm-demo")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        enabled, 0,
        "newly installed wasm ext should be disabled (enabled=0) until restart+enable"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}

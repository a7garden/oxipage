use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use hmac::{Hmac, Mac};
use oxibuilder_core::config::Config;
use oxibuilder_core::extension::Extension;
use oxibuilder_core::registry::ExtensionRegistry;
use oxibuilder_core::state::{AppState, SiteScopedDb};
use oxibuilder_ext_activity::ActivityExtension;
use sha2::Sha256;
use std::sync::Arc;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

const TEST_SECRET: &str = "test-webhook-secret";

async fn test_app(_admin_token: Option<&str>) -> Router {
    // webhook 핸들러가 환경변수에서 시크릿을 읽으므로 테스트 시작 시 설정.
    // SAFETY: 단일 스레드 테스트 시작 시점, 다른 스레드가 env를 읽지 않음.
    unsafe { std::env::set_var("OXIBUILDER_GITHUB_WEBHOOK_SECRET", TEST_SECRET) };
    let pool = oxibuilder_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ActivityExtension)]));
    registry.run_migrations(&pool, &[]).await.unwrap();
    let _state = AppState {
        db: pool.clone(),
        config: Arc::new(Config::default()),
        registry: registry.clone(),
        wasm_loader: None,
        site_override: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        builders: std::sync::Arc::new(vec![]),
    };
    Router::new()
        .nest(
            "/api/console/activity",
            registry.find("activity").unwrap().routes(),
        )
        .layer(axum::extract::Extension(SiteScopedDb {
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

fn push_payload(id: &str, repo: &str, occurred_at: &str) -> String {
    serde_json::json!({
        "id": id,
        "type": "PushEvent",
        "repo": { "name": repo, "url": format!("https://api.github.com/repos/{repo}") },
        "created_at": occurred_at,
        "payload": { "html_url": format!("https://github.com/{repo}/commits/main") }
    })
    .to_string()
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let bytes = mac.finalize().into_bytes();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256={hex}")
}

/// 유효한 HMAC 서명을 포함해 webhook을 호출한다.
async fn webhook(app: &Router, payload: String) -> axum::response::Response {
    let sig = sign(TEST_SECRET, payload.as_bytes());
    app.clone()
        .oneshot(
            Request::post("/api/console/activity/webhook")
                .header("content-type", "application/json")
                .header("x-hub-signature-256", sig)
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn webhook_upserts_public_event_and_duplicate_remains_single() {
    let app = test_app(Some("tok")).await;
    let payload = push_payload("event-1", "owner/repo", "2026-07-27T12:00:00Z");
    assert_eq!(
        webhook(&app, payload.clone()).await.status(),
        StatusCode::OK
    );
    assert_eq!(webhook(&app, payload).await.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::get("/api/console/activity")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    let events = json["data"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["repo_full_name"], "owner/repo");
    assert_eq!(events[0]["event_type"], "push");
    assert_eq!(events[0]["summary"], "push to owner/repo");
}

#[tokio::test]
async fn list_honors_limit_order_and_repo_filter() {
    let app = test_app(Some("tok")).await;
    for (id, repo, at) in [
        ("1", "owner/one", "2026-07-27T10:00:00Z"),
        ("2", "owner/two", "2026-07-27T11:00:00Z"),
        ("3", "owner/one", "2026-07-27T12:00:00Z"),
    ] {
        assert_eq!(
            webhook(&app, push_payload(id, repo, at)).await.status(),
            StatusCode::OK
        );
    }

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/console/activity?limit=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
    assert_eq!(json["data"][0]["occurred_at"], "2026-07-27T12:00:00Z");

    let res = app
        .oneshot(
            Request::get("/api/console/activity?repo=owner%2Fone&limit=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}
#[tokio::test]
async fn malformed_webhook_is_422() {
    let app = test_app(Some("tok")).await;
    // 유효한 서명 + 잘못된 JSON 본문 → 서명 통과 후 validation에서 422.
    let payload = serde_json::json!({"type":"PushEvent"}).to_string();
    let res = webhook(&app, payload).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn webhook_without_signature_is_401() {
    let app = test_app(Some("tok")).await;
    let payload = push_payload("event-x", "owner/repo", "2026-07-27T12:00:00Z");
    let res = app
        .oneshot(
            Request::post("/api/console/activity/webhook")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn webhook_with_bad_signature_is_401() {
    let app = test_app(Some("tok")).await;
    let payload = push_payload("event-y", "owner/repo", "2026-07-27T12:00:00Z");
    let bad_sig = sign("wrong-secret", payload.as_bytes());
    let res = app
        .oneshot(
            Request::post("/api/console/activity/webhook")
                .header("content-type", "application/json")
                .header("x-hub-signature-256", bad_sig)
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn extension_registers_quarter_hour_sync_job() {
    let jobs = ActivityExtension.background_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].name(), "activity_sync");
}

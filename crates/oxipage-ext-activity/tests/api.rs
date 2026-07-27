use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use oxipage_core::config::Config;
use oxipage_core::extension::Extension;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::state::AppState;
use oxipage_ext_activity::ActivityExtension;
use std::sync::Arc;
use tower::ServiceExt;

async fn test_app(admin_token: Option<&str>) -> Router {
    let pool = oxipage_core::db::connect_memory().await.unwrap();
    let registry = Arc::new(ExtensionRegistry::new(vec![Arc::new(ActivityExtension)]));
    registry.run_migrations(&pool).await.unwrap();
    let state = AppState {
        db: pool,
        config: Arc::new(Config::default()),
        admin_token: admin_token.map(Arc::<str>::from),
        registry: registry.clone(),
    };
    Router::new()
        .nest(
            "/api/v1/activity",
            registry.find("activity").unwrap().routes(),
        )
        .with_state(state)
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

async fn webhook(app: &Router, payload: String) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post("/api/v1/activity/webhook")
                .header("content-type", "application/json")
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
            Request::get("/api/v1/activity")
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
            Request::get("/api/v1/activity?limit=2")
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
            Request::get("/api/v1/activity?repo=owner%2Fone&limit=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json = body_json(res).await;
    assert_eq!(json["data"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn sync_without_token_is_401() {
    let app = test_app(Some("tok")).await;
    let res = app
        .oneshot(
            Request::post("/api/v1/activity/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn malformed_webhook_is_422() {
    let app = test_app(Some("tok")).await;
    let res = webhook(&app, serde_json::json!({"type":"PushEvent"}).to_string()).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn extension_registers_quarter_hour_sync_job() {
    let jobs = ActivityExtension.background_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].name(), "activity_sync");
    assert_eq!(jobs[0].schedule(), "0 */15 * * * *");
}

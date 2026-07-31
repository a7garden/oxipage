use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use tower::util::ServiceExt;

#[tokio::test]
async fn get_default_theme_with_no_registered_site_returns_paper() {
    // Empty registry — no default slug, no sites. Handler must NOT hit DB.
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(Request::builder().uri("/theme").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["data"]["theme_id"], "paper");
    assert_eq!(json["data"]["definition"]["id"], "paper");
    assert_eq!(json["data"]["definition"]["accent_hue"], 160.0);
}

#[tokio::test]
async fn get_default_theme_404s_for_unknown_route_after_move() {
    // After moving, GET /theme/extra in the console router should 404.
    // This is just a guard against double-mounting.
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/theme/extra")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

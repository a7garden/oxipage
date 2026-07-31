use axum::body::Body;
use axum::http::{Request, StatusCode};
use oxipage_console::router::build_console_router;
use oxipage_console::sites_runtime::SiteRegistry;
use tower::util::ServiceExt;

#[tokio::test]
async fn per_site_theme_get_unknown_slug_404s() {
    let registry = SiteRegistry::empty_for_tests().await;
    let app = build_console_router(registry);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/s/nonexistent/theme")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

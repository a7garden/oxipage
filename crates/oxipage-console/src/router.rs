//! Console router — builds the Axum router tree with site-prefixed routes.

use crate::middleware::site_db::inject_site_context;
use crate::sites_runtime::SiteRegistry;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

/// Build the console router tree with site CRUD + per-slug build/deploy.
pub fn build_console_router(registry: Arc<SiteRegistry>) -> Router {
    let mut api = Router::new()
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/default", get(get_default).put(set_default));

    for (_slug, ctx) in registry.iter_blocking() {
        let scoped = Router::new()
            .route("/build", post(build_handler))
            .route("/deploy", post(deploy_handler))
            .layer(axum::middleware::from_fn_with_state(
                ctx.clone(),
                inject_site_context,
            ));

        api = api.nest(&format!("/s/{}", ctx.slug), scoped);
    }

    api
}

// ─── Site CRUD stubs (full impl in T5/T6) ───

async fn list_sites() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": [] }))
}

async fn create_site() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "ok": true } }))
}

async fn get_default() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "default_site": null } }))
}

async fn set_default() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "ok": true } }))
}

async fn build_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "ok": true } }))
}

async fn deploy_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "ok": true } }))
}

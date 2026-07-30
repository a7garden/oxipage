//! Console router — top-level routes (CRUD + build/deploy/preview) plus
//! per-site extension routes. Per-site routes use middleware-injected
//! SiteScopedDb.

use crate::build::site_build;
use crate::create_site::create_site_handler;
use crate::deploy::site_deploy;
use crate::middleware::site_db::inject_site_context;
use crate::preview::handler::preview_handler;
use crate::sites_runtime::SiteRegistry;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

/// Build the top-level console routes. Returns `Router<Arc<SiteRegistry>>`
/// without baking state — caller passes the registry once.
pub fn build_top_level_router() -> Router<Arc<SiteRegistry>> {
    Router::new()
        .route("/sites", get(list_sites))
        .route("/sites/default", get(get_default).put(set_default))
        .route("/setup/create-site", post(create_site_handler))
        .route("/build/{slug}", post(site_build::build_handler))
        .route("/deploy/{slug}", post(site_deploy::deploy_handler))
        .route("/preview/{slug}/{*rest}", get(preview_handler))
}

/// Per-site extension nests. Returns `Router<()>`. Handlers use
/// `Extension<SiteScopedDb>` injected by middleware.
pub fn build_per_site_router(registry: &Arc<SiteRegistry>) -> Router {
    let mut api = Router::new();
    for (_slug, ctx) in registry.iter_blocking() {
        let mut nested = Router::new();
        for ext in ctx.registry.iter() {
            if ext.route_dispatcher().is_some() {
                continue;
            }
            nested = nested.nest(&format!("/{}", ext.id()), ext.routes());
        }
        let scoped = nested.layer(axum::middleware::from_fn_with_state(
            ctx.clone(),
            inject_site_context,
        ));
        api = api.nest(&format!("/s/{}", ctx.slug), scoped);
    }
    api
}

/// Full console router. Returns `Router<()>` after baking state.
pub fn build_console_router(registry: Arc<SiteRegistry>) -> Router {
    let per_site = build_per_site_router(&registry);
    let top = build_top_level_router().with_state(registry);
    top.merge(per_site)
}

// ─── Site CRUD stubs ───

async fn list_sites(State(_registry): State<Arc<SiteRegistry>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": [] }))
}

async fn get_default(State(_registry): State<Arc<SiteRegistry>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "default_site": null } }))
}

async fn set_default() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "data": { "ok": true } }))
}

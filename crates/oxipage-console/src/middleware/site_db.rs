//! Middleware that injects per-site context into request extensions.
//!
//! Applied as a layer on each per-slug route nest. The `Arc<SiteContext>`
//! is passed via state (not extracted from path), since the slug is hard-coded
//! into the nest URL at startup.

use crate::sites_runtime::{SiteContext, SiteScopedDb};
use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

/// Inject `SiteScopedDb` + `Arc<SiteContext>` into request extensions
/// for the current site. State is the pre-resolved `SiteContext`.
pub async fn inject_site_context(
    State(ctx): State<Arc<SiteContext>>,
    mut req: Request,
    next: Next,
) -> Response {
    req.extensions_mut().insert(SiteScopedDb { db: ctx.db.clone() });
    req.extensions_mut().insert(ctx);
    next.run(req).await
}

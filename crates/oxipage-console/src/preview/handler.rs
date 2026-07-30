//! `GET /api/console/preview/{slug}/*` — serve one site's `out/` build statically.
//!
//! Used by the admin SPA's preview iframe to verify build output without
//! running a separate static server. The slug is the registered site; the
//! wildcard is the path inside `<site_path>/out/` (e.g. `/index.html`,
//! `/blog/post-slug/index.html`).

use crate::sites_runtime::SiteRegistry;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, Response, StatusCode};
use axum::routing::get;
use axum::Router;
use std::path::PathBuf;
use std::sync::Arc;

pub fn router() -> Router<Arc<SiteRegistry>> {
    Router::new().route("/preview/:slug/*rest", get(preview_handler))
}

pub(crate) async fn preview_handler(
    State(registry): State<Arc<SiteRegistry>>,
    Path((slug, rest)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let ctx = registry
        .ctx_for(&slug)
        .await
        .ok_or((StatusCode::NOT_FOUND, "site_not_found".to_string()))?;

    // Strip leading slash from `rest`, then resolve safely inside out/.
    let clean = rest.trim_start_matches('/');
    let mut file_path = PathBuf::from(&ctx.path).join("out");
    for component in clean.split('/').filter(|c| !c.is_empty()) {
        if matches!(component, "." | "..") {
            return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
        }
        file_path.push(component);
    }

    let bytes = std::fs::read(&file_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            (StatusCode::NOT_FOUND, "preview_file_not_found".into())
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .body(Body::from(bytes))
        .unwrap())
}

//! Media upload and live serving — `/api/console/s/{slug}/media/...`.
//!
//! Spec §7–9. Upload accepts a single `file` field via multipart, validates
//! by magic bytes (not declared Content-Type), chooses extension from the
//! detected MIME, and writes the file atomically to
//! `<media_dir>/<extension>/<uuid>.<ext>`.
//!
//! Live serving is a thin static handler that precedes the Admin SPA
//! fallback so a `/media/...` URL always returns bytes, never `admin.html`.

pub mod serve;
pub mod upload;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

/// Mount the media routes under the per-site nest. Caller wraps with
/// `site_db` middleware so handlers can extract `Extension<Arc<SiteContext>>`.
///
/// The upload route raises axum's default 2 MiB body limit above the 10 MiB
/// handler cap so the handler's own byte-counter (not the extractor) is the
/// authoritative rejection point for oversized files.
pub fn router() -> Router {
    Router::new()
        .route(
            "/media/{extension}",
            post(upload::upload_handler).layer(DefaultBodyLimit::max(12 * 1024 * 1024)),
        )
        .route(
            "/media/{extension}/{file}",
            get(serve::serve_handler).head(serve::serve_handler),
        )
}

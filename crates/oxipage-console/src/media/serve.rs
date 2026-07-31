//! Live serving of uploaded media. Spec §9 (live serving).
//!
//! Reads are lock-free. Unix `rename(2)` is atomic, so a reader always sees
//! either the old path or the fully-written new path — never a partial file.
//! The upload handler writes to a `.tmp` sibling and atomically renames into
//! place, so no cross-handler synchronization is required here.

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;
use std::path::{Component, PathBuf};
use std::sync::Arc;

pub async fn serve_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path((extension, file)): Path<(String, String)>,
) -> Result<Response<Body>, Response<Body>> {
    let mut candidate = PathBuf::from(&ctx.media_dir);
    for component in std::path::Path::new(&extension).components() {
        match component {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return Err(StatusCode::BAD_REQUEST.into_response());
                }
                candidate.push(s.as_ref());
            }
            _ => return Err(StatusCode::BAD_REQUEST.into_response()),
        }
    }
    for component in std::path::Path::new(&file).components() {
        match component {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return Err(StatusCode::BAD_REQUEST.into_response());
                }
                candidate.push(s.as_ref());
            }
            _ => return Err(StatusCode::BAD_REQUEST.into_response()),
        }
    }

    let meta = match tokio::fs::metadata(&candidate).await {
        Ok(m) if m.is_file() => m,
        _ => return Err(StatusCode::NOT_FOUND.into_response()),
    };

    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if !canonical_candidate.starts_with(&canonical_media) {
        return Err(StatusCode::BAD_REQUEST.into_response());
    }

    let bytes = match tokio::fs::read(&candidate).await {
        Ok(b) => b,
        Err(_) => return Err(StatusCode::NOT_FOUND.into_response()),
    };

    let mime = mime_guess::from_path(&candidate).first_or_octet_stream();
    let len = meta.len();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .header(header::CONTENT_LENGTH, len)
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes))
        .unwrap())
}

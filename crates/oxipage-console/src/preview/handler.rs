//! `GET /api/console/preview/{slug}/{*rest}` — serve one site's `out/` build.
//!
//! Three routes are mounted by `router.rs::build_top_level_router`:
//!   - `/api/console/preview/{slug}`          → `redirect_to_slash`  (307)
//!   - `/api/console/preview/{slug}/`         → `preview_trailing`   (serve root)
//!   - `/api/console/preview/{slug}/{*rest}`  → `preview_handler`    (catch-all)
//!
//! axum 0.8 catch-all routes (`{*rest}`) do NOT match a bare trailing slash
//! (`/preview/{slug}/`), so the trailing-slash route exists separately and
//! forwards an empty rest to the same resolution logic. The bare-slug route
//! (no slash at all) 307-redirects to the canonical trailing-slash URL.
//!
//! Resolution rules (spec §6):
//!   empty path             → out/index.html
//!   directory path         → <dir>/index.html
//!   existing file          → exact file
//!   missing client route   → out/404.html
//!   missing build/manifest → 424 build_required
//!
//! For HTML responses, the generated `<base href>` is rewritten to the
//! preview prefix so the bundled SPA resolves its relative `assets/...` tags
//! correctly. The persisted manifest's `deployment_base` is shipped as the
//! artifact's canonical base; the preview operates under a different
//! (longer) URL prefix at request time.
//!
//! All response bodies are served with `Cache-Control: no-store` and
//! `X-Content-Type-Options: nosniff`. No directory listing.

use crate::sites_runtime::SiteRegistry;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Response, StatusCode, header};
use oxipage_core::build_manifest::BuildManifest;
use std::path::{Component, PathBuf};
use std::sync::Arc;

/// `GET /api/console/preview/{slug}` → 307 to `/api/console/preview/{slug}/`.
/// Mounted directly in `router.rs::build_top_level_router`; the handler file
/// owns no routing surface (the file is named `handler.rs`, not `router.rs`).
pub async fn redirect_to_slash(
    State(_registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Response<Body> {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, format!("/api/console/preview/{slug}/"))
        .body(Body::empty())
        .unwrap()
}

/// `GET /api/console/preview/{slug}/` — serve the build root. Exists because
/// axum 0.8 catch-all routes don't match a trailing-slash-only path.
pub async fn preview_trailing(
    State(registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    serve_preview(&registry, &slug, "").await
}

pub(crate) async fn preview_handler(
    State(registry): State<Arc<SiteRegistry>>,
    Path((slug, rest)): Path<(String, String)>,
) -> Result<Response<Body>, (StatusCode, String)> {
    serve_preview(&registry, &slug, &rest).await
}

/// Shared resolution + serving logic. `rest` is the path inside `out/`
/// (already stripped of a leading slash; empty for the root).
async fn serve_preview(
    registry: &Arc<SiteRegistry>,
    slug: &str,
    rest: &str,
) -> Result<Response<Body>, (StatusCode, String)> {
    let ctx = registry
        .ctx_for(slug)
        .await
        .ok_or((StatusCode::NOT_FOUND, "site_not_found".to_string()))?;

    let out_dir = &ctx.out_dir;

    // Manifest gate — spec §5: missing manifest means no build has run.
    let manifest = BuildManifest::read_from(out_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let manifest = match manifest {
        Some(m) => m,
        None => return Err((StatusCode::FAILED_DEPENDENCY, "build_required".into())),
    };

    // Build the candidate path with traversal guards.
    let clean = rest.trim_start_matches('/');
    let mut candidate = PathBuf::from(out_dir);
    let mut has_segments = false;
    for component in std::path::Path::new(clean).components() {
        match component {
            Component::Normal(seg) => {
                let seg_str = seg.to_string_lossy();
                if seg_str.is_empty() || seg_str == "." || seg_str == ".." {
                    return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
                }
                has_segments = true;
                candidate.push(seg_str.as_ref());
            }
            Component::CurDir | Component::ParentDir => {
                return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err((StatusCode::BAD_REQUEST, "path_anchor".into()));
            }
        }
    }

    // 1. exact file
    let resolved = if !has_segments {
        out_dir.join("index.html")
    } else if candidate.is_file() {
        candidate
    } else if candidate.is_dir() {
        // 2. directory index
        candidate.join("index.html")
    } else if looks_like_client_route(&candidate) {
        // 3. SPA fallback
        out_dir.join("404.html")
    } else {
        // 4. static asset / data / media that's missing → real 404
        return Err((StatusCode::NOT_FOUND, "preview_not_found".into()));
    };

    if !resolved.is_file() {
        return Err((StatusCode::NOT_FOUND, "preview_not_found".into()));
    }

    // Containment check — even after component filtering, confirm the resolved
    // path is inside out_dir. Catches symlink races and platform quirks.
    let canonical_out = out_dir.canonicalize().unwrap_or_else(|_| out_dir.clone());
    let canonical_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());
    if !canonical_resolved.starts_with(&canonical_out) {
        return Err((StatusCode::BAD_REQUEST, "path_traversal".into()));
    }

    let bytes =
        std::fs::read(&resolved).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mime = mime_guess::from_path(&resolved).first_or_octet_stream();
    let is_html =
        mime == "text/html" || resolved.extension().and_then(|s| s.to_str()) == Some("html");

    let body = if is_html {
        // The manifest's deployment_base is the artifact's canonical base
        // (e.g. `/repo/`). The preview serves at a different URL prefix — we
        // override the `<base href>` so the SPA's relative `assets/...` tags
        // resolve against the live preview URL.
        let preview_base = preview_base_href(slug);
        rewrite_base_href(&bytes, &preview_base)
    } else {
        bytes
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.to_string())
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");

    if is_html {
        builder = builder
            .header("X-Oxipage-Build-Id", &manifest.build_id)
            .header("X-Oxipage-Build-Theme", &manifest.theme_id)
            .header("X-Oxipage-Build-Asset-Revision", &manifest.asset_revision)
            .header("X-Oxipage-Build-Deployment-Base", &manifest.deployment_base);
    }

    Ok(builder.body(Body::from(body)).unwrap())
}

/// Build a preview-prefix base href from the slug, ensuring it ends with `/`.
/// Equivalent to `/api/console/preview/{slug}/`.
fn preview_base_href(slug: &str) -> String {
    format!("/api/console/preview/{slug}/")
}

/// Decide whether a missing path should fall back to 404.html.
fn looks_like_client_route(candidate: &std::path::Path) -> bool {
    let ext = candidate.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ext.is_empty() {
        return true;
    }
    matches!(ext, "html")
}

/// Replace the `<base href="...">` in the persisted HTML with the
/// per-request preview base. The persisted HTML's `<base href>` is the
/// manifest's `deployment_base` (the artifact's canonical base); we override
/// it for the preview URL.
///
/// The whole `<base ...>` tag is replaced (up to its closing `>`), so no
/// stray characters leak from the original tag. Falls back to inserting a
/// `<base>` if the materialized HTML is missing one (older builds).
fn rewrite_base_href(html: &[u8], preview_base: &str) -> Vec<u8> {
    let haystack = String::from_utf8_lossy(html);
    let replacement = format!("<base href=\"{preview_base}\">");
    const OPEN: &str = "<base href=\"";
    if let Some(start) = haystack.find(OPEN) {
        // Find the closing `>` of the original <base> tag.
        if let Some(tag_end_rel) = haystack[start..].find('>') {
            let tag_end = start + tag_end_rel;
            let mut out = String::with_capacity(haystack.len() + preview_base.len());
            out.push_str(&haystack[..start]);
            out.push_str(&replacement);
            out.push_str(&haystack[tag_end + 1..]);
            return out.into_bytes();
        }
        return html.to_vec();
    }
    // No <base> in the file — inject one at the start of <head>.
    if let Some(idx) = haystack.find("<head>") {
        let mut out = String::with_capacity(haystack.len() + replacement.len() + 8);
        out.push_str(&haystack[..idx + "<head>".len()]);
        out.push('\n');
        out.push_str("    ");
        out.push_str(&replacement);
        out.push_str(&haystack[idx + "<head>".len()..]);
        return out.into_bytes();
    }
    html.to_vec()
}

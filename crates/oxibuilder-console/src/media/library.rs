//! Media library: enumerate + delete uploaded media (spec §5.3).
//!
//! Reuses the same component-normal + canonicalize-under-`media_dir`
//! containment checks as `serve.rs`. List walks `<media_dir>/<extension>/<file>`
//! one level deep; delete removes a single file. There is no reference
//! tracking — the 1-person-owner security model (doc §0.3) makes manual delete
//! with a UI confirmation sufficient.

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::path::{Component, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Serialize)]
pub struct MediaItem {
    pub path: String,
    pub extension: String,
    pub file: String,
    pub mime: String,
    pub bytes: u64,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ListResponse {
    pub data: Vec<MediaItem>,
}

#[derive(serde::Deserialize, Default)]
pub struct ListQuery {
    pub extension: Option<String>,
}

/// Validate a single path segment the same way `serve.rs` does: reject empty,
/// `.` and `..`, and any value that decomposes into more than one component
/// (path separators, including percent-decoded forms already resolved by axum).
fn safe_segment(seg: &str) -> Option<String> {
    let s = seg.trim();
    if s.is_empty() || s == "." || s == ".." {
        return None;
    }
    for component in std::path::Path::new(s).components() {
        match component {
            Component::Normal(p) => {
                if p.is_empty() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(s.to_string())
}

pub async fn list_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse>, StatusCode> {
    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());

    // Collect the extension directories to scan: either the filtered one, or
    // every immediate child directory of media_dir.
    let extension_dirs: Vec<(String, PathBuf)> = match &q.extension {
        Some(ext) => {
            let Some(ext) = safe_segment(ext) else {
                return Err(StatusCode::BAD_REQUEST);
            };
            vec![(ext.clone(), ctx.media_dir.join(&ext))]
        }
        None => match tokio::fs::read_dir(&ctx.media_dir).await {
            Ok(mut rd) => {
                let mut v = Vec::new();
                while let Ok(Some(entry)) = rd.next_entry().await {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        v.push((name, p));
                    }
                }
                v
            }
            Err(_) => return Ok(Json(ListResponse { data: Vec::new() })),
        },
    };

    let mut items: Vec<(SystemTime, MediaItem)> = Vec::new();
    for (extension, ext_dir) in extension_dirs {
        let mut rd = match tokio::fs::read_dir(&ext_dir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            let meta = match tokio::fs::metadata(&path).await {
                Ok(m) if m.is_file() => m,
                _ => continue,
            };
            // Containment: canonicalize must stay under media_dir.
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !canon.starts_with(&canonical_media) {
                continue;
            }
            let file = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            let mtime = meta.modified().unwrap_or_else(|_| SystemTime::now());
            items.push((
                mtime,
                MediaItem {
                    path: format!("media/{extension}/{file}"),
                    extension: extension.clone(),
                    file,
                    mime,
                    bytes: meta.len(),
                    updated_at: crate::operations::system_time_iso(mtime),
                },
            ));
        }
    }
    // Newest first.
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let data = items.into_iter().map(|(_, item)| item).collect();
    Ok(Json(ListResponse { data }))
}

pub async fn delete_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path((extension, file)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut candidate = PathBuf::from(&ctx.media_dir);
    for seg in [&extension, &file] {
        let Some(seg) = safe_segment(seg) else {
            return Err(StatusCode::BAD_REQUEST);
        };
        candidate.push(seg);
    }
    let canonical_media = ctx
        .media_dir
        .canonicalize()
        .unwrap_or_else(|_| ctx.media_dir.clone());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if !canonical_candidate.starts_with(&canonical_media) {
        return Err(StatusCode::BAD_REQUEST);
    }
    match tokio::fs::remove_file(&candidate).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

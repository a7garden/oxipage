//! Multipart upload for site media. Spec §9.

use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::{Multipart, Path};
use axum::http::StatusCode;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

/// 10 MiB hard cap per spec §9. Applied against the running byte counter
/// during streaming so users with a 100 MB file see an early rejection.
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Serialize)]
pub struct UploadResponse {
    pub data: UploadData,
}

#[derive(Serialize)]
pub struct UploadData {
    pub path: String,
    pub mime: &'static str,
    pub bytes: u64,
}

/// Image format detected by reading the first 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
}

impl DetectedFormat {
    fn ext(self) -> &'static str {
        match self {
            DetectedFormat::Jpeg => "jpg",
            DetectedFormat::Png => "png",
            DetectedFormat::WebP => "webp",
            DetectedFormat::Gif => "gif",
        }
    }
    fn mime(self) -> &'static str {
        match self {
            DetectedFormat::Jpeg => "image/jpeg",
            DetectedFormat::Png => "image/png",
            DetectedFormat::WebP => "image/webp",
            DetectedFormat::Gif => "image/gif",
        }
    }
}

fn detect_format(head: &[u8]) -> Option<DetectedFormat> {
    if head.len() >= 3 && head[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(DetectedFormat::Jpeg);
    }
    if head.len() >= 8 && head[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(DetectedFormat::Png);
    }
    if head.len() >= 6 && (&head[..6] == b"GIF87a" || &head[..6] == b"GIF89a") {
        return Some(DetectedFormat::Gif);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(DetectedFormat::WebP);
    }
    None
}

/// Reject path separators and any non-alnum/underscore/hyphen so the
/// extension id can't be used as a path component.
fn is_safe_extension_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn media_dir_for(ctx: &SiteContext, extension: &str) -> std::io::Result<PathBuf> {
    let dir = ctx.media_dir.join(extension);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub async fn upload_handler(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(extension): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    if !is_safe_extension_id(&extension) {
        return Err((StatusCode::BAD_REQUEST, "invalid_extension_id".into()));
    }

    let dest_dir = media_dir_for(&ctx, &extension)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("mkdir: {e}")))?;

    // Hyphenated UUID form for both filename and URL.
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.hyphenated().to_string();
    let tmp_path = dest_dir.join(format!("{uuid_str}.tmp"));
    let mut tmp = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("tmp create: {e}"),
        )
    })?;

    let mut head: Vec<u8> = Vec::with_capacity(16);
    let mut total: u64 = 0;
    let mut detected: Option<DetectedFormat> = None;

    // Stream the single `file` field. The field is consumed (chunked) INSIDE
    // the outer loop because axum 0.8 `Field` borrows `Multipart` — it cannot
    // be stored across a later `next_field()` call.
    let mut found_file = false;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        found_file = true;
        let mut field = field;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("multipart: {e}")))?
        {
            total = total.saturating_add(chunk.len() as u64);
            if total > MAX_FILE_BYTES as u64 {
                drop(tmp);
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "file_too_large".into()));
            }
            if detected.is_none() && head.len() < 16 {
                let need = 16 - head.len();
                let take = need.min(chunk.len());
                head.extend_from_slice(&chunk[..take]);
                detected = detect_format(&head);
            }
            tmp.write_all(&chunk)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {e}")))?;
        }
        break;
    }

    if !found_file {
        drop(tmp);
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err((StatusCode::BAD_REQUEST, "missing_file_field".into()));
    }

    let format = match detected {
        Some(f) => f,
        None => {
            drop(tmp);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_image_format".into(),
            ));
        }
    };

    tmp.flush()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("flush: {e}")))?;
    drop(tmp);

    let final_path = dest_dir.join(format!("{uuid_str}.{}", format.ext()));
    tokio::fs::rename(&tmp_path, &final_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")))?;

    let response_path = format!("media/{extension}/{uuid_str}.{}", format.ext());
    Ok(Json(UploadResponse {
        data: UploadData {
            path: response_path,
            mime: format.mime(),
            bytes: total,
        },
    }))
}

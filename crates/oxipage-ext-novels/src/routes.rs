use crate::model::{ChapterInput, ChapterOrderInput, ChapterPatch, ListQuery, Novel, NovelChapter, NovelInput};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::search;
use oxipage_core::state::SiteScopedDb;

// ─── Novel ───

pub async fn list_novels(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<Novel>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let novels = repo::list_novels(&pool.db, q.draft, limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: novels }))
}

pub async fn create_novel(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<NovelInput>,
) -> Result<Json<DataEnvelope<Novel>>, ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if !matches!(input.status.as_str(), "ongoing" | "completed" | "hiatus") {
        return Err(ApiError::validation(
            "status",
            "status must be ongoing|completed|hiatus",
        ));
    }
    let base_slug = input
        .slug
        .clone()
        .unwrap_or_else(|| repo::slugify(&input.title));
    let slug = repo::ensure_unique_slug(&pool.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;
    let novel = repo::create_novel(&pool.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: novel }))
}

pub async fn show_novel(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Novel>>, ApiError> {
    let novel = repo::find_novel_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    if novel.published_at.is_none() {
        return Err(not_found(&slug));
    }
    Ok(Json(DataEnvelope { data: novel }))
}

pub async fn delete_novel(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_novel(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&pool.db, "novels", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish_novel(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Novel>>, ApiError> {
    if repo::find_novel_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let novel = repo::publish_novel(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    search::upsert(
        &pool.db,
        "novels",
        &novel.slug,
        &novel.title,
        novel.synopsis.as_deref().unwrap_or(""),
        None,
        novel.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)?;
    let synopsis = novel.synopsis.clone().unwrap_or_default();
    let _desc: String = synopsis.chars().take(200).collect();
    Ok(Json(DataEnvelope { data: novel }))
}

// ─── Chapter ───

pub async fn list_chapters(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    let chapters = repo::list_chapters(&pool.db, &slug, false)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: chapters }))
}

pub async fn list_chapters_draft(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    let chapters = repo::list_chapters(&pool.db, &slug, true)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: chapters }))
}

pub async fn create_chapter(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<ChapterInput>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    let ch = repo::create_chapter(&pool.db, &slug, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: ch }))
}

pub async fn show_chapter(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::find_chapter(&pool.db, &slug, order)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&format!("{slug}/{order}")))?;
    if ch.published_at.is_none() {
        return Err(not_found(&format!("{slug}/{order}")));
    }
    Ok(Json(DataEnvelope { data: ch }))
}

pub async fn update_chapter(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, order)): Path<(String, i32)>,
    Json(patch): Json<ChapterPatch>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::update_chapter(&pool.db, &slug, order, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&format!("{slug}/{order}")))?;
    if ch.published_at.is_some() {
        let doc_id = format!("{slug}/chapters/{}", ch.chapter_order);
        search::upsert(
            &pool.db,
            "novels",
            &doc_id,
            &ch.title,
            &ch.body,
            None,
            ch.published_at.as_deref(),
        )
        .await
        .map_err(ApiError::internal)?;
    }
    Ok(Json(DataEnvelope { data: ch }))
}

pub async fn delete_chapter(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_chapter(&pool.db, &slug, order)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&format!("{slug}/{order}")));
    }
    let doc_id = format!("{slug}/chapters/{order}");
    search::delete(&pool.db, "novels", &doc_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "deleted": true }),
    }))
}

pub async fn publish_chapter(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::publish_chapter(&pool.db, &slug, order)
        .await
        .map_err(ApiError::internal)?;
    let doc_id = format!("{slug}/chapters/{}", ch.chapter_order);
    search::upsert(
        &pool.db,
        "novels",
        &doc_id,
        &ch.title,
        &ch.body,
        None,
        ch.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)?;
    let _preview: String = ch.body.chars().take(200).collect();
    Ok(Json(DataEnvelope { data: ch }))
}

fn not_found(s: &str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("novel/chapter '{s}' not found"),
    )
}

pub async fn reorder_chapters(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<ChapterOrderInput>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    if input
        .chapter_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != input.chapter_ids.len()
    {
        return Err(ApiError::validation(
            "chapter_ids",
            "chapter_ids contains duplicates",
        ));
    }
    let chapters = repo::reorder_chapters(&pool.db, &slug, &input.chapter_ids).await.map_err(
        |e| {
            let msg = e.to_string();
            if msg.starts_with("stale_order") {
                ApiError::new(
                    axum::http::StatusCode::CONFLICT,
                    "stale_order",
                    "submitted IDs do not match current chapter set",
                )
            } else {
                ApiError::internal(e)
            }
        },
    )?;
    Ok(Json(DataEnvelope { data: chapters }))
}

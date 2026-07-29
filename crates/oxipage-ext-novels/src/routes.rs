use crate::model::{ChapterInput, ChapterPatch, ListQuery, Novel, NovelChapter, NovelInput};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::search;
use oxipage_core::state::AppState;

// ─── Novel ───

pub async fn list_novels(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<Novel>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let novels = repo::list_novels(&state.db, q.draft, limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: novels }))
}

pub async fn create_novel(
    State(state): State<AppState>,
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
    let slug = repo::ensure_unique_slug(&state.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;
    let novel = repo::create_novel(&state.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: novel }))
}

pub async fn show_novel(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Novel>>, ApiError> {
    let novel = repo::find_novel_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    if novel.published_at.is_none() {
        return Err(not_found(&slug));
    }
    Ok(Json(DataEnvelope { data: novel }))
}

pub async fn delete_novel(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_novel(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&state.db, "novels", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish_novel(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Novel>>, ApiError> {
    if repo::find_novel_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let novel = repo::publish_novel(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    search::upsert(
        &state.db,
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
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    let chapters = repo::list_chapters(&state.db, &slug, false)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: chapters }))
}

pub async fn list_chapters_draft(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<Vec<NovelChapter>>>, ApiError> {
    let chapters = repo::list_chapters(&state.db, &slug, true)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: chapters }))
}

pub async fn create_chapter(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<ChapterInput>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    let ch = repo::create_chapter(&state.db, &slug, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: ch }))
}

pub async fn show_chapter(
    State(state): State<AppState>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::find_chapter(&state.db, &slug, order)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&format!("{slug}/{order}")))?;
    if ch.published_at.is_none() {
        return Err(not_found(&format!("{slug}/{order}")));
    }
    Ok(Json(DataEnvelope { data: ch }))
}

pub async fn update_chapter(
    State(state): State<AppState>,
    Path((slug, order)): Path<(String, i32)>,
    Json(patch): Json<ChapterPatch>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::update_chapter(&state.db, &slug, order, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&format!("{slug}/{order}")))?;
    if ch.published_at.is_some() {
        let doc_id = format!("{slug}/chapters/{}", ch.chapter_order);
        search::upsert(
            &state.db,
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
    State(state): State<AppState>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_chapter(&state.db, &slug, order)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&format!("{slug}/{order}")));
    }
    let doc_id = format!("{slug}/chapters/{order}");
    search::delete(&state.db, "novels", &doc_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "deleted": true }),
    }))
}

pub async fn publish_chapter(
    State(state): State<AppState>,
    Path((slug, order)): Path<(String, i32)>,
) -> Result<Json<DataEnvelope<NovelChapter>>, ApiError> {
    let ch = repo::publish_chapter(&state.db, &slug, order)
        .await
        .map_err(ApiError::internal)?;
    let doc_id = format!("{slug}/chapters/{}", ch.chapter_order);
    search::upsert(
        &state.db,
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

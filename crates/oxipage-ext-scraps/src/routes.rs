use crate::model::{ListQuery, ScrapInput, ScrapItem, ScrapPatch};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};
use oxipage_core::auth::AdminAuth;
use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::search;
use oxipage_core::state::AppState;

pub async fn list_published(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<ScrapItem>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let items = repo::list(&state.db, true, q.source.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: items }))
}

pub async fn list_queue(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<ScrapItem>>>, ApiError> {
    let limit = q.limit.unwrap_or(50);
    let items = repo::list(&state.db, false, q.source.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: items }))
}

pub async fn create_manual(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<ScrapInput>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    validate_input(&input)?;
    let item = repo::create_published(&state.db, &input)
        .await
        .map_err(ApiError::internal)?;
    reindex(&state, &item).await?;
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    let item = repo::find_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if item.published_at.is_none() {
        return Err(not_found(id));
    }
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn update(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(patch): Json<ScrapPatch>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    let item = repo::update(&state.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if item.published_at.is_some() {
        reindex(&state, &item).await?;
    }
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn delete(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&state.db, id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(id));
    }
    search::delete(&state.db, "scraps", &crate::model::search_doc_id(id))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "deleted": true }),
    }))
}

pub async fn publish(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    auth.require_scope("post:publish")?;
    let item = repo::publish(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    reindex(&state, &item).await?;
    let note = item.note_ko.clone().or_else(|| item.note_en.clone()).unwrap_or_default();
    let desc: String = note.chars().take(200).collect();
    Ok(Json(DataEnvelope { data: item }))
}

/// 백그라운드 잡으로 큐에 들어간 row를 DB에서 직접 insert하기 위한 helper.
/// 테스트/관리 도구에서만 쓰이며 HTTP 라우트로 노출되지 않는다.
pub async fn debug_insert_queue_item(
    pool: &sqlx::SqlitePool,
    source: &str,
    source_item_id: &str,
    source_url: &str,
    title: &str,
    og_image_url: Option<&str>,
) -> anyhow::Result<ScrapItem> {
    repo::upsert_queue_item(pool, source, source_item_id, source_url, title, og_image_url).await
}

fn validate_input(input: &ScrapInput) -> Result<(), ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if !is_valid_url(&input.source_url) {
        return Err(ApiError::validation(
            "source_url",
            "source_url must start with http:// or https://",
        ));
    }
    if let Some(src) = input.source.as_deref()
        && !matches!(src, "hackernews" | "geeknews" | "manual")
    {
        return Err(ApiError::validation(
            "source",
            "source must be hackernews|geeknews|manual",
        ));
    }
    Ok(())
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

async fn reindex(state: &AppState, item: &ScrapItem) -> Result<(), ApiError> {
    let body = crate::model::fts_body(item.note_ko.as_deref(), item.note_en.as_deref());
    search::upsert(
        &state.db,
        "scraps",
        &crate::model::search_doc_id(item.id),
        &item.title,
        &body,
        None,
        item.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

fn not_found(id: i64) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("scrap {id} not found"),
    )
}
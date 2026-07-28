use crate::model::{LinkCardInput, LinkCardPatch, ListQuery};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<crate::model::LinkCard>>>, ApiError> {
    let limit = q.limit.unwrap_or(50);
    let cards = repo::list(&state.db, q.featured, limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: cards }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<LinkCardInput>,
) -> Result<Json<DataEnvelope<crate::model::LinkCard>>, ApiError> {
    validate(&input.title, &input.url)?;
    let card = repo::create(&state.db, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: card }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<crate::model::LinkCard>>, ApiError> {
    let card = repo::find_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    Ok(Json(DataEnvelope { data: card }))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(patch): Json<LinkCardPatch>,
) -> Result<Json<DataEnvelope<crate::model::LinkCard>>, ApiError> {
    if let Some(ref title) = patch.title
        && title.trim().is_empty()
    {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if let Some(ref url) = patch.url
        && !is_valid_url(url)
    {
        return Err(ApiError::validation("url", "url must start with http:// or https://"));
    }
    let card = repo::update(&state.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    Ok(Json(DataEnvelope { data: card }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&state.db, id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(id));
    }
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "deleted": true }),
    }))
}

fn validate(title: &str, url: &str) -> Result<(), ApiError> {
    if title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if !is_valid_url(url) {
        return Err(ApiError::validation("url", "url must start with http:// or https://"));
    }
    Ok(())
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn not_found(id: i64) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("link {id} not found"),
    )
}

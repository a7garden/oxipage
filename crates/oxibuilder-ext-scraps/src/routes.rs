use crate::model::{ListQuery, ScrapInput, ScrapItem, ScrapPatch};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};

use oxibuilder_core::error::ApiError;
use oxibuilder_core::extension::DataEnvelope;
use oxibuilder_core::search;
use oxibuilder_core::state::SiteScopedDb;
use sqlx::SqlitePool;

pub async fn list_published(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<ScrapItem>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let items = repo::list(&pool.db, true, q.source.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: items }))
}

pub async fn list_queue(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<ScrapItem>>>, ApiError> {
    let limit = q.limit.unwrap_or(50);
    let items = repo::list(&pool.db, false, q.source.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: items }))
}

pub async fn create_manual(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<ScrapInput>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    validate_input(&input)?;
    let item = repo::create_published(&pool.db, &input)
        .await
        .map_err(ApiError::internal)?;
    reindex(&pool.db, &item).await?;
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn show(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    let item = repo::find_by_id(&pool.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if item.published_at.is_none() {
        return Err(not_found(id));
    }
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn update(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
    Json(patch): Json<ScrapPatch>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    let item = repo::update(&pool.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if item.published_at.is_some() {
        reindex(&pool.db, &item).await?;
    }
    Ok(Json(DataEnvelope { data: item }))
}

pub async fn delete(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&pool.db, id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(id));
    }
    search::delete(&pool.db, "scraps", &crate::model::search_doc_id(id))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "deleted": true }),
    }))
}

pub async fn publish(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<ScrapItem>>, ApiError> {
    let item = repo::publish(&pool.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    reindex(&pool.db, &item).await?;
    let note = item
        .note_ko
        .clone()
        .or_else(|| item.note_en.clone())
        .unwrap_or_default();
    let _desc: String = note.chars().take(200).collect();
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
    repo::upsert_queue_item(
        pool,
        source,
        source_item_id,
        source_url,
        title,
        og_image_url,
    )
    .await
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
    if let Some(og) = &input.og_image_url
        && !og.is_empty()
        && !oxibuilder_core::validation::is_image_value(og)
    {
        return Err(ApiError::validation(
            "og_image_url",
            "og_image_url must be an http(s) URL or site media path",
        ));
    }
    Ok(())
}

fn is_valid_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

async fn reindex(db: &SqlitePool, item: &ScrapItem) -> Result<(), ApiError> {
    let body = crate::model::fts_body(item.note_ko.as_deref(), item.note_en.as_deref());
    search::upsert(
        db,
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

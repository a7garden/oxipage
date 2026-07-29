use crate::model::{BlogPatch, BlogPost, BlogPostInput, ListQuery};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::search;
use oxipage_core::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<BlogPost>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let posts = repo::list(&state.db, q.draft, q.lang.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: posts }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<BlogPostInput>,
) -> Result<Json<DataEnvelope<BlogPost>>, ApiError> {
    validate_input(&input)?;
    let base_slug = input
        .slug
        .clone()
        .unwrap_or_else(|| repo::slugify(&input.title));
    let slug = repo::ensure_unique_slug(&state.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;
    let post = repo::create(&state.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: post }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<BlogPost>>, ApiError> {
    let post = repo::find_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    // 발행본만 공개. 초안은 404로 숨김.
    if post.published_at.is_none() {
        return Err(not_found(&slug));
    }
    Ok(Json(DataEnvelope { data: post }))
}

pub async fn update(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(patch): Json<BlogPatch>,
) -> Result<Json<DataEnvelope<BlogPost>>, ApiError> {
    if let Some(lang) = &patch.lang
        && lang != "ko"
        && lang != "en"
    {
        return Err(ApiError::validation("lang", "lang must be 'ko' or 'en'"));
    }
    let post = repo::update(&state.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    // 발행본이면 FTS re-upsert.
    if post.published_at.is_some() {
        reindex(&state, &post).await?;
    }
    Ok(Json(DataEnvelope { data: post }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&state.db, "blog", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<BlogPost>>, ApiError> {
    if repo::find_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let post = repo::publish(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    reindex(&state, &post).await?;
    let _snapshot_path = format!("/blog/{}", post.slug);
    let _desc: String = post.body.chars().take(200).collect();
    Ok(Json(DataEnvelope { data: post }))
}

async fn reindex(state: &AppState, post: &BlogPost) -> Result<(), ApiError> {
    search::upsert(
        &state.db,
        "blog",
        &post.slug,
        &post.title,
        &post.body,
        Some(&post.lang),
        post.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

fn validate_input(input: &BlogPostInput) -> Result<(), ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if input.lang != "ko" && input.lang != "en" {
        return Err(ApiError::validation("lang", "lang must be 'ko' or 'en'"));
    }
    Ok(())
}

fn not_found(slug: &str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("blog post '{slug}' not found"),
    )
}

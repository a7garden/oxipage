use crate::client::BooksClient;
use crate::model::{
    Book, BookInput, BookPatch, BookSearchResult, ListQuery, SearchQuery, validate_source,
    validate_status,
};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::rating::Rating;
use oxipage_core::search;
use oxipage_core::state::SiteScopedDb;
use sqlx::SqlitePool;

pub async fn list(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<Book>>>, ApiError> {
    if let Some(s) = &q.status
        && !validate_status(s)
    {
        return Err(ApiError::validation(
            "status",
            "status must be wishlist|reading|completed|dropped",
        ));
    }
    let limit = q.limit.unwrap_or(20);
    let books = repo::list(&pool.db, q.status.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: books }))
}

pub async fn create(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<BookInput>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    validate_create(&input)?;
    let book = repo::create(&pool.db, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: book }))
}

pub async fn show(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    let book = repo::find_by_id(&pool.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if book.published_at.is_none() {
        return Err(not_found(id));
    }
    Ok(Json(DataEnvelope { data: book }))
}

pub async fn update(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
    Json(patch): Json<BookPatch>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    validate_patch(&patch)?;
    let book = repo::update(&pool.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if book.published_at.is_some() {
        reindex(&pool.db, &book).await?;
    }
    Ok(Json(DataEnvelope { data: book }))
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
    search::delete(&pool.db, "books", &id.to_string())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "deleted": true }),
    }))
}

pub async fn publish(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    if repo::find_by_id(&pool.db, id)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(id));
    }
    let book = repo::publish(&pool.db, id)
        .await
        .map_err(ApiError::internal)?;
    reindex(&pool.db, &book).await?;
    let review = book
        .review_ko
        .clone()
        .or_else(|| book.review_en.clone())
        .unwrap_or_default();
    let _desc: String = review.chars().take(200).collect();
    Ok(Json(DataEnvelope { data: book }))
}

/// 외부 도서 검색 (aladin → google_books).
/// 알라딘 키(`OXIPAGE_ALADIN_TTBKEY`)가 없으면 503 `book_search_disabled` (acceptance test).
/// Google Books는 키가 필요 없으므로 알라딘이 켜져 있으면 폴백까지 동작.
pub async fn external_search(
    Query(q): Query<SearchQuery>,
) -> Result<Json<DataEnvelope<Vec<BookSearchResult>>>, ApiError> {
    let Some(query) = q.q.as_deref().filter(|s| !s.trim().is_empty()) else {
        return Err(ApiError::validation("q", "q must not be empty"));
    };
    let client = BooksClient::from_env();
    if !client.aladin_enabled() {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "book_search_disabled",
            "OXIPAGE_ALADIN_TTBKEY is not set; book external search is disabled",
        ));
    }
    let limit = q.limit.unwrap_or(10).clamp(1, 20) as usize;
    let results = client
        .search(query, limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: results }))
}

async fn reindex(db: &SqlitePool, book: &Book) -> Result<(), ApiError> {
    // FTS 본문: review_en 우선, 없으면 review_ko. 둘 다 없으면 빈 문자열.
    let body = book
        .review_en
        .as_deref()
        .or(book.review_ko.as_deref())
        .unwrap_or("");
    search::upsert(
        db,
        "books",
        &book.id.to_string(),
        &book.title,
        body,
        None,
        book.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

fn validate_create(input: &BookInput) -> Result<(), ApiError> {
    if input.title.trim().is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if !validate_source(&input.source) {
        return Err(ApiError::validation(
            "source",
            "source must be aladin|google_books|open_library|manual",
        ));
    }
    if !validate_status(&input.status) {
        return Err(ApiError::validation(
            "status",
            "status must be wishlist|reading|completed|dropped",
        ));
    }
    Rating::new(input.rating).map_err(|e| ApiError::validation("rating", &e.to_string()))?;
    Ok(())
}

fn validate_patch(patch: &BookPatch) -> Result<(), ApiError> {
    if let Some(title) = &patch.title
        && title.trim().is_empty()
    {
        return Err(ApiError::validation("title", "title must not be empty"));
    }
    if let Some(s) = &patch.source
        && !validate_source(s)
    {
        return Err(ApiError::validation(
            "source",
            "source must be aladin|google_books|open_library|manual",
        ));
    }
    if let Some(s) = &patch.status
        && !validate_status(s)
    {
        return Err(ApiError::validation(
            "status",
            "status must be wishlist|reading|completed|dropped",
        ));
    }
    if let Some(r) = patch.rating {
        Rating::new(r).map_err(|e| ApiError::validation("rating", &e.to_string()))?;
    }
    Ok(())
}

fn not_found(id: i64) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("book {id} not found"),
    )
}

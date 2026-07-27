use crate::client::BooksClient;
use crate::model::{
    Book, BookInput, BookPatch, BookSearchResult, ListQuery, SearchQuery, validate_source,
    validate_status,
};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};
use oxipage_core::auth::AdminAuth;
use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::rating::Rating;
use oxipage_core::search;
use oxipage_core::snapshot;
use oxipage_core::state::AppState;

pub async fn list(
    State(state): State<AppState>,
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
    let books = repo::list(&state.db, q.status.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: books }))
}

pub async fn create(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<BookInput>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    validate_create(&input)?;
    let book = repo::create(&state.db, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: book }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    let book = repo::find_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if book.published_at.is_none() {
        return Err(not_found(id));
    }
    Ok(Json(DataEnvelope { data: book }))
}

pub async fn update(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(patch): Json<BookPatch>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    validate_patch(&patch)?;
    let book = repo::update(&state.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if book.published_at.is_some() {
        reindex(&state, &book).await?;
    }
    Ok(Json(DataEnvelope { data: book }))
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
    search::delete(&state.db, "books", &id.to_string())
        .await
        .map_err(ApiError::internal)?;
    let _ = snapshot::remove_snapshot(&state, &format!("/books/{id}")).await;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "deleted": true }),
    }))
}

pub async fn publish(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    auth.require_scope("post:publish")?;
    if repo::find_by_id(&state.db, id)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(id));
    }
    let book = repo::publish(&state.db, id)
        .await
        .map_err(ApiError::internal)?;
    reindex(&state, &book).await?;
    let review = book.review_ko.clone().or_else(|| book.review_en.clone()).unwrap_or_default();
    let desc: String = review.chars().take(200).collect();
    snapshot::write_snapshot_for(
        &state,
        &format!("/books/{}", book.id),
        &snapshot::SnapshotData {
            title: book.title.clone(),
            description: if desc.trim().is_empty() { book.title.clone() } else { desc },
            canonical_url: format!(
                "{}/books/{}",
                state.config.site.base_url.trim_end_matches('/'),
                book.id
            ),
            og_image: book.cover_image_url.clone(),
            body_markdown: review,
            lang: if book.review_ko.is_some() { "ko".to_string() } else { "en".to_string() },
        },
    )
    .await;
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
    let results = client.search(query, limit).await.map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: results }))
}

async fn reindex(state: &AppState, book: &Book) -> Result<(), ApiError> {
    // FTS 본문: review_en 우선, 없으면 review_ko. 둘 다 없으면 빈 문자열.
    let body = book
        .review_en
        .as_deref()
        .or(book.review_ko.as_deref())
        .unwrap_or("");
    search::upsert(
        &state.db,
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
    Rating::new(input.rating)
        .map_err(|e| ApiError::validation("rating", &e.to_string()))?;
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

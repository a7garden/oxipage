use crate::client::BooksClient;
use crate::model::{
    Book, BookInput, BookPatch, BookSearchResult, ListQuery, SearchQuery, validate_source,
    validate_status,
};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};

use oxibuilder_core::error::ApiError;
use oxibuilder_core::extension::DataEnvelope;
use oxibuilder_core::rating::Rating;
use oxibuilder_core::search;
use oxibuilder_core::state::SiteScopedDb;
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
    let books = repo::list(&pool.db, q.status.as_deref(), limit, q.draft)
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

/// 알라딘/구글북스에서 메타를 다시 받아 신규 컬럼(`category`/`publisher`/`page_count`)
/// 만 안전하게 PATCH 한다. 사용자 편집(title/review/rating)은 절대 건드리지 않는다.
/// - entry 가 없으면 404.
/// - source 가 `manual`/`open_library` 면 422 (재조회 불가).
/// - aladin 소스인데 `OXIBUILDER_ALADIN_TTBKEY` 가 unset 이면 503 `book_search_disabled`.
pub async fn refresh(
    Extension(pool): Extension<SiteScopedDb>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<Book>>, ApiError> {
    let book = repo::find_by_id(&pool.db, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;

    // 재조회 대상인지 판단. manual/open_library 는 외부 소스가 아니다.
    match book.source.as_str() {
        "aladin" | "google_books" => {}
        _ => {
            return Err(ApiError::validation(
                "source",
                "refresh only supports entries with source=aladin|google_books",
            ));
        }
    }

    // 검색 쿼리: isbn13 우선, 없으면 title. 외부 검색 endpoint 와 동일.
    let query = book
        .isbn13
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| book.title.clone());
    if query.trim().is_empty() {
        return Err(ApiError::validation(
            "isbn13",
            "entry has neither isbn13 nor title; cannot search for refresh",
        ));
    }

    let client = BooksClient::from_env();
    if book.source == "aladin" && !client.aladin_enabled() {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "book_search_disabled",
            "OXIBUILDER_ALADIN_TTBKEY is not set; book external search is disabled",
        ));
    }

    let results = client.search(&query, 5).await.map_err(ApiError::internal)?;
    let hit = match results.into_iter().next() {
        Some(r) => r,
        None => {
            // 검색 결과 없음 = 변경 없음. 안전: 현재 값을 그대로 반환.
            return Ok(Json(DataEnvelope { data: book }));
        }
    };

    // 안전 패치: 신규 메타 필드만 PATCH. 나머지는 보존.
    let patch = BookPatch {
        category: hit.category,
        publisher: hit.publisher,
        page_count: hit.page_count,
        ..Default::default()
    };
    let updated = repo::update(&pool.db, id, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(id))?;
    if updated.published_at.is_some() {
        reindex(&pool.db, &updated).await?;
    }
    Ok(Json(DataEnvelope { data: updated }))
}

/// 외부 도서 검색 (aladin → google_books).
/// 알라딘 키(`OXIBUILDER_ALADIN_TTBKEY`)가 없으면 503 `book_search_disabled` (acceptance test).
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
            "OXIBUILDER_ALADIN_TTBKEY is not set; book external search is disabled",
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
    if let Some(isbn) = &input.isbn13
        && !isbn.is_empty()
        && !oxibuilder_core::validation::validate_isbn13(isbn)
    {
        return Err(ApiError::validation(
            "isbn13",
            "isbn13 is not a valid ISBN-13",
        ));
    }
    if !oxibuilder_core::validation::validate_date_order(
        input.started_at.as_deref(),
        input.finished_at.as_deref(),
    ) {
        return Err(ApiError::validation(
            "finished_at",
            "finished_at must not precede started_at",
        ));
    }
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
    if let Some(isbn) = &patch.isbn13
        && !isbn.is_empty()
        && !oxibuilder_core::validation::validate_isbn13(isbn)
    {
        return Err(ApiError::validation(
            "isbn13",
            "isbn13 is not a valid ISBN-13",
        ));
    }
    if !oxibuilder_core::validation::validate_date_order(
        patch.started_at.as_deref(),
        patch.finished_at.as_deref(),
    ) {
        return Err(ApiError::validation(
            "finished_at",
            "finished_at must not precede started_at",
        ));
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

use crate::integration::{MovieMeta, TmdbClient};
use crate::model::{
    GenreInput, ListQuery, MovieEntry, MovieEntryDetail, MovieEntryInput, MovieEntryPatch,
    SeriesGroup, SeriesGroupDetail, SeriesGroupInput, SeriesGroupPatch, TmdbSearchResult,
};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};
use axum::http::StatusCode;

use oxibuilder_core::error::ApiError;
use oxibuilder_core::extension::DataEnvelope;
use oxibuilder_core::rating::Rating;
use oxibuilder_core::search;
use oxibuilder_core::state::SiteScopedDb;

// ─── MovieEntry ───

pub async fn list(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<MovieEntryDetail>>>, ApiError> {
    let limit = q.limit.unwrap_or(200);
    let entries = repo::list_entries_detail(&pool.db, limit, q.draft)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: entries }))
}

pub async fn create(
    Extension(pool): Extension<SiteScopedDb>,
    mut input: Json<MovieEntryInput>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    validate_input(&input)?;

    // rating 0~10 검증.
    Rating::new(input.rating).map_err(|_| {
        ApiError::validation(
            "rating",
            "rating must be an integer between 0 and 10 (inclusive)",
        )
    })?;

    // release_year 4자리 + series_order 양수 검증.
    if let Some(y) = input.release_year
        && oxibuilder_core::validation::validate_year(y).is_none()
    {
        return Err(ApiError::validation(
            "release_year",
            "release_year must be a 4-digit year",
        ));
    }
    if let Some(o) = input.series_order
        && o <= 0
    {
        return Err(ApiError::validation(
            "series_order",
            "series_order must be positive",
        ));
    }

    // tmdb_id가 있고 키가 있으면 풀 메타 fetch (ko/en 제목, 장르, 출연진, 런타임).
    let tmdb = TmdbClient::from_env();
    let meta: Option<MovieMeta> = if let Some(id) = input.tmdb_id
        && tmdb.enabled()
    {
        match tmdb.fetch_movie_full(id).await {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::warn!(error = ?e, tmdb_id = id, "TMDB full meta fetch failed; falling back to client input");
                None
            }
        }
    } else {
        None
    };

    // 우선순위: 클라이언트 명시 > TMDB > None.
    let title = input
        .title
        .clone()
        .or_else(|| meta.as_ref().and_then(|m| m.title_ko.clone()))
        .or_else(|| meta.as_ref().and_then(|m| m.title_en.clone()))
        .ok_or_else(|| {
            ApiError::validation(
                "title",
                "title is required when tmdb_id is not provided or TMDB is disabled",
            )
        })?;
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }

    let title_ko = input
        .title_ko
        .clone()
        .or_else(|| meta.as_ref().and_then(|m| m.title_ko.clone()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let title_en = input
        .title_en
        .clone()
        .or_else(|| meta.as_ref().and_then(|m| m.title_en.clone()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let poster_path = input
        .poster_path
        .clone()
        .or_else(|| meta.as_ref().and_then(|m| m.poster_path.clone()));
    let release_year = input
        .release_year
        .or_else(|| meta.as_ref().and_then(|m| m.release_year));
    let runtime_min = input
        .runtime_min
        .or_else(|| meta.as_ref().and_then(|m| m.runtime_min));
    let origin = input
        .origin
        .clone()
        .or_else(|| meta.as_ref().and_then(|m| m.origin.clone()));

    // 장르/출연진/감독: 입력이 없으면 TMDB 메타로 채운다.
    if input.genres.is_none()
        && let Some(m) = &meta
    {
        input.genres = Some(
            m.genres
                .iter()
                .map(|g| GenreInput {
                    name_en: Some(g.name_en.clone()),
                    name_ko: g.name_ko.clone(),
                })
                .collect(),
        );
    }
    if input.cast.is_none() {
        input.cast = meta.as_ref().map(|m| m.cast.clone());
    }
    if input.directors.is_none() {
        input.directors = meta.as_ref().map(|m| m.directors.clone());
    }

    // slug: 명시 > title.
    let base_slug = input.slug.clone().unwrap_or_else(|| repo::slugify(&title));
    let slug = repo::ensure_unique_entry_slug(&pool.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;

    let entry = repo::create_entry(
        &pool.db,
        &input,
        &slug,
        input.tmdb_id,
        title,
        title_ko,
        title_en,
        poster_path,
        release_year,
        runtime_min,
        origin,
    )
    .await
    .map_err(ApiError::internal)?;

    let detail = repo::find_entry_detail_by_slug(&pool.db, &entry.slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&entry.slug))?;
    Ok(Json(DataEnvelope { data: detail }))
}

pub async fn show(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    let detail = repo::find_entry_detail_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    // 초안은 404로 숨김.
    if detail.entry.published_at.is_none() {
        return Err(not_found(&slug));
    }
    Ok(Json(DataEnvelope { data: detail }))
}

pub async fn update(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(patch): Json<MovieEntryPatch>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    // 부분 입력 검증.
    if let Some(media) = &patch.media_type
        && media != "movie"
        && media != "tv"
    {
        return Err(ApiError::validation(
            "media_type",
            "media_type must be 'movie' or 'tv'",
        ));
    }
    if let Some(r) = patch.rating {
        Rating::new(r).map_err(|_| {
            ApiError::validation(
                "rating",
                "rating must be an integer between 0 and 10 (inclusive)",
            )
        })?;
    }

    let entry = repo::update_entry(&pool.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;

    // 발행본이면 FTS re-upsert.
    if entry.published_at.is_some() {
        reindex(&pool.db, &entry).await?;
    }
    let detail = repo::find_entry_detail_by_slug(&pool.db, &entry.slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&entry.slug))?;
    Ok(Json(DataEnvelope { data: detail }))
}

pub async fn delete(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_entry(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&pool.db, "movies", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    if repo::find_entry_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let entry = repo::publish_entry(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    reindex(&pool.db, &entry).await?;
    let detail = repo::find_entry_detail_by_slug(&pool.db, &entry.slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&entry.slug))?;
    Ok(Json(DataEnvelope { data: detail }))
}


/// TMDB 메타를 다시 가져와 새 컬럼(`origin`)만 안전하게 PATCH 한다.
/// - 키가 없으면 503 `tmdb_disabled`.
/// - `tmdb_id`가 없으면 422 `validation_error` (TMDB 연동 자체가 안된 항목).
pub async fn refresh(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntryDetail>>, ApiError> {
    let entry = repo::find_entry_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;

    // tmdb_id 가 없으면 키 체크보다 먼저 422 (키가 있어도 fetch 자체가 불가능).
    let tmdb_id = entry.tmdb_id.ok_or_else(|| {
        ApiError::validation("tmdb_id", "entry has no tmdb_id; cannot refresh from TMDB")
    })?;
    let tmdb = TmdbClient::from_env();
    if !tmdb.enabled() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "tmdb_disabled",
            "TMDB integration is disabled (set OXIBUILDER_TMDB_KEY to enable)",
        ));
    }

    let meta = tmdb
        .fetch_movie_full(tmdb_id)
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, slug, tmdb_id, "TMDB refresh fetch failed");
            ApiError::internal(e)
        })?;

    // 안전 패치: TMDB-sourced 필드 중 신규(`origin`)만 갱신. 나머지는 보존.
    repo::update_entry(
        &pool.db,
        &slug,
        &MovieEntryPatch {
            origin: meta.origin,
            ..Default::default()
        },
    )
    .await
    .map_err(ApiError::internal)?;

    let detail = repo::find_entry_detail_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    Ok(Json(DataEnvelope { data: detail }))
}

// ─── TMDB search ───

pub async fn tmdb_search(
    Query(q): Query<TmdbSearchQuery>,
) -> Result<Json<DataEnvelope<Vec<TmdbSearchResult>>>, ApiError> {
    let tmdb = TmdbClient::from_env();
    if !tmdb.enabled() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "tmdb_disabled",
            "TMDB integration is disabled (set OXIBUILDER_TMDB_KEY to enable)",
        ));
    }
    let query = q.q.trim();
    if query.is_empty() {
        return Err(ApiError::validation("q", "q must not be empty"));
    }
    let results = tmdb.search(query).await.map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: results }))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TmdbSearchQuery {
    pub q: String,
}

// ─── SeriesGroup ───

pub async fn create_group(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<SeriesGroupInput>,
) -> Result<Json<DataEnvelope<SeriesGroup>>, ApiError> {
    let ko_empty = input
        .title_ko
        .as_deref()
        .is_none_or(|s| s.trim().is_empty());
    let en_empty = input
        .title_en
        .as_deref()
        .is_none_or(|s| s.trim().is_empty());
    if ko_empty && en_empty {
        return Err(ApiError::validation(
            "title_ko",
            "title_ko or title_en must be non-empty",
        ));
    }
    if let Some(r) = input.group_rating {
        Rating::new(r).map_err(|_| {
            ApiError::validation(
                "group_rating",
                "group_rating must be an integer between 0 and 10 (inclusive)",
            )
        })?;
    }

    let base_title = input
        .title_en
        .clone()
        .or_else(|| input.title_ko.clone())
        .unwrap_or_default();
    let base_slug = input
        .slug
        .clone()
        .unwrap_or_else(|| repo::slugify(&base_title));
    let slug = repo::ensure_unique_group_slug(&pool.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;

    let group = repo::create_group(&pool.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: group }))
}

pub async fn list_groups(
    Extension(pool): Extension<SiteScopedDb>,
) -> Result<Json<DataEnvelope<Vec<SeriesGroup>>>, ApiError> {
    let groups = repo::list_groups(&pool.db)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: groups }))
}

pub async fn show_group(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<SeriesGroupDetail>>, ApiError> {
    let group = repo::find_group_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found_group(&slug))?;
    let entries = repo::list_entries_by_group_id(&pool.db, group.id, true)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: SeriesGroupDetail { group, entries },
    }))
}

pub async fn update_group(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(patch): Json<SeriesGroupPatch>,
) -> Result<Json<DataEnvelope<SeriesGroup>>, ApiError> {
    let group = repo::update_group(&pool.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found_group(&slug))?;
    Ok(Json(DataEnvelope { data: group }))
}

pub async fn delete_group(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_group(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found_group(&slug));
    }
    Ok(Json(DataEnvelope {
        data: serde_json::json!({"removed": true}),
    }))
}

fn not_found_group(slug: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        &format!("series group '{slug}' not found"),
    )
}

// ─── helpers ───

fn validate_input(input: &MovieEntryInput) -> Result<(), ApiError> {
    if input.media_type != "movie" && input.media_type != "tv" {
        return Err(ApiError::validation(
            "media_type",
            "media_type must be 'movie' or 'tv'",
        ));
    }
    // tmdb_id도 title도 둘 다 없으면 안 된다.
    if input.tmdb_id.is_none() && input.title.as_deref().is_none_or(|s| s.trim().is_empty()) {
        return Err(ApiError::validation(
            "title",
            "either tmdb_id or title must be provided",
        ));
    }
    Ok(())
}

fn not_found(slug: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        &format!("movie '{slug}' not found"),
    )
}

async fn reindex(db: &sqlx::SqlitePool, entry: &MovieEntry) -> Result<(), ApiError> {
    // 검색 제목은 현지화 표시 제목(ko 우선).
    let title = entry.display_title();
    let body_en = entry.review_en.as_deref().unwrap_or("");
    let body_ko = entry.review_ko.as_deref().unwrap_or("");
    let body = if !body_en.is_empty() {
        body_en
    } else {
        body_ko
    };
    let lang = if entry.review_en.is_some() {
        Some("en")
    } else if entry.review_ko.is_some() {
        Some("ko")
    } else {
        None
    };
    search::upsert(
        db,
        "movies",
        &entry.slug,
        title,
        body,
        lang,
        entry.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

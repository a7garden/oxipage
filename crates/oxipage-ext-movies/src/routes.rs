use crate::integration::TmdbClient;
use crate::model::{
    ListQuery, MovieEntry, MovieEntryInput, MovieEntryPatch, SeriesGroup, SeriesGroupDetail,
    SeriesGroupInput, TmdbSearchResult,
};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::rating::Rating;
use oxipage_core::search;
use oxipage_core::state::AppState;

// ─── MovieEntry ───

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<MovieEntry>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let entries = repo::list_entries_published(&state.db, q.series_group.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: entries }))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<MovieEntryInput>,
) -> Result<Json<DataEnvelope<MovieEntry>>, ApiError> {
    validate_input(&input)?;

    // rating 0~10 검증.
    Rating::new(input.rating).map_err(|_| {
        ApiError::validation(
            "rating",
            "rating must be an integer between 0 and 10 (inclusive)",
        )
    })?;

    // tmdb_id가 있고 키가 있으면 메타 1회 fetch. 클라이언트 명시 값이 있으면 그게 우선.
    let tmdb = TmdbClient::from_env();
    let fetched: Option<TmdbSearchResult> = if let Some(id) = input.tmdb_id
        && tmdb.enabled()
    {
        match tmdb.fetch_movie(id).await {
            Ok(m) => Some(m),
            Err(e) => {
                // 메타 fetch 실패는 치명적이진 않다. 키만 있으면 manual 폴백.
                tracing::warn!(error = ?e, tmdb_id = id, "TMDB movie fetch failed; falling back to client input");
                None
            }
        }
    } else {
        None
    };

    // 우선순위: 클라이언트 명시 > TMDB fetch > None.
    let title = input
        .title
        .clone()
        .or_else(|| fetched.as_ref().map(|f| f.title.clone()))
        .ok_or_else(|| {
            ApiError::validation(
                "title",
                "title is required when tmdb_id is not provided or TMDB is disabled",
            )
        })?;
    let title = title.trim();
    if title.is_empty() {
        return Err(ApiError::validation("title", "title must not be empty"));
    }

    let poster_path = input
        .poster_path
        .clone()
        .or_else(|| fetched.as_ref().and_then(|f| f.poster_path.clone()));
    let release_year = input
        .release_year
        .or_else(|| fetched.as_ref().and_then(|f| f.release_year));

    // slug: 명시 > title.
    let base_slug = input.slug.clone().unwrap_or_else(|| repo::slugify(title));
    let slug = repo::ensure_unique_entry_slug(&state.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;

    let entry = repo::create_entry(
        &state.db,
        &input,
        &slug,
        input.tmdb_id,
        title.to_string(),
        poster_path,
        release_year,
    )
    .await
    .map_err(ApiError::internal)?;

    Ok(Json(DataEnvelope { data: entry }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntry>>, ApiError> {
    let entry = repo::find_entry_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    // 초안은 404로 숨김.
    if entry.published_at.is_none() {
        return Err(not_found(&slug));
    }
    Ok(Json(DataEnvelope { data: entry }))
}

pub async fn update(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(patch): Json<MovieEntryPatch>,
) -> Result<Json<DataEnvelope<MovieEntry>>, ApiError> {
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

    let entry = repo::update_entry(&state.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;

    // 발행본이면 FTS re-upsert.
    if entry.published_at.is_some() {
        reindex(&state, &entry).await?;
    }
    Ok(Json(DataEnvelope { data: entry }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_entry(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&state.db, "movies", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<MovieEntry>>, ApiError> {
    if repo::find_entry_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let entry = repo::publish_entry(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    reindex(&state, &entry).await?;
    let review = entry
        .review_ko
        .clone()
        .or_else(|| entry.review_en.clone())
        .unwrap_or_default();
    let _desc: String = review.chars().take(200).collect();
    let _og_image = entry.poster_path.clone().map(|p| {
        if p.starts_with("http") {
            p
        } else {
            format!("https://image.tmdb.org/t/p/w500{p}")
        }
    });
    Ok(Json(DataEnvelope { data: entry }))
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
            "TMDB integration is disabled (set OXIPAGE_TMDB_KEY to enable)",
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
    State(state): State<AppState>,
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
    let slug = repo::ensure_unique_group_slug(&state.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;

    let group = repo::create_group(&state.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: group }))
}

pub async fn list_groups(
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<Vec<SeriesGroup>>>, ApiError> {
    let groups = repo::list_groups(&state.db)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: groups }))
}

pub async fn show_group(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<SeriesGroupDetail>>, ApiError> {
    let group = repo::find_group_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found_group(&slug))?;
    let entries = repo::list_entries_by_group_id(&state.db, group.id, true)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: SeriesGroupDetail { group, entries },
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

async fn reindex(state: &AppState, entry: &MovieEntry) -> Result<(), ApiError> {
    // title 우선: 영어 리뷰가 있으면 en. 없으면 ko. 둘 다 없으면 빈 문자열.
    let title = entry.title.clone();
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
        &state.db,
        "movies",
        &entry.slug,
        &title,
        body,
        lang,
        entry.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

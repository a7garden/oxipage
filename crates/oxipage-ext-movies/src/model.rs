use serde::{Deserialize, Serialize};

/// doc/02 §2.9 MovieEntry — 개별 작품 평가 행.
/// DB INTEGER 칼럼은 i8로 두고 Rating 검증은 핸들러에서.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MovieEntry {
    pub id: i64,
    pub slug: String,
    pub tmdb_id: Option<i64>,
    pub media_type: String,
    pub title: String,
    pub poster_path: Option<String>,
    pub release_year: Option<i32>,
    pub watched_at: Option<String>,
    pub rating: i8,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    pub rewatch: i8,
    pub series_group_id: Option<i64>,
    pub series_order: Option<i32>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// doc/02 §2.9 SeriesGroup — 프랜차이즈 묶음 (선택).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SeriesGroup {
    pub id: i64,
    pub slug: String,
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub cover_image: Option<String>,
    pub group_rating: Option<i8>,
    pub group_review_ko: Option<String>,
    pub group_review_en: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// series_group/{slug} 응답 — 그룹 + 속한 movie_entry들.
#[derive(Debug, Clone, Serialize)]
pub struct SeriesGroupDetail {
    #[serde(flatten)]
    pub group: SeriesGroup,
    pub entries: Vec<MovieEntry>,
}

/// POST /api/console/movies 입력.
/// published_at은 받지 않는다 (초안 우선 원칙).
#[derive(Debug, Clone, Deserialize)]
pub struct MovieEntryInput {
    pub tmdb_id: Option<i64>,
    pub media_type: String,
    /// tmdb_id 없고 키도 없으면 클라이언트가 직접 제공해야 한다.
    pub title: Option<String>,
    pub poster_path: Option<String>,
    pub release_year: Option<i32>,
    pub watched_at: Option<String>,
    /// 0~10 정수. 핸들러에서 Rating::new로 검증.
    pub rating: i8,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    #[serde(default)]
    pub rewatch: bool,
    pub series_group_id: Option<i64>,
    pub series_order: Option<i32>,
    pub slug: Option<String>,
}

/// PATCH 입력 — 전부 Option.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MovieEntryPatch {
    pub tmdb_id: Option<i64>,
    pub media_type: Option<String>,
    pub title: Option<String>,
    pub poster_path: Option<String>,
    pub release_year: Option<i32>,
    pub watched_at: Option<String>,
    pub rating: Option<i8>,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    pub rewatch: Option<bool>,
    pub series_group_id: Option<i64>,
    pub series_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub series_group: Option<String>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeriesGroupInput {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub slug: Option<String>,
    pub cover_image: Option<String>,
    pub group_rating: Option<i8>,
    pub group_review_ko: Option<String>,
    pub group_review_en: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TmdbSearchResult {
    pub tmdb_id: i64,
    pub title: String,
    pub poster_path: Option<String>,
    pub release_year: Option<i32>,
    /// `https://image.tmdb.org/t/p/w500{poster_path}` 절대 URL.
    pub poster_url: Option<String>,
}

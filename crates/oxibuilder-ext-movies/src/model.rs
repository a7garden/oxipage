use serde::{Deserialize, Serialize};

/// doc/02 §2.9 MovieEntry — 개별 작품 평가 행.
/// DB INTEGER 칼럼은 i8로 두고 Rating 검증은 핸들러에서.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MovieEntry {
    pub id: i64,
    pub slug: String,
    pub tmdb_id: Option<i64>,
    pub media_type: String,
    /// 캐노니컬 제목 (NOT NULL, 슬러그/FTS 원천). 표시는 title_ko/title_en 우선.
    pub title: String,
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub poster_path: Option<String>,
    /// 콤마 구분 ISO-3166 alpha-2 ("KR,US"). TMDB production_countries 기반.
    pub origin: Option<String>,
    pub release_year: Option<i32>,
    pub runtime_min: Option<i32>,
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

impl MovieEntry {
    /// 표시용 현지화 제목. ko 우선 → title 폴백 → en.
    pub fn display_title(&self) -> &str {
        self.title_ko
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.title_en.as_deref().filter(|s| !s.is_empty()))
            .unwrap_or(&self.title)
    }
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

// ─── Genres & people ───

/// 장르 현지화 이름 쌍. name_en 이 정규키.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreName {
    pub name_en: String,
    #[serde(default)]
    pub name_ko: Option<String>,
}

/// 입력용 장르 (en 누락 시 ko 를 en 에도 채운다 — 수동 입력 단일 이름 대응).
#[derive(Debug, Clone, Deserialize)]
pub struct GenreInput {
    #[serde(default)]
    pub name_en: Option<String>,
    #[serde(default)]
    pub name_ko: Option<String>,
}

/// 인물 요약 (목록/카드 표시용). 캐릭터명/빌링은 출연 매핑에서 온다.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PersonSummary {
    pub id: i64,
    pub slug: String,
    pub name_en: String,
    pub name_ko: Option<String>,
    pub profile_path: Option<String>,
    pub role: String,
    /// movie_entry_person 조인에서만 채워짐 (목록 조립 시).
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing: Option<i32>,
}

/// 인물 입력. name_en 이 정규키/슬러그 원천 (한글 전용이면 ko 를 en 에도).
#[derive(Debug, Clone, Deserialize)]
pub struct PersonInput {
    pub tmdb_person_id: Option<i64>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub name_en: Option<String>,
    #[serde(default)]
    pub name_ko: Option<String>,
    #[serde(default)]
    pub profile_path: Option<String>,
    /// 'actor' (기본) | 'director'.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub character_name: Option<String>,
    #[serde(default)]
    pub billing: Option<i32>,
}

/// 목록/빌드 응답 — 엔트리 + 장르 + 출연진 + 감독.
#[derive(Debug, Clone, Serialize)]
pub struct MovieEntryDetail {
    #[serde(flatten)]
    pub entry: MovieEntry,
    pub genres: Vec<GenreName>,
    pub cast: Vec<PersonSummary>,
    pub directors: Vec<PersonSummary>,
}

// ─── API payloads ───

/// POST /api/console/movies 입력.
/// published_at은 받지 않는다 (초안 우선 원칙).
#[derive(Debug, Clone, Deserialize)]
pub struct MovieEntryInput {
    pub tmdb_id: Option<i64>,
    pub media_type: String,
    /// tmdb_id 없고 키도 없으면 클라이언트가 직접 제공해야 한다.
    pub title: Option<String>,
    #[serde(default)]
    pub title_ko: Option<String>,
    #[serde(default)]
    pub title_en: Option<String>,
    pub poster_path: Option<String>,
    pub origin: Option<String>,
    pub release_year: Option<i32>,
    #[serde(default)]
    pub runtime_min: Option<i32>,
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
    #[serde(default)]
    pub genres: Option<Vec<GenreInput>>,
    #[serde(default)]
    pub cast: Option<Vec<PersonInput>>,
    #[serde(default)]
    pub directors: Option<Vec<PersonInput>>,
}

/// PATCH 입력 — 전부 Option.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MovieEntryPatch {
    pub tmdb_id: Option<i64>,
    pub media_type: Option<String>,
    pub title: Option<String>,
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub poster_path: Option<String>,
    pub origin: Option<String>,
    pub release_year: Option<i32>,
    pub runtime_min: Option<i32>,
    pub watched_at: Option<String>,
    pub rating: Option<i8>,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    pub rewatch: Option<bool>,
    pub series_group_id: Option<i64>,
    pub series_order: Option<i32>,
    /// Some(vec) = 전체 교체, None = 미변경.
    #[serde(default)]
    pub genres: Option<Vec<GenreInput>>,
    #[serde(default)]
    pub cast: Option<Vec<PersonInput>>,
    #[serde(default)]
    pub directors: Option<Vec<PersonInput>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub series_group: Option<String>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SeriesGroupPatch {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub cover_image: Option<String>,
    pub group_rating: Option<i8>,
    pub group_review_ko: Option<String>,
    pub group_review_en: Option<String>,
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
    /// `https://image.tmdbdb.org/t/p/w500{poster_path}` 절대 URL.
    pub poster_url: Option<String>,
    /// 검색은 movie 엔드포인트 기준.
    #[serde(default)]
    pub media_type: String,
}

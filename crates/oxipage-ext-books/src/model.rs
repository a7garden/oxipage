use serde::{Deserialize, Serialize};

/// DB row. rating은 DB INTEGER (0~10). 모델 struct에서 i8로 받고
/// `oxipage_core::rating::Rating`으로 감싸 검증한다.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Book {
    pub id: i64,
    pub source: String,
    pub external_id: Option<String>,
    pub isbn13: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub cover_image_url: Option<String>,
    pub rating: i8,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 외부 검색 결과 (aladin/google_books 공통 형태). 매니페스트 응답 모델.
#[derive(Debug, Clone, Serialize)]
pub struct BookSearchResult {
    pub source: String,
    pub external_id: Option<String>,
    pub isbn13: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub cover_image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookInput {
    #[serde(default = "default_source")]
    pub source: String,
    pub external_id: Option<String>,
    pub isbn13: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub cover_image_url: Option<String>,
    /// 0~10 정수. 핸들러에서 `Rating::new`로 검증한다.
    pub rating: i8,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

fn default_source() -> String {
    "manual".to_string()
}

fn default_status() -> String {
    "wishlist".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BookPatch {
    pub source: Option<String>,
    pub external_id: Option<String>,
    pub isbn13: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub cover_image_url: Option<String>,
    /// None이면 변경 안 함. 0~10 검증은 핸들러에서.
    pub rating: Option<i8>,
    pub review_ko: Option<String>,
    pub review_en: Option<String>,
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

/// 허용 source / status 값 (라우터/핸들러에서 검증).
pub const ALLOWED_SOURCES: &[&str] = &["aladin", "google_books", "open_library", "manual"];
pub const ALLOWED_STATUSES: &[&str] = &["wishlist", "reading", "completed", "dropped"];

pub fn validate_source(s: &str) -> bool {
    ALLOWED_SOURCES.contains(&s)
}

pub fn validate_status(s: &str) -> bool {
    ALLOWED_STATUSES.contains(&s)
}

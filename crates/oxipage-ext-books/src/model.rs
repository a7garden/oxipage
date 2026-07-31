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

/// 레거시 status 값 정규화 — 구 DB의 `read`/`dnf`를 현재 4값 체계로 매핑.
/// (`ALLOWED_STATUSES` 참조) 쓰기 경로는 CHECK 제약이 이미 차단하므로 읽기 전용.
pub fn normalize_status(s: &str) -> &str {
    match s {
        "read" => "completed",
        "dnf" => "dropped",
        other => other,
    }
}

impl Book {
    /// 읽기 경로 정규화: 레거시 status 값을 현재 4값 체계로 변환해 반환.
    pub fn normalize_status(mut self) -> Self {
        self.status = normalize_status(&self.status).to_string();
        self
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_status_maps_legacy_values() {
        assert_eq!(normalize_status("read"), "completed");
        assert_eq!(normalize_status("dnf"), "dropped");
    }

    #[test]
    fn normalize_status_keeps_current_values() {
        for s in ["wishlist", "reading", "completed", "dropped"] {
            assert_eq!(normalize_status(s), s);
        }
        assert_eq!(normalize_status("unknown"), "unknown");
    }
}

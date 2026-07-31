use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Profile {
    pub display_name: String,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub avatar_url: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    #[sqlx(json)]
    pub education: Vec<Education>,
    #[sqlx(json)]
    pub custom_links: Vec<CustomLink>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Education {
    pub institution: Option<String>,
    pub degree: Option<String>,
    pub field: Option<String>,
    pub start_year: Option<i32>,
    pub end_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomLink {
    pub label: String,
    pub url: String,
    pub icon: Option<String>,
}

/// PUT 전체 교체 입력. 생략된 Option 필드는 NULL로 덮어쓴다.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    /// Last-known `updated_at` from GET /profile. Used for optimistic concurrency.
    /// Use an empty string for unconditional first-write (no prior row).
    pub expected_updated_at: String,
    pub display_name: String,
    pub tagline_ko: Option<String>,
    pub tagline_en: Option<String>,
    pub avatar_url: Option<String>,
    pub bio_ko: Option<String>,
    pub bio_en: Option<String>,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub linkedin_url: Option<String>,
    #[serde(default)]
    pub education: Vec<Education>,
    #[serde(default)]
    pub custom_links: Vec<CustomLink>,
}

/// Pragmatic address check: local@domain.tld with no spaces.
pub fn validate_email(s: &str) -> bool {
    let mut parts = s.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    let mut dparts = domain.split('.');
    let d0 = dparts.next().unwrap_or("");
    let d1 = dparts.next().unwrap_or("");
    !local.is_empty() && !d0.is_empty() && !d1.is_empty() && !s.contains(' ')
}

/// True when start <= end, ignoring missing sides.
pub fn validate_year_range(start: Option<i32>, end: Option<i32>) -> bool {
    match (start, end) {
        (Some(s), Some(e)) => s <= e,
        _ => true,
    }
}

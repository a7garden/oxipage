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

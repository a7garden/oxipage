use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub slug: String,
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    #[sqlx(json)]
    pub tech_stack: Vec<String>,
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    #[sqlx(json)]
    pub links: serde_json::Value,
    pub featured: bool,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Screenshot {
    pub id: i64,
    pub project_id: i64,
    pub url: String,
    pub alt_ko: Option<String>,
    pub alt_en: Option<String>,
    pub display_order: i32,
    pub created_at: String,
}

/// show 응답: project + screenshots 결합.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub project: Project,
    pub screenshots: Vec<Screenshot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectInput {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    #[serde(default)]
    pub links: serde_json::Value,
    #[serde(default)]
    pub featured: bool,
    pub slug: Option<String>,
}

fn default_status() -> String {
    "wip".into()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectPatch {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    pub tech_stack: Option<Vec<String>>,
    pub status: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub links: Option<serde_json::Value>,
    pub featured: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScreenshotPatch {
    pub alt_ko: Option<String>,
    pub alt_en: Option<String>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScreenshotInput {
    pub url: String,
    pub alt_ko: Option<String>,
    pub alt_en: Option<String>,
    #[serde(default)]
    pub display_order: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    /// `draft=true` → 미발행 행 포함. 관리 콘솔용.
    #[serde(default)]
    pub draft: bool,
}

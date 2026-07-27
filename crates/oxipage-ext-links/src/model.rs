use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LinkCard {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    pub thumbnail_url: Option<String>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub display_order: i32,
    pub featured: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkCardInput {
    pub title: String,
    pub url: String,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub display_order: i32,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LinkCardPatch {
    pub title: Option<String>,
    pub url: Option<String>,
    pub description_ko: Option<String>,
    pub description_en: Option<String>,
    pub thumbnail_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub display_order: Option<i32>,
    pub featured: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub featured: Option<bool>,
    pub limit: Option<i64>,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Novel {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub synopsis: Option<String>,
    pub cover_image: Option<String>,
    pub status: String,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct NovelChapter {
    pub id: i64,
    pub novel_id: i64,
    pub chapter_order: i32,
    pub title: String,
    pub body: String,
    pub char_count: i64,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NovelInput {
    pub title: String,
    pub synopsis: Option<String>,
    pub cover_image: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub slug: Option<String>,
}

fn default_status() -> String {
    "ongoing".into()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NovelPatch {
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub cover_image: Option<String>,
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterInput {
    pub chapter_order: i32,
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChapterPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub chapter_order: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    pub draft: bool,
    pub limit: Option<i64>,
}

/// 공백 제외 자수. 한국어 word count는 불규칙하므로 자수를 쓴다 (doc/02 §2.5).
pub fn char_count(body: &str) -> i64 {
    body.chars().filter(|c| !c.is_whitespace()).count() as i64
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterOrderInput {
    pub chapter_ids: Vec<i64>,
}

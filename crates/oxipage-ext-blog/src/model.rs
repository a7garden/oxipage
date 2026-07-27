use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct BlogPost {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub body: String,
    pub lang: String,
    pub translation_group_id: Option<i64>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// POST 입력. published_at은 받지 않는다 (초안 우선 원칙).
#[derive(Debug, Clone, Deserialize)]
pub struct BlogPostInput {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub translation_group_id: Option<i64>,
    /// 사용자 명시 slug. 미지정 시 title로부터 자동 생성.
    pub slug: Option<String>,
}

fn default_lang() -> String {
    "ko".into()
}

/// PATCH 입력. 전부 Option.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BlogPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub lang: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    pub draft: bool,
    pub lang: Option<String>,
    pub limit: Option<i64>,
}

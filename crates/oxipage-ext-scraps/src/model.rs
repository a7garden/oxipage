use serde::{Deserialize, Serialize};

/// `scrap_item` row. background 잡이 채운 추천 큐와 사람이 publish 한 본문을 모두 표현.
/// `source`는 doc/02 §2.7의 enum 제약(`hackernews|geeknews|manual`).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ScrapItem {
    pub id: i64,
    pub source: String,
    pub source_item_id: Option<String>,
    pub source_url: String,
    pub title: String,
    pub og_image_url: Option<String>,
    pub note_ko: Option<String>,
    pub note_en: Option<String>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub scraped_at: String,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// manual 입력 + 잡 upsert 입력 양쪽에 쓰이는 입력 폼.
/// manual에서는 `source`가 항상 `"manual"`, `source_item_id`는 None.
#[derive(Debug, Clone, Deserialize)]
pub struct ScrapInput {
    pub source_url: String,
    pub title: String,
    #[serde(default)]
    pub source: Option<String>,
    pub source_item_id: Option<String>,
    pub og_image_url: Option<String>,
    pub note_ko: Option<String>,
    pub note_en: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 큐 후보의 note/tags 보정용 부분 업데이트 폼.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScrapPatch {
    pub note_ko: Option<String>,
    pub note_en: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    pub og_image_url: Option<String>,
}

/// list/queue 공통 쿼리. `source`는 옵셔널 필터, `limit`은 클램프됨.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListQuery {
    pub source: Option<String>,
    pub limit: Option<i64>,
}

/// FTS body 합성. 한국어/영어 메모가 모두 있으면 둘 다 이어 붙여 검색 품질을 올린다.
pub fn fts_body(note_ko: Option<&str>, note_en: Option<&str>) -> String {
    match (note_ko, note_en) {
        (Some(ko), Some(en)) if !ko.is_empty() && !en.is_empty() => format!("{ko}\n\n{en}"),
        (Some(ko), _) if !ko.is_empty() => ko.to_string(),
        (_, Some(en)) if !en.is_empty() => en.to_string(),
        _ => String::new(),
    }
}

/// 입력의 `source` 정규화: 누락 또는 알 수 없는 값은 `manual`.
/// CHECK 제약(`hackernews|geeknews|manual`)에 맞춰 화이트리스트한다.
pub fn normalize_source(source: Option<&str>) -> &'static str {
    match source {
        Some("hackernews") => "hackernews",
        Some("geeknews") => "geeknews",
        _ => "manual",
    }
}

/// 검색에 쓸 doc_id. URL은 너무 길고 비결정적이라 정수 id 그대로 쓴다.
pub fn search_doc_id(id: i64) -> String {
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_body_prefers_both_notes_when_present() {
        let s = fts_body(Some("안녕"), Some("hello"));
        assert_eq!(s, "안녕\n\nhello");
    }

    #[test]
    fn fts_body_falls_back_to_single_note() {
        assert_eq!(fts_body(Some("ko"), None), "ko");
        assert_eq!(fts_body(None, Some("en")), "en");
        assert_eq!(fts_body(None, None), "");
        assert_eq!(fts_body(Some(""), Some("")), "");
    }

    #[test]
    fn normalize_source_whitelists_values() {
        assert_eq!(normalize_source(Some("hackernews")), "hackernews");
        assert_eq!(normalize_source(Some("geeknews")), "geeknews");
        assert_eq!(normalize_source(Some("manual")), "manual");
        assert_eq!(normalize_source(None), "manual");
        assert_eq!(normalize_source(Some("bogus")), "manual");
    }
}

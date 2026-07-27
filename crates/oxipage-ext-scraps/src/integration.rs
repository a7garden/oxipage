//! 외부 소스에서 큐 후보를 받아오는 순수 파서.
//!
//! Phase 2 한계: HTTP fetch는 별도 잡 인프라(pool 주입)가 필요해서 lib.rs의
//! `ScrapCollectJob::run()`은 no-op이다. 대신 여기서 파싱 로직을 충분히
//! 단위 테스트해 두면, 추후 잡에 pool을 주입할 때 그대로 호출하면 된다.

/// HackerNews topstories 응답에서 앞쪽 N개의 id만 추린다.
/// (Firebase API는 500개 정도의 id 배열을 반환한다.)
pub fn take_top_ids(ids: &[i64], limit: usize) -> Vec<i64> {
    ids.iter().copied().take(limit).collect()
}

/// HN 단건 item JSON → 큐 후보. `url` 필드가 없으면 HN discuss 링크로 폴백.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HackerNewsItem {
    pub id: i64,
    pub title: String,
    pub url: String,
}

/// `serde_json::Value` 한 개를 받아 파싱한다 — 실제 fetch 시 `reqwest::json()` 결과를 그대로 넘긴다.
/// 잘못된 입력은 `None`을 반환해 caller가 단순히 스킵할 수 있게 한다.
pub fn parse_hn_item(value: &serde_json::Value) -> Option<HackerNewsItem> {
    let id = value.get("id")?.as_i64()?;
    let title = value.get("title")?.as_str()?.to_string();
    if title.is_empty() {
        return None;
    }
    let url = value
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("https://news.ycombinator.com/item?id={id}"));
    Some(HackerNewsItem { id, title, url })
}

/// GeekNews RSS XML에서 `<item>` 단위로 추출한다.
/// 의존성을 피하기 위해 단순 split 파싱을 쓴다 — 백그라운드 잡 자체가 no-op이라
/// 실제 XML 검증은 서버 통합 시점에 다시 본다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeekNewsItem {
    pub link: String,
    pub title: String,
}

pub fn parse_geeknews_rss(xml: &str) -> Vec<GeekNewsItem> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(open) = xml[cursor..].find("<item>") {
        let start = cursor + open + "<item>".len();
        let Some(close) = xml[start..].find("</item>") else {
            break;
        };
        let block = &xml[start..start + close];
        if let Some(item) = parse_one_item(block) {
            out.push(item);
        }
        cursor = start + close + "</item>".len();
    }
    out
}

fn parse_one_item(block: &str) -> Option<GeekNewsItem> {
    let link = extract_tag(block, "link")?;
    let title = extract_tag(block, "title")?;
    if link.is_empty() || title.is_empty() {
        return None;
    }
    Some(GeekNewsItem { link, title })
}

/// `<tag>...</tag>` 첫 번째 매치를 추출한다. CDATA / attribute / 멀티라인 모두 허용.
fn extract_tag(input: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let s = input.find(&open)? + open.len();
    let e = input[s..].find(&close)?;
    Some(input[s..s + e].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn take_top_ids_limits() {
        let ids: Vec<i64> = (1..=30).collect();
        assert_eq!(take_top_ids(&ids, 5), vec![1, 2, 3, 4, 5]);
        assert_eq!(take_top_ids(&ids, 100), ids);
        assert!(take_top_ids(&[], 5).is_empty());
    }

    #[test]
    fn parse_hn_item_uses_url_field() {
        let v = json!({"id": 42, "title": "Hello", "url": "https://example.com/x"});
        let item = parse_hn_item(&v).unwrap();
        assert_eq!(item.id, 42);
        assert_eq!(item.title, "Hello");
        assert_eq!(item.url, "https://example.com/x");
    }

    #[test]
    fn parse_hn_item_falls_back_to_discuss_url() {
        let v = json!({"id": 7, "title": "Ask HN"});
        let item = parse_hn_item(&v).unwrap();
        assert_eq!(item.url, "https://news.ycombinator.com/item?id=7");
    }

    #[test]
    fn parse_hn_item_drops_missing_fields() {
        assert!(parse_hn_item(&json!({"id": 1})).is_none());
        assert!(parse_hn_item(&json!({"title": "x"})).is_none());
        assert!(parse_hn_item(&json!({"id": 1, "title": ""})).is_none());
    }

    #[test]
    fn parse_geeknews_rss_extracts_multiple_items() {
        let xml = r#"<rss><channel>
            <item><title>First post</title><link>https://news.hada.io/post/1</link></item>
            <item><title>Second</title><link>https://news.hada.io/post/2</link></item>
        </channel></rss>"#;
        let items = parse_geeknews_rss(xml);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "First post");
        assert_eq!(items[0].link, "https://news.hada.io/post/1");
        assert_eq!(items[1].link, "https://news.hada.io/post/2");
    }

    #[test]
    fn parse_geeknews_rss_ignores_malformed_items() {
        let xml = r#"<rss><channel>
            <item><title>ok</title><link>https://x</link></item>
            <item><title>missing link</title></item>
            <item><link>https://missing-title</link></item>
        </channel></rss>"#;
        let items = parse_geeknews_rss(xml);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn parse_geeknews_rss_handles_empty() {
        assert!(parse_geeknews_rss("").is_empty());
        assert!(parse_geeknews_rss("<rss><channel></channel></rss>").is_empty());
    }
}
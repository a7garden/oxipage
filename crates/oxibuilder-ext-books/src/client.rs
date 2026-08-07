//! 외부 도서 API 클라이언트 (doc/02 §2.10).
//!
//! 우선순위:
//!   1. 알라딘 OpenAPI (`OXIBUILDER_ALADIN_TTBKEY` env)
//!   2. Google Books API (키 불필요, rate limit 있음)
//!
//! `from_env()`는 매 핸들러 호출마다 새 인스턴스를 만든다. 사이트는 1인 운영이고
//! 환경변수가 토글될 수 있으므로 캐싱하지 않는다. 내부 reqwest::Client는
//! connection pool을 재사용하기 위해 한 번 만들어 보관한다.

use crate::model::BookSearchResult;
use anyhow::{Context, anyhow};
use serde::Deserialize;

const ALADIN_ENDPOINT: &str = "http://www.aladin.co.kr/ttb/api/ItemSearch.aspx";
const GOOGLE_BOOKS_ENDPOINT: &str = "https://www.googleapis.com/books/v1/volumes";

pub struct BooksClient {
    http: reqwest::Client,
    aladin_key: Option<String>,
}

impl BooksClient {
    pub fn from_env() -> Self {
        let aladin_key = std::env::var("OXIBUILDER_ALADIN_TTBKEY")
            .ok()
            .filter(|v| !v.is_empty());
        let http = reqwest::Client::builder()
            .user_agent(concat!("oxibuilder-ext-books/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http, aladin_key }
    }

    pub fn aladin_enabled(&self) -> bool {
        self.aladin_key.is_some()
    }

    /// 알라딘 키가 있으면 알라딘 우선 시도. 결과가 있으면 그대로 반환, 없거나
    /// 실패하면 Google Books로 폴백. 둘 다 비어있으면 빈 vec.
    pub async fn search(&self, q: &str, limit: usize) -> anyhow::Result<Vec<BookSearchResult>> {
        if q.trim().is_empty() {
            return Ok(Vec::new());
        }
        if let Some(key) = &self.aladin_key {
            match self.search_aladin(key, q, limit).await {
                Ok(v) if !v.is_empty() => return Ok(v),
                Ok(_) => {
                    tracing::debug!(
                        query = q,
                        "aladin returned no results; falling back to google_books"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = ?e, query = q, "aladin search failed; falling back to google_books");
                }
            }
        }
        self.search_google(q, limit).await
    }

    /// 1순위: 알라딘 OpenAPI. QueryType=Title 고정 (제목 검색이 가장 흔함).
    async fn search_aladin(
        &self,
        key: &str,
        q: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<BookSearchResult>> {
        let resp = self
            .http
            .get(ALADIN_ENDPOINT)
            .query(&[
                ("TTBKey", key),
                ("Query", q),
                ("QueryType", "Title"),
                ("MaxResults", &limit.min(20).to_string()),
                ("Output", "js"),
                ("Version", "20131101"),
            ])
            .send()
            .await
            .context("aladin request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("aladin http {}", resp.status()));
        }
        let body: AladinResponse = resp.json().await.context("aladin decode failed")?;
        Ok(body
            .item
            .into_iter()
            .map(|i| BookSearchResult {
                source: "aladin".to_string(),
                external_id: Some(i.item_id.map(|n| n.to_string()).unwrap_or_default()),
                isbn13: normalize_isbn13(i.isbn13),
                title: i.title.unwrap_or_default(),
                author: i.author,
                cover_image_url: i.cover,
                category: i.category_name,
                publisher: i.publisher,
                page_count: i.sub_info.and_then(|s| s.item_page),
            })
            .collect())
    }

    /// 폴백: Google Books API (키 없이도 동작, rate limit 있음).
    async fn search_google(&self, q: &str, limit: usize) -> anyhow::Result<Vec<BookSearchResult>> {
        let resp = self
            .http
            .get(GOOGLE_BOOKS_ENDPOINT)
            .query(&[
                ("q", q),
                ("maxResults", &limit.min(20).to_string()),
                ("printType", "books"),
            ])
            .send()
            .await
            .context("google_books request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("google_books http {}", resp.status()));
        }
        let body: GoogleBooksResponse = resp.json().await.context("google_books decode failed")?;
        Ok(body
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|it| {
                let v = it.volume_info;
                BookSearchResult {
                    source: "google_books".to_string(),
                    external_id: Some(it.id),
                    isbn13: pick_isbn13(v.industry_identifiers.as_deref()),
                    title: v.title.unwrap_or_default(),
                    author: v.authors.and_then(|a| a.into_iter().next()),
                    cover_image_url: v.image_links.and_then(|im| im.thumbnail),
                    category: v.categories.and_then(|c| c.into_iter().next()),
                    publisher: v.publisher,
                    page_count: v.page_count,
                }
            })
            .collect())
    }
}

/// 알라딘 isbn13은 13자리 문자열이거나 공백 포함 문자열로 올 수 있다.
fn normalize_isbn13(s: Option<String>) -> Option<String> {
    let t = s?;
    let cleaned: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if cleaned.len() == 13 {
        Some(cleaned)
    } else {
        None
    }
}

/// Google Books industryIdentifiers: [{type: "ISBN_13", identifier: "..."}, ...].
fn pick_isbn13(ids: Option<&[IndustryIdentifier]>) -> Option<String> {
    let arr = ids?;
    for id in arr {
        if id.type_.as_deref() == Some("ISBN_13") {
            return Some(id.identifier.clone());
        }
    }
    None
}

// ─── Wire types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AladinResponse {
    #[serde(default)]
    item: Vec<AladinItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AladinItem {
    item_id: Option<i64>,
    title: Option<String>,
    author: Option<String>,
    cover: Option<String>,
    isbn13: Option<String>,
    category_name: Option<String>,
    publisher: Option<String>,
    sub_info: Option<AladinSubInfo>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AladinSubInfo {
    item_page: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GoogleBooksResponse {
    items: Option<Vec<GoogleBookItem>>,
}

#[derive(Debug, Deserialize)]
struct GoogleBookItem {
    id: String,
    #[serde(rename = "volumeInfo")]
    volume_info: GoogleVolumeInfo,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GoogleVolumeInfo {
    title: Option<String>,
    authors: Option<Vec<String>>,
    #[serde(rename = "imageLinks")]
    image_links: Option<GoogleImageLinks>,
    #[serde(rename = "industryIdentifiers")]
    industry_identifiers: Option<Vec<IndustryIdentifier>>,
    categories: Option<Vec<String>>,
    publisher: Option<String>,
    page_count: Option<i64>,
}
#[derive(Debug, Deserialize, Default)]
struct GoogleImageLinks {
    thumbnail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IndustryIdentifier {
    #[serde(rename = "type")]
    type_: Option<String>,
    identifier: String,
}

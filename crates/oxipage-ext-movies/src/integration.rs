//! TMDB 연동 (doc/02 §2.9).
//!
//! `OXIPAGE_TMDB_KEY` 환경변수에서 키를 읽는다. 미설정이면 `enabled()=false`.
//! 검색/메타 fetch 모두 동일 클라이언트에서 처리.

use crate::model::TmdbSearchResult;
use anyhow::{Context, anyhow};
use serde::Deserialize;

const SEARCH_URL: &str = "https://api.themoviedb.org/3/search/movie";
const MOVIE_URL: &str = "https://api.themoviedb.org/3/movie";
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w500";

/// TMDB 클라이언트. 키가 있으면 활성화, 없으면 manual 모드.
#[derive(Clone)]
pub struct TmdbClient {
    http: reqwest::Client,
    api_key: Option<String>,
}

impl TmdbClient {
    pub fn from_env() -> Self {
        // reqwest의 기본 client는 connect timeout이 길다.
        // 1인 사이트 + TMDB 응답이 느려도 멈추지 않을 정도의 합리적 timeout을 둔다.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("oxipage-ext-movies/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builder");
        let api_key = std::env::var("OXIPAGE_TMDB_KEY").ok().filter(|s| !s.is_empty());
        Self { http, api_key }
    }

    pub fn enabled(&self) -> bool {
        self.api_key.is_some()
    }

    /// `/search/movie?query={q}&language=ko-KR`.
    /// 페이지당 20개, 첫 페이지만. total_pages는 무시 (1인 사이트 1쿼리 = 1페이지 충분).
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<TmdbSearchResult>> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("TMDB API key not configured"))?;
        let resp = self
            .http
            .get(SEARCH_URL)
            .query(&[
                ("api_key", key),
                ("query", query),
                ("language", "ko-KR"),
                ("page", "1"),
            ])
            .send()
            .await
            .context("TMDB search request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!("TMDB search returned status {}", resp.status()));
        }
        let body: SearchResponse = resp.json().await.context("TMDB search parse failed")?;
        Ok(body
            .results
            .into_iter()
            .map(|r| {
                let poster_path = if r.poster_path.is_empty() {
                    None
                } else {
                    Some(r.poster_path.clone())
                };
                let poster_url = poster_path.as_ref().map(|p| format!("{POSTER_BASE}{p}"));
                TmdbSearchResult {
                    tmdb_id: r.id,
                    title: r.title,
                    poster_path,
                    release_year: r.release_date.as_deref().and_then(parse_year),
                    poster_url,
                }
            })
            .collect())
    }

    /// `/movie/{id}?language=ko-KR` — 메타 1회 fetch.
    /// rating/overview 등도 함께 오지만 우리는 title/poster/release_year만 쓴다.
    pub async fn fetch_movie(&self, tmdb_id: i64) -> anyhow::Result<TmdbSearchResult> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("TMDB API key not configured"))?;
        let url = format!("{MOVIE_URL}/{tmdb_id}");
        let resp = self
            .http
            .get(&url)
            .query(&[("api_key", key), ("language", "ko-KR")])
            .send()
            .await
            .context("TMDB movie fetch request failed")?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "TMDB movie fetch returned status {} for id={tmdb_id}",
                resp.status()
            ));
        }
        let body: MovieDetail = resp.json().await.context("TMDB movie parse failed")?;
        let poster_path = if body.poster_path.is_empty() {
            None
        } else {
            Some(body.poster_path.clone())
        };
        let poster_url = poster_path.as_ref().map(|p| format!("{POSTER_BASE}{p}"));
        Ok(TmdbSearchResult {
            tmdb_id: body.id,
            title: body.title,
            poster_path,
            release_year: body.release_date.as_deref().and_then(parse_year),
            poster_url,
        })
    }
}

/// "2002-07-15" → 2002. 빈 문자열/파싱 실패는 None.
fn parse_year(date: &str) -> Option<i32> {
    date.get(..4).and_then(|y| y.parse().ok())
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    id: i64,
    title: String,
    poster_path: String,
    #[serde(default)]
    release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MovieDetail {
    id: i64,
    title: String,
    #[serde(default)]
    poster_path: String,
    #[serde(default)]
    release_date: Option<String>,
}

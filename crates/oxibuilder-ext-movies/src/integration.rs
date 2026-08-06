//! TMDB 연동 (doc/02 §2.9).
//!
//! `OXIBUILDER_TMDB_KEY` 환경변수에서 키를 읽는다. 미설정이면 `enabled()=false`.
//! - `search()`: 검색 피커용 (ko-KR 제목만).
//! - `fetch_movie_full()`: 생성 시 풍부한 메타 — ko/en 제목, 장르(쌍),
//!   출연진/감독, 런타임. 두 요청(ko-KR / en-US+credits)으로 조립.

use crate::model::{GenreName, PersonInput};
use anyhow::{Context, anyhow};
use serde::Deserialize;
use std::collections::HashMap;

const SEARCH_URL: &str = "https://api.themoviedb.org/3/search/movie";
const MOVIE_URL: &str = "https://api.themoviedb.org/3/movie";
const POSTER_BASE: &str = "https://image.tmdb.org/t/p/w500";

/// TMDB 클라이언트. 키가 있으면 활성화, 없으면 manual 모드.
#[derive(Clone)]
pub struct TmdbClient {
    http: reqwest::Client,
    api_key: Option<String>,
}

/// 풍부한 메타 fetch 결과. 핸들러가 엔트리 입력과 머지한다.
#[derive(Debug, Clone, Default)]
pub struct MovieMeta {
    pub title_ko: Option<String>,
    pub title_en: Option<String>,
    pub poster_path: Option<String>,
    pub release_year: Option<i32>,
    pub runtime_min: Option<i32>,
    pub genres: Vec<GenreName>,
    pub cast: Vec<PersonInput>,
    pub directors: Vec<PersonInput>,
}

impl TmdbClient {
    pub fn from_env() -> Self {
        // reqwest의 기본 client는 connect timeout이 길다.
        // 1인 사이트 + TMDB 응답이 느려도 멈추지 않을 정도의 합리적 timeout을 둔다.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("oxibuilder-ext-movies/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builder");
        let api_key = std::env::var("OXIBUILDER_TMDB_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        Self { http, api_key }
    }

    pub fn enabled(&self) -> bool {
        self.api_key.is_some()
    }

    /// `/search/movie?query={q}&language=ko-KR`.
    /// 페이지당 20개, 첫 페이지만. total_pages는 무시 (1인 사이트 1쿼리 = 1페이지 충분).
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<crate::model::TmdbSearchResult>> {
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
                crate::model::TmdbSearchResult {
                    tmdb_id: r.id,
                    title: r.title,
                    poster_path,
                    release_year: r.release_date.as_deref().and_then(parse_year),
                    poster_url,
                    media_type: "movie".to_string(),
                }
            })
            .collect())
    }

    /// `/movie/{id}` ko-KR + en-US(credits) 두 요청으로 풍부한 메타 조립.
    /// 한쪽 실패해도 가능한 만큼 채운다 (1인 사이트, 생성 1회).
    pub async fn fetch_movie_full(&self, tmdb_id: i64) -> anyhow::Result<MovieMeta> {
        if !self.enabled() {
            return Err(anyhow!("TMDB API key not configured"));
        }

        // 1) ko-KR: 한국어 제목, 런타임, 장르(ko), 포스터, 개봉년.
        let ko: Option<MovieDetailKo> = self
            .fetch_detail(tmdb_id, "ko-KR", &[])
            .await
            .map_err(|e| {
                tracing::warn!(error = ?e, tmdb_id, "TMDB ko-KR detail fetch failed");
            })
            .ok();

        // 2) en-US + credits: 영어 제목, 장르(en), 출연진/감독.
        let en: Option<MovieDetailEn> = self
            .fetch_detail(tmdb_id, "en-US", &[("append_to_response", "credits")])
            .await
            .map_err(|e| {
                tracing::warn!(error = ?e, tmdb_id, "TMDB en-US detail fetch failed");
            })
            .ok();

        let (ko, en) = match (ko, en) {
            (Some(k), Some(e)) => (Some(k), Some(e)),
            (Some(k), None) => (Some(k), None),
            (None, Some(e)) => (None, Some(e)),
            (None, None) => {
                return Err(anyhow!(
                    "TMDB detail fetch failed for id={tmdb_id} (both locales)"
                ));
            }
        };

        // 장르 쌍 조립: en 을 정규키로, ko 를 id 로 조인.
        let en_genres: HashMap<i64, String> = en
            .as_ref()
            .map(|e| e.genres.iter().map(|g| (g.id, g.name.clone())).collect())
            .unwrap_or_default();
        let genres: Vec<GenreName> = if let Some(k) = &ko {
            k.genres
                .iter()
                .map(|g| GenreName {
                    name_en: en_genres
                        .get(&g.id)
                        .cloned()
                        .unwrap_or_else(|| g.name.clone()),
                    name_ko: Some(g.name.clone()),
                })
                .collect()
        } else {
            // ko 가 없으면 en 만.
            en.as_ref()
                .map(|e| {
                    e.genres
                        .iter()
                        .map(|g| GenreName {
                            name_en: g.name.clone(),
                            name_ko: None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        // 출연진: en credits 의 cast (TMDB 인물명은 비현지화 → name_en).
        let cast = en
            .as_ref()
            .and_then(|e| e.credits.as_ref())
            .map(|c| {
                let mut list: Vec<PersonInput> = c
                    .cast
                    .iter()
                    .take(15)
                    .map(|p| PersonInput {
                        tmdb_person_id: Some(p.id),
                        slug: None,
                        name_en: Some(p.name.clone()),
                        name_ko: None,
                        profile_path: non_empty(&p.profile_path).map(String::from),
                        role: Some("actor".into()),
                        character_name: non_empty(&p.character).map(String::from),
                        billing: p.order,
                    })
                    .collect();
                list.sort_by_key(|p| p.billing.unwrap_or(i32::MAX));
                list
            })
            .unwrap_or_default();

        // 감독: crew where job == "Director".
        let directors = en
            .as_ref()
            .and_then(|e| e.credits.as_ref())
            .map(|c| {
                c.crew
                    .iter()
                    .filter(|p| p.job == "Director")
                    .take(5)
                    .map(|p| PersonInput {
                        tmdb_person_id: Some(p.id),
                        slug: None,
                        name_en: Some(p.name.clone()),
                        name_ko: None,
                        profile_path: non_empty(&p.profile_path).map(String::from),
                        role: Some("director".into()),
                        character_name: None,
                        billing: None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let poster_path = ko
            .as_ref()
            .and_then(|k| non_empty(&k.poster_path).map(String::from))
            .or_else(|| {
                en.as_ref()
                    .and_then(|e| non_empty(&e.poster_path).map(String::from))
            });

        Ok(MovieMeta {
            title_ko: ko
                .as_ref()
                .and_then(|k| non_empty(&k.title).map(String::from)),
            title_en: en
                .as_ref()
                .and_then(|e| non_empty(&e.title).map(String::from)),
            poster_path,
            release_year: ko
                .as_ref()
                .and_then(|k| k.release_date.as_deref().and_then(parse_year))
                .or_else(|| {
                    en.as_ref()
                        .and_then(|e| e.release_date.as_deref().and_then(parse_year))
                }),
            runtime_min: ko
                .as_ref()
                .and_then(|k| k.runtime)
                .or_else(|| en.as_ref().and_then(|e| e.runtime)),
            genres,
            cast,
            directors,
        })
    }

    async fn fetch_detail<D: serde::de::DeserializeOwned>(
        &self,
        tmdb_id: i64,
        language: &str,
        extra: &[(&str, &str)],
    ) -> anyhow::Result<D> {
        let key = self.api_key.as_deref().expect("api key present");
        let url = format!("{MOVIE_URL}/{tmdb_id}");
        let mut q = vec![("api_key", key), ("language", language)];
        q.extend_from_slice(extra);
        let resp = self
            .http
            .get(&url)
            .query(&q)
            .send()
            .await
            .with_context(|| format!("TMDB detail request failed (lang={language})"))?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "TMDB detail returned status {} for id={tmdb_id} (lang={language})",
                resp.status()
            ));
        }
        resp.json::<D>().await.context("TMDB detail parse failed")
    }
}

/// "2002-07-15" → 2002. 빈 문자열/파싱 실패는 None.
fn parse_year(date: &str) -> Option<i32> {
    date.get(..4).and_then(|y| y.parse().ok())
}

fn non_empty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
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
struct GenreDto {
    id: i64,
    name: String,
}

#[derive(Debug, Deserialize)]
struct MovieDetailKo {
    #[serde(default)]
    title: String,
    #[serde(default)]
    poster_path: String,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    runtime: Option<i32>,
    #[serde(default)]
    genres: Vec<GenreDto>,
}

#[derive(Debug, Deserialize)]
struct MovieDetailEn {
    #[serde(default)]
    title: String,
    #[serde(default)]
    poster_path: String,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    runtime: Option<i32>,
    #[serde(default)]
    genres: Vec<GenreDto>,
    #[serde(default)]
    credits: Option<CreditsDto>,
}

#[derive(Debug, Deserialize)]
struct CreditsDto {
    #[serde(default)]
    cast: Vec<CastDto>,
    #[serde(default)]
    crew: Vec<CrewDto>,
}

#[derive(Debug, Deserialize)]
struct CastDto {
    id: i64,
    name: String,
    #[serde(default)]
    character: String,
    #[serde(default)]
    order: Option<i32>,
    #[serde(default)]
    profile_path: String,
}

#[derive(Debug, Deserialize)]
struct CrewDto {
    id: i64,
    name: String,
    #[serde(default)]
    job: String,
    #[serde(default)]
    profile_path: String,
}

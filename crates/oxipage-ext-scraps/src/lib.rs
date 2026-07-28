pub mod integration;
pub mod model;
pub mod repo;
pub mod routes;
use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::scheduler::ScheduledJob;
use oxipage_core::state::AppState;
use std::sync::Arc;

pub struct ScrapsExtension;

#[async_trait]
impl Extension for ScrapsExtension {
    fn id(&self) -> &'static str {
        "scraps"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["scrap_item"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "스크랩".to_string(),
            Lang::En => "Scraps".to_string(),
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            name: "init",
            sql: include_str!("../migrations/0001_init.sql"),
        }]
    }

    fn routes(&self) -> Router<AppState> {
        Router::new()
            .route(
                "/",
                get(routes::list_published).post(routes::create_manual),
            )
            .route("/queue", get(routes::list_queue))
            .route("/{id}", get(routes::show).patch(routes::update).delete(routes::delete))
            .route("/{id}/publish", post(routes::publish))
    }

    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        vec![Arc::new(ScrapCollectJob)]
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let items = repo::list(&ctx.db, true, None, 3).await.ok()?;
        let entries = items
            .into_iter()
            .map(|it| LobbyCardItem {
                title: it.title,
                url: format!("/scraps/{}", it.id),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items: entries,
        })
    }
}

/// 30분 주기로 HackerNews/GeekNews 후보를 큐에 채우는 잡 (doc/01 §1.9).
///
/// **구현 (doc/08 수정):** `ScheduledJob::run(&self, &AppState)` 시그니처로
/// DB pool에 접근해 실제 fetch/upsert를 수행한다. 이전엔 시그니처가
/// `run(&self)`라 구조적으로 no-op이었다.
struct ScrapCollectJob;

/// HN 후보 수집 상한. Firebase topstories는 ~500개 id를 반환하지만,
/// 큐는 "추천" 용도이므로 상위 N개만 가져온다 (doc/02 §2.7).
const HN_TOP_LIMIT: usize = 20;

#[async_trait]
impl ScheduledJob for ScrapCollectJob {
    fn schedule(&self) -> &str {
        "0 */30 * * * *"
    }

    fn name(&self) -> &str {
        "scraps_collect"
    }

    async fn run(&self, ctx: &AppState) -> anyhow::Result<()> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("oxipage-ext-scraps/", env!("CARGO_PKG_VERSION")))
            .build()?;

        let mut collected = 0usize;

        // HackerNews — 실패해도 GeekNews는 계속 (독립 소스).
        match fetch_hackernews(&http).await {
            Ok(items) => {
                for item in items {
                    match repo::upsert_queue_item(
                        &ctx.db,
                        "hackernews",
                        &item.id.to_string(),
                        &item.url,
                        &item.title,
                        None,
                    )
                    .await
                    {
                        Ok(_) => collected += 1,
                        Err(e) => {
                            tracing::warn!(hn_id = item.id, error = ?e, "HN upsert failed")
                        }
                    }
                }
            }
            Err(e) => tracing::warn!(error = ?e, "HackerNews fetch failed"),
        }

        // GeekNews RSS.
        match fetch_geeknews(&http).await {
            Ok(items) => {
                for item in items {
                    // GeekNews는 고유 숫자 id가 없어 link를 source_item_id로 쓴다.
                    match repo::upsert_queue_item(
                        &ctx.db,
                        "geeknews",
                        &item.link,
                        &item.link,
                        &item.title,
                        None,
                    )
                    .await
                    {
                        Ok(_) => collected += 1,
                        Err(e) => {
                            tracing::warn!(link = %item.link, error = ?e, "GeekNews upsert failed")
                        }
                    }
                }
            }
            Err(e) => tracing::warn!(error = ?e, "GeekNews fetch failed"),
        }

        tracing::info!(collected, "scraps collect tick completed");
        Ok(())
    }
}

/// HackerNews topstories → 단건 item fetch → 큐 후보.
async fn fetch_hackernews(
    http: &reqwest::Client,
) -> anyhow::Result<Vec<integration::HackerNewsItem>> {
    const TOPSTORIES: &str = "https://hacker-news.firebaseio.com/v0/topstories.json";
    const ITEM: &str = "https://hacker-news.firebaseio.com/v0/item";

    let ids: Vec<i64> = http.get(TOPSTORIES).send().await?.json().await?;
    let ids = integration::take_top_ids(&ids, HN_TOP_LIMIT);

    let mut out = Vec::new();
    for id in ids {
        let url = format!("{ITEM}/{id}.json");
        match http.get(&url).send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(value) => {
                    if let Some(item) = integration::parse_hn_item(&value) {
                        out.push(item);
                    }
                }
                Err(e) => tracing::debug!(hn_id = id, error = ?e, "HN item parse failed"),
            },
            Err(e) => tracing::debug!(hn_id = id, error = ?e, "HN item fetch failed"),
        }
    }
    Ok(out)
}

/// GeekNews RSS 피드 → 큐 후보.
async fn fetch_geeknews(
    http: &reqwest::Client,
) -> anyhow::Result<Vec<integration::GeekNewsItem>> {
    const RSS: &str = "https://news.hada.io/rss";
    let xml = http.get(RSS).send().await?.text().await?;
    Ok(integration::parse_geeknews_rss(&xml))
}

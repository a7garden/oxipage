pub mod integration;
pub mod model;
pub mod repo;
pub mod routes;
use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxibuilder_core::client::Client;

use oxibuilder_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxibuilder_core::extension::{
    CliArg, CliCommand, CliHandler, CliSubcommand, Extension, Lang, LobbyCard, LobbyCardItem,
    Migration,
};
use oxibuilder_core::scheduler::ScheduledJob;
use oxibuilder_core::state::AppState;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ── CLI handlers ──

struct ScrapAddHandler;
impl CliHandler for ScrapAddHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let url = args.get("url").cloned().unwrap_or_default();
        let title = args.get("title").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "url": url, "title": title });
        if let Some(tags) = args.get("tags") {
            body["tags"] = serde_json::json!(tags);
        }
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post("/api/console/scraps/", &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct ScrapQueueHandler;
impl CliHandler for ScrapQueueHandler {
    fn run(
        &self,
        _args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .get("/api/console/scraps/queue")
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct ScrapDeleteHandler;
impl CliHandler for ScrapDeleteHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let id = args.get("id").cloned().unwrap_or_default();
        let client = client.clone();
        Box::pin(async move {
            let path = format!("/api/console/scraps/{id}");
            let resp = client
                .delete(&path)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

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

    fn routes(&self) -> Router {
        Router::new()
            .route("/", get(routes::list_published).post(routes::create_manual))
            .route("/queue", get(routes::list_queue))
            .route(
                "/{id}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
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

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "scraps",
            about: "Manage scrapbook items",
            subcommands: vec![
                CliSubcommand {
                    name: "add",
                    about: "Add a scrap manually",
                    args: vec![
                        CliArg {
                            long: "url",
                            short: Some('u'),
                            help: "Source URL",
                            required: true,
                        },
                        CliArg {
                            long: "title",
                            short: Some('t'),
                            help: "Scrap title",
                            required: true,
                        },
                        CliArg {
                            long: "tags",
                            short: Some('g'),
                            help: "Comma-separated tags",
                            required: false,
                        },
                    ],
                    handler: Some(Arc::new(ScrapAddHandler)),
                },
                CliSubcommand {
                    name: "queue",
                    about: "List pending scrap queue",
                    args: vec![],
                    handler: Some(Arc::new(ScrapQueueHandler)),
                },
                CliSubcommand {
                    name: "delete",
                    about: "Delete a scrap by ID",
                    args: vec![CliArg {
                        long: "id",
                        short: None,
                        help: "Scrap item ID",
                        required: true,
                    }],
                    handler: Some(Arc::new(ScrapDeleteHandler)),
                },
            ],
        }]
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
    fn name(&self) -> &str {
        "scraps_collect"
    }

    async fn run(&self, ctx: &AppState) -> anyhow::Result<()> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("oxibuilder-ext-scraps/", env!("CARGO_PKG_VERSION")))
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
async fn fetch_geeknews(http: &reqwest::Client) -> anyhow::Result<Vec<integration::GeekNewsItem>> {
    const RSS: &str = "https://news.hada.io/rss";
    let xml = http.get(RSS).send().await?.text().await?;
    Ok(integration::parse_geeknews_rss(&xml))
}

impl BuildExt for ScrapsExtension {
    fn ext_id(&self) -> &'static str {
        "scraps"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let items: Vec<model::ScrapItem> = rt.block_on(repo::list(db, true, None, 200))?;
        let mut pages = Vec::with_capacity(items.len());
        for item in &items {
            let excerpt: String = item
                .note_ko
                .as_deref()
                .or(item.note_en.as_deref())
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            pages.push(StaticPage {
                path: format!("scraps/{}/index.html", item.id),
                content: format!(
                    r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
    <title>{title}</title><meta property="og:title" content="{title}"><meta property="og:description" content="{excerpt}">
    <meta property="og:type" content="website"><meta property="og:url" content="/scraps/{id}/">
    <link rel="canonical" href="/scraps/{id}/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
                    title=item.title, excerpt=excerpt, id=item.id),
            });
        }
        Ok(pages)
    }

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let items: Vec<model::ScrapItem> = rt.block_on(repo::list(db, true, None, 200))?;
        Ok(Box::new(items))
    }

    fn build_search_docs(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let items: Vec<model::ScrapItem> = rt.block_on(repo::list(db, true, None, 200))?;
        Ok(items
            .into_iter()
            .map(|s| {
                let body = s.note_ko.or(s.note_en).unwrap_or_default();
                let excerpt: String = body.chars().take(200).collect();
                SearchDoc {
                    id: format!("scraps/{}", s.id),
                    title: s.title,
                    body_preview: excerpt,
                    doc_type: "scraps".into(),
                    url: format!("/scraps/{}", s.id),
                    published_at: s.published_at,
                }
            })
            .collect())
    }
}

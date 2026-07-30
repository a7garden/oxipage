pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::client::Client;

use oxipage_core::extension::{
    CliArg, CliCommand, CliHandler, CliSubcommand, Extension, Lang, LobbyCard, LobbyCardItem,
    Migration,
};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct NovelsExtension;

// ── CLI handlers ──

struct NovelAddHandler;
impl CliHandler for NovelAddHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let title = args.get("title").cloned().unwrap_or_default();
        let slug = args.get("slug").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "title": title, "slug": slug });
        if let Some(genre) = args.get("genre") {
            body["genre"] = serde_json::json!(genre);
        }
        if let Some(synopsis) = args.get("synopsis") {
            body["synopsis"] = serde_json::json!(synopsis);
        }
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post("/api/console/novels/", &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct NovelListHandler;
impl CliHandler for NovelListHandler {
    fn run(
        &self,
        _args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .get("/api/console/novels/")
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct NovelChapterAddHandler;
impl CliHandler for NovelChapterAddHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let slug = args.get("slug").cloned().unwrap_or_default();
        let title = args.get("title").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "title": title });
        if let Some(order) = args.get("order") {
            body["order"] = serde_json::json!(order);
        }
        if let Some(content) = args.get("content") {
            body["content"] = serde_json::json!(content);
        }
        let client = client.clone();
        Box::pin(async move {
            let path = format!("/api/console/novels/{slug}/chapters");
            let resp = client
                .post(&path, &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

#[async_trait]
impl Extension for NovelsExtension {
    fn id(&self) -> &'static str {
        "novels"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["novel", "novel_chapter"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "소설".to_string(),
            Lang::En => "Novels".to_string(),
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
            .route("/", get(routes::list_novels).post(routes::create_novel))
            .route(
                "/{slug}",
                get(routes::show_novel).delete(routes::delete_novel),
            )
            .route("/{slug}/publish", post(routes::publish_novel))
            .route(
                "/{slug}/chapters",
                get(routes::list_chapters).post(routes::create_chapter),
            )
            .route("/{slug}/chapters/draft", get(routes::list_chapters_draft))
            .route(
                "/{slug}/chapters/{order}",
                get(routes::show_chapter)
                    .patch(routes::update_chapter)
                    .delete(routes::delete_chapter),
            )
            .route(
                "/{slug}/chapters/{order}/publish",
                post(routes::publish_chapter),
            )
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let novels = repo::list_novels(&ctx.db, false, 3).await.ok()?;
        let items = novels
            .into_iter()
            .map(|n| LobbyCardItem {
                title: n.title,
                url: format!("/novels/{}", n.slug),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "novels",
            about: "Manage novels and chapters",
            subcommands: vec![
                CliSubcommand {
                    name: "add",
                    about: "Create a new novel",
                    args: vec![
                        CliArg {
                            long: "title",
                            short: Some('t'),
                            help: "Novel title",
                            required: true,
                        },
                        CliArg {
                            long: "slug",
                            short: Some('s'),
                            help: "URL slug",
                            required: true,
                        },
                        CliArg {
                            long: "genre",
                            short: Some('g'),
                            help: "Genre tag",
                            required: false,
                        },
                        CliArg {
                            long: "synopsis",
                            short: None,
                            help: "Short description",
                            required: false,
                        },
                    ],
                    handler: Some(Arc::new(NovelAddHandler)),
                },
                CliSubcommand {
                    name: "list",
                    about: "List novels",
                    args: vec![],
                    handler: Some(Arc::new(NovelListHandler)),
                },
                CliSubcommand {
                    name: "chapter",
                    about: "Add a chapter to a novel",
                    args: vec![
                        CliArg {
                            long: "slug",
                            short: Some('s'),
                            help: "Novel slug",
                            required: true,
                        },
                        CliArg {
                            long: "title",
                            short: Some('t'),
                            help: "Chapter title",
                            required: true,
                        },
                        CliArg {
                            long: "order",
                            short: Some('o'),
                            help: "Chapter order number",
                            required: false,
                        },
                        CliArg {
                            long: "content",
                            short: Some('c'),
                            help: "Chapter content (plain text)",
                            required: false,
                        },
                    ],
                    handler: Some(Arc::new(NovelChapterAddHandler)),
                },
            ],
        }]
    }
}

impl BuildExt for NovelsExtension {
    fn ext_id(&self) -> &'static str {
        "novels"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let novels: Vec<model::Novel> = rt.block_on(repo::list_novels(db, false, 200))?;
        let mut pages = Vec::new();

        for novel in &novels {
            let excerpt: String = novel
                .synopsis
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            pages.push(StaticPage {
                path: format!("novels/{}/index.html", novel.slug),
                content: format!(
                    r#"<!DOCTYPE html>
    <html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
    <title>{title}</title><meta property="og:title" content="{title}">
    <meta property="og:description" content="{excerpt}"><meta property="og:type" content="website">
    <meta property="og:url" content="/novels/{slug}/"><link rel="canonical" href="/novels/{slug}/">
    </head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
                    title=novel.title, excerpt=excerpt, slug=novel.slug),
            });

            let chapters: Vec<model::NovelChapter> = rt
                .block_on(repo::list_chapters(db, &novel.slug, false))
                .unwrap_or_default();
            for ch in &chapters {
                pages.push(StaticPage {
                    path: format!("novels/{}/chapter-{}/index.html", novel.slug, ch.chapter_order),
                    content: format!(
                        r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
    <title>{novel} - {chapter}</title><link rel="canonical" href="/novels/{slug}/chapter-{order}/">
    </head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
                        novel=novel.title, chapter=ch.title, slug=novel.slug, order=ch.chapter_order),
                });
                pages.push(StaticPage {
                    path: format!(
                        "novels/{}/chapter-{}/index.md",
                        novel.slug, ch.chapter_order
                    ),
                    content: ch.body.clone(),
                });
            }
        }
        Ok(pages)
    }

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let novels: Vec<model::Novel> = rt.block_on(repo::list_novels(db, false, 200))?;
        Ok(Box::new(novels))
    }

    fn build_search_docs(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let novels: Vec<model::Novel> = rt.block_on(repo::list_novels(db, false, 200))?;
        Ok(novels
            .into_iter()
            .map(|n| {
                let excerpt: String = n
                    .synopsis
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect();
                SearchDoc {
                    id: format!("novels/{}", n.slug),
                    title: n.title,
                    body_preview: excerpt,
                    doc_type: "novels".into(),
                    url: format!("/novels/{}", n.slug),
                    published_at: n.published_at,
                }
            })
            .collect())
    }
}

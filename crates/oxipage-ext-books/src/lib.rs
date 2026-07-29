pub mod client;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::client::Client;

use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{
    CliArg, CliCommand, CliHandler, CliSubcommand, Extension, ExternalApiKey, ExternalKeyScope,
    Lang, LobbyCard, LobbyCardItem, Migration,
};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::error::Error;
use std::pin::Pin;
use std::sync::Arc;

pub struct BooksExtension;

// ── CLI handlers ──

struct BookAddHandler;
impl CliHandler for BookAddHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let title = args.get("title").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "title": title });
        if let Some(author) = args.get("author") {
            body["author"] = serde_json::json!(author);
        }
        if let Some(rating) = args.get("rating") {
            body["rating"] = serde_json::json!(rating);
        }
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post("/api/v1/books/", &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

#[async_trait]
impl Extension for BooksExtension {
    fn id(&self) -> &'static str {
        "books"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["book_entry"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "책".to_string(),
            Lang::En => "Books".to_string(),
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
            .route("/", get(routes::list).post(routes::create))
            .route("/search", get(routes::external_search))
            .route(
                "/{id}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{id}/publish", post(routes::publish))
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let recent = repo::list(&ctx.db, None, 3).await.ok()?;
        if recent.is_empty() {
            return None;
        }
        let items = recent
            .into_iter()
            .map(|b| LobbyCardItem {
                title: b.title,
                url: format!("/books/{}", b.id),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "books",
            about: "Manage book reviews",
            subcommands: vec![CliSubcommand {
                name: "add",
                about: "Add a book review",
                args: vec![
                    CliArg {
                        long: "title",
                        short: Some('t'),
                        help: "Book title",
                        required: true,
                    },
                    CliArg {
                        long: "author",
                        short: Some('a'),
                        help: "Book author",
                        required: false,
                    },
                    CliArg {
                        long: "rating",
                        short: Some('r'),
                        help: "Rating (1-10)",
                        required: false,
                    },
                ],
                handler: Some(Arc::new(BookAddHandler)),
            }],
        }]
    }

    fn external_api_keys(&self) -> Vec<ExternalApiKey> {
        vec![ExternalApiKey {
            id: "aladin_key",
            label_ko: "알라딘 TTBKey",
            label_en: "Aladin TTBKey",
            env_var: "OXIPAGE_ALADIN_TTBKEY",
            required: false,
            scope: ExternalKeyScope::ExtensionConfig,
        }]
    }
}

impl BuildExt for BooksExtension {
    fn ext_id(&self) -> &'static str {
        "books"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let books: Vec<model::Book> = handle.block_on(repo::list(db, None, 200))?;
        let mut pages = Vec::with_capacity(books.len());
        for b in &books {
            let excerpt: String = b
                .review_ko
                .as_deref()
                .or(b.review_en.as_deref())
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            pages.push(StaticPage {
                path: format!("books/{}/index.html", b.id),
                content: format!(
                    r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>{title}</title><meta property="og:title" content="{title}"><meta property="og:description" content="{excerpt}">
<meta property="og:type" content="website"><meta property="og:url" content="/books/{id}/">
<link rel="canonical" href="/books/{id}/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
                    title=b.title, excerpt=excerpt, id=b.id),
            });
        }
        Ok(pages)
    }

    fn build_data(
        &self,
        db: &SqlitePool,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let books: Vec<model::Book> = handle.block_on(repo::list(db, None, 200))?;
        Ok(Box::new(books))
    }

    fn build_search_docs(
        &self,
        db: &SqlitePool,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let books: Vec<model::Book> = handle.block_on(repo::list(db, None, 200))?;
        Ok(books
            .into_iter()
            .map(|b| {
                let body = b.review_ko.or(b.review_en).unwrap_or_default();
                let excerpt: String = body.chars().take(200).collect();
                SearchDoc {
                    id: format!("books/{}", b.id),
                    title: b.title,
                    body_preview: excerpt,
                    doc_type: "books".into(),
                    url: format!("/books/{}", b.id),
                    published_at: b.finished_at,
                }
            })
            .collect())
    }
}

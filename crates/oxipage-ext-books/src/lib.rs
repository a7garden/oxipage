pub mod client;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::client::Client;
use oxipage_core::extension::{CliArg, CliHandler, CliCommand, CliSubcommand, Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct BooksExtension;

// ── CLI handlers ──

struct BookAddHandler;
impl CliHandler for BookAddHandler {
    fn run(&self, args: BTreeMap<String, String>, client: &Client)
        -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
    {
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
            let resp = client.post("/api/v1/books/", &body).await
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
            subcommands: vec![
                CliSubcommand {
                    name: "add",
                    about: "Add a book review",
                    args: vec![
                        CliArg { long: "title", short: Some('t'), help: "Book title", required: true },
                        CliArg { long: "author", short: Some('a'), help: "Book author", required: false },
                        CliArg { long: "rating", short: Some('r'), help: "Rating (1-10)", required: false },
                    ],
                    handler: Some(Arc::new(BookAddHandler)),
                },
            ],
        }]
    }
}

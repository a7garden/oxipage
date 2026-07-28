pub mod integration;
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

pub struct MoviesExtension;

// ── CLI handlers ──

struct MovieAddHandler;
impl CliHandler for MovieAddHandler {
    fn run(&self, args: BTreeMap<String, String>, client: &Client)
        -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
    {
        let title = args.get("title").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "title": title });
        if let Some(slug) = args.get("slug") {
            body["slug"] = serde_json::json!(slug);
        }
        if let Some(rating) = args.get("rating") {
            body["rating"] = serde_json::json!(rating);
        }
        let client = client.clone();
        Box::pin(async move {
            let resp = client.post("/api/v1/movies/", &body).await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct MovieSeriesCreateHandler;
impl CliHandler for MovieSeriesCreateHandler {
    fn run(&self, args: BTreeMap<String, String>, client: &Client)
        -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>>
    {
        let name = args.get("name").cloned().unwrap_or_default();
        let slug = args.get("slug").cloned().unwrap_or_default();
        let body = serde_json::json!({ "name": name, "slug": slug });
        let client = client.clone();
        Box::pin(async move {
            let resp = client.post("/api/v1/movies/series", &body).await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

#[async_trait]
impl Extension for MoviesExtension {
    fn id(&self) -> &'static str {
        "movies"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["series_group", "movie_entry"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "영화".to_string(),
            Lang::En => "Movies".to_string(),
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
            .route("/search", get(routes::tmdb_search))
            .route(
                "/{slug}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{slug}/publish", post(routes::publish))
            .route("/series", get(routes::list_groups).post(routes::create_group))
            .route("/series/{slug}", get(routes::show_group))
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let entries = repo::list_entries_published(&ctx.db, None, 3).await.ok()?;
        let items = entries
            .into_iter()
            .map(|e| LobbyCardItem {
                title: e.title,
                url: format!("/movies/{}", e.slug),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "movies",
            about: "Manage movies and series",
            subcommands: vec![
                CliSubcommand {
                    name: "add",
                    about: "Add a movie review",
                    args: vec![
                        CliArg { long: "title", short: Some('t'), help: "Movie title", required: true },
                        CliArg { long: "slug", short: Some('s'), help: "URL slug", required: false },
                        CliArg { long: "rating", short: Some('r'), help: "Rating (1-10)", required: false },
                    ],
                    handler: Some(Arc::new(MovieAddHandler)),
                },
                CliSubcommand {
                    name: "series",
                    about: "Create a series group",
                    args: vec![
                        CliArg { long: "name", short: Some('n'), help: "Series name", required: true },
                        CliArg { long: "slug", short: Some('s'), help: "URL slug", required: true },
                    ],
                    handler: Some(Arc::new(MovieSeriesCreateHandler)),
                },
            ],
        }]
    }
}

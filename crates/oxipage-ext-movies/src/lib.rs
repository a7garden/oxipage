pub mod integration;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct MoviesExtension;

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
            // MovieEntry
            .route("/", get(routes::list).post(routes::create))
            .route("/search", get(routes::tmdb_search))
            .route(
                "/{slug}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{slug}/publish", post(routes::publish))
            // SeriesGroup
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
}

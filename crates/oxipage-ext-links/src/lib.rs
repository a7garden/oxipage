pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct LinksExtension;

#[async_trait]
impl Extension for LinksExtension {
    fn id(&self) -> &'static str {
        "links"
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "링크".to_string(),
            Lang::En => "Links".to_string(),
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
            .route(
                "/{id}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let cards = repo::list(&ctx.db, Some(true), 5).await.ok()?;
        if cards.is_empty() {
            return None;
        }
        let items = cards
            .into_iter()
            .map(|c| LobbyCardItem {
                title: c.title,
                url: c.url,
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }
}

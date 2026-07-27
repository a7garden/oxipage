pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct BlogExtension;

#[async_trait]
impl Extension for BlogExtension {
    fn id(&self) -> &'static str {
        "blog"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["blog_post"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "블로그".to_string(),
            Lang::En => "Blog".to_string(),
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
                "/{slug}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{slug}/publish", post(routes::publish))
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let posts = repo::list(&ctx.db, false, None, 3).await.ok()?;
        let items = posts
            .into_iter()
            .map(|p| LobbyCardItem {
                title: p.title,
                url: format!("/blog/{}", p.slug),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }
}

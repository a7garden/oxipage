pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct NovelsExtension;

#[async_trait]
impl Extension for NovelsExtension {
    fn id(&self) -> &'static str {
        "novels"
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

    fn routes(&self) -> Router<AppState> {
        Router::new()
            // Novel
            .route("/", get(routes::list_novels).post(routes::create_novel))
            .route("/{slug}", get(routes::show_novel).delete(routes::delete_novel))
            .route("/{slug}/publish", post(routes::publish_novel))
            // Chapter
            .route("/{slug}/chapters", get(routes::list_chapters).post(routes::create_chapter))
            .route("/{slug}/chapters/draft", get(routes::list_chapters_draft))
            .route(
                "/{slug}/chapters/{order}",
                get(routes::show_chapter)
                    .patch(routes::update_chapter)
                    .delete(routes::delete_chapter),
            )
            .route("/{slug}/chapters/{order}/publish", post(routes::publish_chapter))
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
}

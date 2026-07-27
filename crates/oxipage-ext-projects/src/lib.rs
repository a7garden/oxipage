pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;

pub struct ProjectsExtension;

#[async_trait]
impl Extension for ProjectsExtension {
    fn id(&self) -> &'static str {
        "projects"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["project", "screenshot"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "프로젝트".to_string(),
            Lang::En => "Projects".to_string(),
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
                get(routes::list).post(routes::create),
            )
            .route(
                "/{slug}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{slug}/publish", post(routes::publish))
            .route("/{slug}/screenshots", post(routes::add_screenshot))
            .route(
                "/{slug}/screenshots/{sid}",
                axum::routing::delete(routes::delete_screenshot),
            )
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let projects = repo::list(&ctx.db, None, 3).await.ok()?;
        let items = projects
            .into_iter()
            .map(|p| LobbyCardItem {
                title: p.title_en.or(p.title_ko).unwrap_or_default(),
                url: format!("/projects/{}", p.slug),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }
}

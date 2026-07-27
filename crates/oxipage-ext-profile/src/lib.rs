pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxipage_core::state::AppState;

pub struct ProfileExtension;

#[async_trait]
impl Extension for ProfileExtension {
    fn id(&self) -> &'static str {
        "profile"
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "프로필".to_string(),
            Lang::En => "Profile".to_string(),
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
        Router::new().route("/", get(routes::get_profile).put(routes::put_profile))
    }

    async fn lobby_summary(&self, _ctx: &AppState) -> Option<LobbyCard> {
        None
    }

    async fn on_startup(&self, ctx: &AppState) -> anyhow::Result<()> {
        repo::ensure_singleton(&ctx.db, &ctx.config.site.name).await
    }
}

pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{Extension, Lang, LobbyCard, Migration};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::error::Error;

pub struct ProfileExtension;

#[async_trait]
impl Extension for ProfileExtension {
    fn id(&self) -> &'static str {
        "profile"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["profile"]
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

impl BuildExt for ProfileExtension {
    fn ext_id(&self) -> &'static str {
        "profile"
    }

    fn build_pages(&self, db: &SqlitePool) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let profile: Option<model::Profile> = handle.block_on(repo::get(db))?;

        let Some(profile) = profile else {
            return Ok(vec![]);
        };

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{name}</title>
  <meta property="og:title" content="{name}">
  <meta property="og:type" content="profile">
  <meta property="og:url" content="/profile/">
  <link rel="canonical" href="/profile/">
</head>
<body>
  <div id="root"></div>
  <script src="/assets/index.js"></script>
</body>
</html>
"#,
            name = profile.display_name
        );

        Ok(vec![StaticPage {
            path: "profile/index.html".to_string(),
            content: html,
        }])
    }

    fn build_data(&self, db: &SqlitePool) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let profile: Option<model::Profile> = handle.block_on(repo::get(db))?;
        Ok(Box::new(profile))
    }

    fn build_search_docs(&self, _db: &SqlitePool) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        Ok(vec![])
    }
}

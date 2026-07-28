pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;

use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use std::error::Error;
use sqlx::SqlitePool;
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;
use tokio::runtime::Handle;

pub struct LinksExtension;

#[async_trait]
impl Extension for LinksExtension {
    fn id(&self) -> &'static str {
        "links"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["link_card"]
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

impl BuildExt for LinksExtension {
    fn ext_id(&self) -> &'static str { "links" }

    fn build_pages(&self, db: &SqlitePool) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let handle = Handle::current();
        let cards: Vec<model::LinkCard> = handle.block_on(repo::list(db, None, 500))?;
        let _html = cards.iter().map(|c| {
            format!("<li><a href=\"{}\">{}</a></li>", c.url, c.title)
        }).collect::<Vec<_>>().join("\n");
        Ok(vec![StaticPage {
            path: "links/index.html".into(),
            content: r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Links</title><link rel="canonical" href="/links/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#.to_string(),
        }])
    }

    fn build_data(&self, db: &SqlitePool) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let handle = Handle::current();
        let cards: Vec<model::LinkCard> = handle.block_on(repo::list(db, None, 500))?;
        Ok(Box::new(cards))
    }

    fn build_search_docs(&self, _db: &SqlitePool) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        Ok(vec![])
    }
}

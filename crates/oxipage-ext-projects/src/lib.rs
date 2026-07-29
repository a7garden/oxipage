pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::error::Error;

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
            .route("/", get(routes::list).post(routes::create))
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

impl BuildExt for ProjectsExtension {
    fn ext_id(&self) -> &'static str {
        "projects"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let projects: Vec<model::Project> = rt.block_on(repo::list(db, None, 200))?;

        let mut pages = Vec::with_capacity(projects.len());

        for p in &projects {
            let title = p
                .title_en
                .as_deref()
                .or(p.title_ko.as_deref())
                .unwrap_or("");
            let desc = p
                .description_en
                .as_deref()
                .or(p.description_ko.as_deref())
                .unwrap_or("");
            let excerpt: String = desc.chars().take(160).collect();

            pages.push(StaticPage {
                path: format!("projects/{}/index.html", p.slug),
                content: format!(
                    r#"<!DOCTYPE html>
    <html lang="ko">
    <head>
      <meta charset="UTF-8">
      <meta name="viewport" content="width=device-width, initial-scale=1.0">
      <title>{title}</title>
      <meta property="og:title" content="{title}">
      <meta property="og:description" content="{excerpt}">
      <meta property="og:type" content="website">
      <meta property="og:url" content="/projects/{slug}/">
      <link rel="canonical" href="/projects/{slug}/">
    </head>
    <body>
      <div id="root"></div>
      <script src="/assets/index.js"></script>
    </body>
    </html>
    "#,
                    title = title,
                    slug = p.slug,
                    excerpt = excerpt
                ),
            });
        }

        Ok(pages)
    }

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let projects: Vec<model::Project> = rt.block_on(repo::list(db, None, 200))?;
        Ok(Box::new(projects))
    }

    fn build_search_docs(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let projects: Vec<model::Project> = rt.block_on(repo::list(db, None, 200))?;

        let docs: Vec<SearchDoc> = projects
            .into_iter()
            .map(|p| {
                let title = p.title_en.or(p.title_ko).unwrap_or_default();
                let desc = p.description_en.or(p.description_ko).unwrap_or_default();
                let excerpt: String = desc.chars().take(200).collect();
                SearchDoc {
                    id: format!("projects/{}", p.slug),
                    title,
                    body_preview: excerpt,
                    doc_type: "projects".to_string(),
                    url: format!("/projects/{}", p.slug),
                    published_at: p.published_at,
                }
            })
            .collect();

        Ok(docs)
    }
}

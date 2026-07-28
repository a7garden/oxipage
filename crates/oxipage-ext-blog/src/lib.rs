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

impl BuildExt for BlogExtension {
    fn ext_id(&self) -> &'static str {
        "blog"
    }

    fn build_pages(&self, db: &SqlitePool) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let posts: Vec<model::BlogPost> = handle.block_on(repo::list(db, false, None, i64::MAX))?;

        let mut pages = Vec::with_capacity(posts.len() * 3);

        for post in &posts {
            // HTML page with OG metas
            let excerpt = body_excerpt(&post.body, 160);

            let html = format!(
                r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <meta property="og:title" content="{title}">
  <meta property="og:description" content="{excerpt}">
  <meta property="og:type" content="article">
  <meta property="og:url" content="/blog/{slug}/">
  <meta name="twitter:card" content="summary">
  <link rel="canonical" href="/blog/{slug}/">
</head>
<body>
  <div id="root"></div>
  <script src="/assets/index.js"></script>
</body>
</html>
"#,
                lang = post.lang,
                title = post.title,
                slug = post.slug,
                excerpt = excerpt
            );

            pages.push(StaticPage {
                path: format!("blog/{}/index.html", post.slug),
                content: html,
            });

            // Markdown source file
            pages.push(StaticPage {
                path: format!("blog/{}/index.md", post.slug),
                content: post.body.clone(),
            });

            // JSON metadata
            let meta = serde_json::json!({
                "title": post.title,
                "slug": post.slug,
                "lang": post.lang,
                "tags": post.tags,
                "published_at": post.published_at,
            });
            pages.push(StaticPage {
                path: format!("blog/{}/index.json", post.slug),
                content: serde_json::to_string_pretty(&meta).unwrap_or_default(),
            });
        }

        Ok(pages)
    }

    fn build_data(&self, db: &SqlitePool) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let posts: Vec<model::BlogPost> = handle.block_on(repo::list(db, false, None, i64::MAX))?;
        Ok(Box::new(posts))
    }

    fn build_search_docs(&self, db: &SqlitePool) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let handle = tokio::runtime::Handle::current();
        let posts: Vec<model::BlogPost> = handle.block_on(repo::list(db, false, None, i64::MAX))?;

        let docs: Vec<SearchDoc> = posts
            .into_iter()
            .map(|p| {
                let excerpt = body_excerpt(&p.body, 200);
                SearchDoc {
                    id: format!("blog/{}", p.slug),
                    title: p.title,
                    body_preview: excerpt,
                    doc_type: "blog".to_string(),
                    url: format!("/blog/{}", p.slug),
                    published_at: p.published_at,
                }
            })
            .collect();

        Ok(docs)
    }
}

/// Body excerpt for search index / OG description.
fn body_excerpt(body: &str, max_chars: usize) -> String {
    let plain: String = body.chars().filter(|c| !c.is_control()).collect();
    let excerpt: String = plain.chars().take(max_chars).collect();
    if excerpt.len() < plain.len() {
        format!("{}…", excerpt)
    } else {
        excerpt
    }
}

pub mod integration;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::client::Client;

use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{
    CliArg, CliCommand, CliHandler, CliSubcommand, Extension, ExtensionWizard, Lang, LobbyCard,
    LobbyCardItem, Migration, SetupField, SetupFieldKind, SetupSaveHandler, SetupStep, StepOutcome,
    persist_extension_config,
};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct MoviesExtension;

// ── CLI handlers ──

struct MovieAddHandler;
impl CliHandler for MovieAddHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let title = args.get("title").cloned().unwrap_or_default();
        let mut body = serde_json::json!({ "title": title });
        if let Some(slug) = args.get("slug") {
            body["slug"] = serde_json::json!(slug);
        }
        if let Some(rating) = args.get("rating") {
            body["rating"] = serde_json::json!(rating);
        }
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post("/api/v1/movies/", &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct MovieSeriesCreateHandler;
impl CliHandler for MovieSeriesCreateHandler {
    fn run(
        &self,
        args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let name = args.get("name").cloned().unwrap_or_default();
        let slug = args.get("slug").cloned().unwrap_or_default();
        let body = serde_json::json!({ "name": name, "slug": slug });
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post("/api/v1/movies/series", &body)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

struct MoviesKeySave;
#[async_trait]
impl SetupSaveHandler for MoviesKeySave {
    async fn save(
        &self,
        ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        if let Some(v) = form.get("tmdb_key").and_then(|x| x.as_str())
            && !v.is_empty()
        {
            // SAFETY: setup wizard 는 단일 사용자 로컬 환경에서만 동작.
            unsafe {
                std::env::set_var("OXIPAGE_TMDB_KEY", v);
            }
            persist_extension_config(ctx, "movies", "OXIPAGE_TMDB_KEY", v).await?;
        }
        Ok(StepOutcome::from_form(form))
    }
}
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
            .route("/", get(routes::list).post(routes::create))
            .route("/search", get(routes::tmdb_search))
            .route(
                "/{slug}",
                get(routes::show)
                    .patch(routes::update)
                    .delete(routes::delete),
            )
            .route("/{slug}/publish", post(routes::publish))
            .route(
                "/series",
                get(routes::list_groups).post(routes::create_group),
            )
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

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "movies",
            about: "Manage movies and series",
            subcommands: vec![
                CliSubcommand {
                    name: "add",
                    about: "Add a movie review",
                    args: vec![
                        CliArg {
                            long: "title",
                            short: Some('t'),
                            help: "Movie title",
                            required: true,
                        },
                        CliArg {
                            long: "slug",
                            short: Some('s'),
                            help: "URL slug",
                            required: false,
                        },
                        CliArg {
                            long: "rating",
                            short: Some('r'),
                            help: "Rating (1-10)",
                            required: false,
                        },
                    ],
                    handler: Some(Arc::new(MovieAddHandler)),
                },
                CliSubcommand {
                    name: "series",
                    about: "Create a series group",
                    args: vec![
                        CliArg {
                            long: "name",
                            short: Some('n'),
                            help: "Series name",
                            required: true,
                        },
                        CliArg {
                            long: "slug",
                            short: Some('s'),
                            help: "URL slug",
                            required: true,
                        },
                    ],
                    handler: Some(Arc::new(MovieSeriesCreateHandler)),
                },
            ],
        }]
    }

    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![SetupStep {
                id: "movies_key",
                title_ko: "TMDB API 키",
                title_en: "TMDB API key",
                description_ko: "영화 정보 연동을 위한 TMDB 키 (선택)",
                description_en: "TMDB key for movie data (optional)",
                fields: vec![SetupField {
                    name: "tmdb_key",
                    label_ko: "TMDB API 키",
                    label_en: "TMDB API key",
                    kind: SetupFieldKind::Secret,
                    required: false,
                    placeholder_ko: None,
                    placeholder_en: None,
                }],
                save_handler: Arc::new(MoviesKeySave),
                prefill: BTreeMap::new(),
                visible_when: None,
            }],
        })
    }
}
impl BuildExt for MoviesExtension {
    fn ext_id(&self) -> &'static str {
        "movies"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let entries: Vec<model::MovieEntry> =
            rt.block_on(repo::list_entries_published(db, None, 200))?;
        let mut pages = Vec::with_capacity(entries.len());
        for e in &entries {
            let title = &e.title;
            let excerpt: String = e
                .review_ko
                .as_deref()
                .or(e.review_en.as_deref())
                .unwrap_or("")
                .chars()
                .take(160)
                .collect();
            pages.push(StaticPage {
                path: format!("movies/{}/index.html", e.slug),
                content: format!(
                    r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
    <title>{title}</title><meta property="og:title" content="{title}"><meta property="og:description" content="{excerpt}">
    <meta property="og:type" content="website"><meta property="og:url" content="/movies/{slug}/">
    <link rel="canonical" href="/movies/{slug}/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
                    title=title, excerpt=excerpt, slug=e.slug),
            });
        }
        Ok(pages)
    }

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let entries: Vec<model::MovieEntry> =
            rt.block_on(repo::list_entries_published(db, None, 200))?;
        Ok(Box::new(entries))
    }

    fn build_search_docs(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        let entries: Vec<model::MovieEntry> =
            rt.block_on(repo::list_entries_published(db, None, 200))?;
        Ok(entries
            .into_iter()
            .map(|e| {
                let title = e.title;
                let body = e.review_ko.or(e.review_en).unwrap_or_default();
                let excerpt: String = body.chars().take(200).collect();
                SearchDoc {
                    id: format!("movies/{}", e.slug),
                    title,
                    body_preview: excerpt,
                    doc_type: "movies".into(),
                    url: format!("/movies/{}", e.slug),
                    published_at: e.published_at,
                }
            })
            .collect())
    }
}

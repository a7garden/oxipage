pub mod client;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::extension::{Extension, Lang, LobbyCard, LobbyCardItem, Migration};
use oxipage_core::scheduler::ScheduledJob;
use oxipage_core::state::AppState;
use std::sync::Arc;

pub struct ActivityExtension;
pub struct ActivitySyncJob;

#[async_trait]
impl ScheduledJob for ActivitySyncJob {
    fn schedule(&self) -> &str {
        "0 */15 * * * *"
    }

    fn name(&self) -> &str {
        "activity_sync"
    }

    async fn run(&self) -> anyhow::Result<()> {
        match std::env::var("OXIPAGE_GITHUB_USERNAME") {
            Ok(username) if !username.trim().is_empty() => {
                tracing::info!(%username, "activity sync job is deferred to the authenticated sync endpoint in Phase 2");
            }
            _ => tracing::debug!("activity sync skipped: GitHub username is not configured"),
        }
        Ok(())
    }
}

#[async_trait]
impl Extension for ActivityExtension {
    fn id(&self) -> &'static str {
        "activity"
    }
    fn table_names(&self) -> Vec<&'static str> {
        vec!["activity_event"]
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "활동".to_string(),
            Lang::En => "Activity".to_string(),
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
            .route("/", get(routes::list))
            .route("/webhook", post(routes::webhook))
            .route("/sync", post(routes::sync))
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let events = repo::list(&ctx.db, None, 5).await.ok()?;
        if events.is_empty() {
            return None;
        }
        let items = events
            .into_iter()
            .map(|event| LobbyCardItem {
                title: event.summary,
                url: event.url,
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items,
        })
    }

    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        vec![Arc::new(ActivitySyncJob)]
    }
}

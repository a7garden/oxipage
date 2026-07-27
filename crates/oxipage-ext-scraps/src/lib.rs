pub mod integration;
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

pub struct ScrapsExtension;

#[async_trait]
impl Extension for ScrapsExtension {
    fn id(&self) -> &'static str {
        "scraps"
    }

    fn display_name(&self, lang: Lang) -> String {
        match lang {
            Lang::Ko => "스크랩".to_string(),
            Lang::En => "Scraps".to_string(),
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
                get(routes::list_published).post(routes::create_manual),
            )
            .route("/queue", get(routes::list_queue))
            .route("/{id}", get(routes::show).patch(routes::update).delete(routes::delete))
            .route("/{id}/publish", post(routes::publish))
    }

    fn background_jobs(&self) -> Vec<Arc<dyn ScheduledJob>> {
        vec![Arc::new(ScrapCollectJob)]
    }

    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard> {
        let items = repo::list(&ctx.db, true, None, 3).await.ok()?;
        let entries = items
            .into_iter()
            .map(|it| LobbyCardItem {
                title: it.title,
                url: format!("/scraps/{}", it.id),
            })
            .collect();
        Some(LobbyCard {
            id: self.id().to_string(),
            items: entries,
        })
    }
}

/// 30분 주기로 HackerNews/GeekNews 후보를 큐에 채우는 잡.
///
/// Phase 2 한계 (task spec): DB 풀에 직접 접근하려면 `AppState`가 필요한데
/// `ScheduledJob::run()`은 인자를 받지 않는다. 그래서 fetch/upsert 본체는 별도
/// 인프라(서버 부팅 시 잡에 풀 주입) 도입 후 구현하고, 현재는 텔레메트리만
/// 남기는 no-op으로 둔다. 실제 fetch 로직은 `integration.rs`에 두어 추후
/// 작업에서 깨끗하게 이식할 수 있도록 한다.
struct ScrapCollectJob;

#[async_trait]
impl ScheduledJob for ScrapCollectJob {
    fn schedule(&self) -> &str {
        "0 */30 * * * *"
    }

    fn name(&self) -> &str {
        "scraps_collect"
    }

    async fn run(&self) -> anyhow::Result<()> {
        tracing::info!("scraps collect tick (no-op until pool injection lands)");
        Ok(())
    }
}
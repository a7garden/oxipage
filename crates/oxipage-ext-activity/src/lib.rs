pub mod client;
pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use oxipage_core::client::Client;

use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{
    CliCommand, CliHandler, CliSubcommand, Extension, ExtensionWizard, Lang, LobbyCard,
    LobbyCardItem, Migration, SetupField, SetupFieldKind, SetupSaveHandler, SetupStep, StepOutcome,
    VisibilityRule,
};
use oxipage_core::scheduler::ScheduledJob;
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ── CLI handlers ──

struct ActivitySyncHandler;
impl CliHandler for ActivitySyncHandler {
    fn run(
        &self,
        _args: BTreeMap<String, String>,
        client: &Client,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let client = client.clone();
        Box::pin(async move {
            let resp = client
                .post_raw("/api/console/activity/sync", serde_json::json!({}))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            Ok(())
        })
    }
}

pub struct ActivityExtension;
pub struct ActivitySyncJob;

#[async_trait]
impl ScheduledJob for ActivitySyncJob {
    fn name(&self) -> &str {
        "activity_sync"
    }

    /// GitHub 공개 Events API 폴링 (doc/01 §1.9, doc/02 §2.8).
    /// webhook가 1순위, 이 폴링은 누락 보정용 보조.
    ///
    /// **구현 (doc/08 수정):** `ScheduledJob::run(&self, &AppState)` 시그니처로
    /// DB pool/config에 접근해 실제 fetch/upsert를 수행한다. 이전엔 시그니처가
    /// `run(&self)`라 "deferred to Phase 2" 로그만 찍는 no-op이었다.
    async fn run(&self, ctx: &AppState) -> anyhow::Result<()> {
        let client =
            client::GithubClient::with_username(ctx.config.integrations.github_username())?;
        if !client.enabled() {
            tracing::debug!("activity sync skipped: GitHub username not configured");
            return Ok(());
        }
        let events = client.fetch_public_events().await?;
        let total = events.len();
        let mut synced = 0usize;
        for event in events {
            // 빈 필드 이벤트는 스킵 (routes::validate_event와 동일한 안전망).
            if event.kind.trim().is_empty()
                || event.repo.name.trim().is_empty()
                || event.created_at.trim().is_empty()
            {
                continue;
            }
            match repo::upsert(&ctx.db, &event.into_input()).await {
                Ok(_) => synced += 1,
                Err(e) => tracing::warn!(error = ?e, "activity upsert failed"),
            }
        }
        tracing::info!(total, synced, "activity sync tick completed");
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

    fn cli_commands(&self) -> Vec<CliCommand> {
        vec![CliCommand {
            name: "activity",
            about: "Manage activity feed",
            subcommands: vec![CliSubcommand {
                name: "sync",
                about: "Trigger activity sync from GitHub",
                args: vec![],
                handler: Some(Arc::new(ActivitySyncHandler)),
            }],
        }]
    }

    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![
                SetupStep {
                    id: "activity_github",
                    title_ko: "GitHub 사용자명",
                    title_en: "GitHub username",
                    description_ko: "활동 동기화에 사용할 GitHub 사용자명 (선택)",
                    description_en: "GitHub username for activity sync (optional)",
                    fields: vec![SetupField {
                        name: "github_username",
                        label_ko: "GitHub 사용자명",
                        label_en: "GitHub username",
                        kind: SetupFieldKind::Text,
                        required: false,
                        placeholder_ko: None,
                        placeholder_en: None,
                    }],
                    save_handler: Arc::new(ActivityGithubSave),
                    prefill: BTreeMap::new(),
                    visible_when: None,
                },
                SetupStep {
                    id: "activity_sync",
                    title_ko: "활동 동기화",
                    title_en: "Sync activity",
                    description_ko: "GitHub 공개 활동을 지금 가져옵니다",
                    description_en: "Fetch public GitHub activity now",
                    fields: vec![],
                    save_handler: Arc::new(ActivitySyncSave),
                    prefill: BTreeMap::new(),
                    visible_when: Some(VisibilityRule::FieldNotEmpty {
                        step_id: "activity_github",
                        field: "github_username",
                    }),
                },
            ],
        })
    }
}

struct ActivityGithubSave;
#[async_trait]
impl SetupSaveHandler for ActivityGithubSave {
    async fn save(
        &self,
        _ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        if let Some(v) = form.get("github_username").and_then(|x| x.as_str())
            && !v.is_empty()
        {
            // SAFETY: setup wizard 는 단일 사용자 로컬 환경에서만 동작.
            // activity 는 config.integrations 가 env 를 직접 읽으므로 env 만 set.
            unsafe {
                std::env::set_var("OXIPAGE_GITHUB_USERNAME", v);
            }
        }
        Ok(StepOutcome::from_form(form))
    }
}

struct ActivitySyncSave;
#[async_trait]
impl SetupSaveHandler for ActivitySyncSave {
    async fn save(
        &self,
        ctx: &AppState,
        _form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        let client = client::GithubClient::from_env()?;
        let mut synced = 0u32;
        if client.enabled()
            && let Ok(events) = client.fetch_public_events().await {
                for event in events {
                    if event.kind.trim().is_empty()
                        || event.repo.name.trim().is_empty()
                        || event.created_at.trim().is_empty()
                    {
                        continue;
                    }
                    if repo::upsert(&ctx.db, &event.into_input()).await.is_ok() {
                        synced += 1;
                    }
                }
            }
        let mut m = serde_json::Map::new();
        m.insert("synced".into(), synced.to_string().into());
        Ok(StepOutcome { values: m })
    }
}
impl BuildExt for ActivityExtension {
    fn ext_id(&self) -> &'static str {
        "activity"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let events: Vec<model::ActivityEvent> = rt.block_on(repo::list(db, None, 200))?;
        let items = events
            .iter()
            .map(|e| format!("<li>{summary} — {repo}</li>", summary = e.summary, repo = e.repo_full_name))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(vec![StaticPage {
            path: "activity/index.html".into(),
            content: format!(
                r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0">
    <title>Activity</title><link rel="canonical" href="/activity/"></head><body><div id="root"></div><noscript><ul>
    {items}
    </ul></noscript><script src="/assets/index.js"></script></body></html>"#
            ),
        }])
    }

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let events: Vec<model::ActivityEvent> = rt.block_on(repo::list(db, None, 200))?;
        Ok(Box::new(events))
    }

    fn build_search_docs(
        &self,
        _db: &SqlitePool,
        _rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        Ok(vec![])
    }
}

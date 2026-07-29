pub mod model;
pub mod repo;
pub mod routes;

use async_trait::async_trait;
use axum::Router;
use axum::routing::get;
use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::extension::{
    Extension, ExtensionWizard, Lang, LobbyCard, Migration, PrefillSource, SetupField,
    SetupFieldKind, SetupSaveHandler, SetupStep, StepOutcome,
};
use oxipage_core::state::AppState;
use sqlx::SqlitePool;
use std::error::Error;
use std::sync::Arc;

pub struct ProfileExtension;

/// profile 확장의 setup wizard 저장 핸들러 — 자기 repo로 위임.
struct ProfileSetupSaveHandler;
#[async_trait]
impl SetupSaveHandler for ProfileSetupSaveHandler {
    async fn save(
        &self,
        ctx: &AppState,
        form: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<StepOutcome> {
        repo::update_from_setup_form(&ctx.db, form)
            .await
            .map(|_| StepOutcome::from_form(form))
    }
}

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

    fn setup_wizard(&self) -> Option<ExtensionWizard> {
        Some(ExtensionWizard {
            steps: vec![SetupStep {
                id: "profile",
                title_ko: "프로필",
                title_en: "Profile",
                description_ko: "사이트에 표시할 신상 정보",
                description_en: "Profile info displayed on your site",
                fields: vec![
                    SetupField {
                        name: "display_name",
                        label_ko: "표시 이름",
                        label_en: "Display name",
                        kind: SetupFieldKind::Text,
                        required: true,
                        placeholder_ko: None,
                        placeholder_en: None,
                    },
                    SetupField {
                        name: "tagline_ko",
                        label_ko: "한 줄 소개 (한국어)",
                        label_en: "Tagline (Korean)",
                        kind: SetupFieldKind::Text,
                        required: false,
                        placeholder_ko: Some("개발자 & 작가"),
                        placeholder_en: None,
                    },
                    SetupField {
                        name: "tagline_en",
                        label_ko: "한 줄 소개 (English)",
                        label_en: "Tagline (English)",
                        kind: SetupFieldKind::Text,
                        required: false,
                        placeholder_ko: None,
                        placeholder_en: Some("Developer & Writer"),
                    },
                    SetupField {
                        name: "github_username",
                        label_ko: "GitHub 사용자명",
                        label_en: "GitHub username",
                        kind: SetupFieldKind::Text,
                        required: false,
                        placeholder_ko: None,
                        placeholder_en: None,
                    },
                    SetupField {
                        name: "bio_ko",
                        label_ko: "자기소개 (한국어, markdown)",
                        label_en: "Bio (Korean, markdown)",
                        kind: SetupFieldKind::Textarea,
                        required: false,
                        placeholder_ko: None,
                        placeholder_en: None,
                    },
                    SetupField {
                        name: "bio_en",
                        label_ko: "자기소개 (English, markdown)",
                        label_en: "Bio (English, markdown)",
                        kind: SetupFieldKind::Textarea,
                        required: false,
                        placeholder_ko: None,
                        placeholder_en: None,
                    },
                ],
                save_handler: Arc::new(ProfileSetupSaveHandler),
                prefill: {
                    let mut m = std::collections::BTreeMap::new();
                    // site_name으로 display_name을 pre-fill (UX: 사이트 이름을 본인 이름으로 시작).
                    m.insert("display_name", PrefillSource::SiteName);
                    m
                },
                visible_when: None,
            }],
        })
    }

    async fn on_startup(&self, ctx: &AppState) -> anyhow::Result<()> {
        repo::ensure_singleton(&ctx.db, &ctx.config.site.name).await
    }
}

impl BuildExt for ProfileExtension {
    fn ext_id(&self) -> &'static str {
        "profile"
    }

    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>> {
        let profile: Option<model::Profile> = rt.block_on(repo::get(db))?;

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

    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn Error + Send + Sync>> {
        let profile: Option<model::Profile> = rt.block_on(repo::get(db))?;
        Ok(Box::new(profile))
    }

    fn build_search_docs(
        &self,
        _db: &SqlitePool,
        _rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>> {
        Ok(vec![])
    }
}

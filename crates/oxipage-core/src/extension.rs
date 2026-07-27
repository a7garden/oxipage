use crate::state::AppState;
use async_trait::async_trait;
use axum::Router;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCardItem {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LobbyCard {
    pub id: String,
    pub items: Vec<LobbyCardItem>,
}

#[async_trait]
pub trait Extension: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self, lang: Lang) -> String;
    fn migrations(&self) -> Vec<Migration>;
    fn routes(&self) -> Router<AppState>;
    async fn lobby_summary(&self, ctx: &AppState) -> Option<LobbyCard>;
    async fn on_startup(&self, _ctx: &AppState) -> anyhow::Result<()> {
        Ok(())
    }
}

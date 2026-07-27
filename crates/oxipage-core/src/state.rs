use crate::config::Config;
use crate::registry::ExtensionRegistry;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
    pub admin_token: Option<Arc<str>>,
    pub registry: Arc<ExtensionRegistry>,
}

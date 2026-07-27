use crate::extension::Extension;
use crate::migrate;
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct ExtensionRegistry {
    extensions: Vec<Arc<dyn Extension>>,
}

impl ExtensionRegistry {
    pub fn new(extensions: Vec<Arc<dyn Extension>>) -> Self {
        ExtensionRegistry { extensions }
    }

    pub fn find(&self, id: &str) -> Option<&Arc<dyn Extension>> {
        self.extensions.iter().find(|e| e.id() == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Extension>> {
        self.extensions.iter()
    }

    pub async fn run_migrations(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        migrate::run_migrations(pool, "_core", migrate::CORE_MIGRATIONS).await?;
        for ext in &self.extensions {
            migrate::run_migrations(pool, ext.id(), &ext.migrations()).await?;
        }
        Ok(())
    }
}

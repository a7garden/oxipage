//! Console-level state database (`~/.config/oxipage/console.db`).
//!
//! Stores setup wizard state and console-wide metadata, separate from
//! per-site databases. The `setup_state` table tracks whether the first-run
//! wizard has completed (spec §6.0).

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;

/// Console-level state: a lightweight SQLite DB with setup tracking.
pub struct ConsoleState {
    pool: SqlitePool,
}

impl ConsoleState {
    /// Open (or create) `console.db` in the given data directory and run
    /// migrations. Creates the directory if it doesn't exist.
    pub async fn open(data_dir: &Path) -> anyhow::Result<Self> {
        tokio::fs::create_dir_all(data_dir).await?;
        let db_path = data_dir.join("console.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await?;

        // Run migrations
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS setup_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                setup_completed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );",
        )
        .execute(&pool)
        .await?;

        // Seed the single-row state
        sqlx::query("INSERT OR IGNORE INTO setup_state (id) VALUES (1);")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    /// Reference to the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Returns `true` if the setup wizard has never completed.
    pub async fn is_setup_needed(&self) -> bool {
        let row: Result<(Option<String>,), _> =
            sqlx::query_as("SELECT setup_completed_at FROM setup_state WHERE id = 1")
                .fetch_one(&self.pool)
                .await;
        row.map(|r| r.0.is_none()).unwrap_or(true)
    }

    /// Mark the setup wizard as complete (set `setup_completed_at` to now).
    pub async fn mark_setup_complete(&self) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE setup_state SET setup_completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn console_state_migrates_setup_state_table() {
        let tmp = TempDir::with_prefix("oxipage-console-state-").unwrap();
        let state = ConsoleState::open(tmp.path()).await.unwrap();
        let row: (i64, Option<String>) =
            sqlx::query_as("SELECT id, setup_completed_at FROM setup_state WHERE id = 1")
                .fetch_one(state.pool())
                .await
                .unwrap();
        assert_eq!(row.0, 1);
        assert!(row.1.is_none(), "setup should not be completed yet");
    }

    #[tokio::test]
    async fn is_setup_needed_returns_true_before_completion() {
        let tmp = TempDir::with_prefix("oxipage-console-state-").unwrap();
        let state = ConsoleState::open(tmp.path()).await.unwrap();
        assert!(state.is_setup_needed().await);
    }

    #[tokio::test]
    async fn mark_setup_complete_makes_is_setup_needed_false() {
        let tmp = TempDir::with_prefix("oxipage-console-state-").unwrap();
        let state = ConsoleState::open(tmp.path()).await.unwrap();
        state.mark_setup_complete().await.unwrap();
        assert!(!state.is_setup_needed().await);
    }

    #[tokio::test]
    async fn console_db_created_in_data_dir() {
        let tmp = TempDir::with_prefix("oxipage-console-state-").unwrap();
        ConsoleState::open(tmp.path()).await.unwrap();
        assert!(tmp.path().join("console.db").exists());
    }
}

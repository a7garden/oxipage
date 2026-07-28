use crate::extension::Migration;
use sqlx::SqlitePool;

pub const CORE_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "lobby_config",
        sql: include_str!("../migrations/core/0001_lobby_config.sql"),
    },
    Migration {
        version: 2,
        name: "search_documents",
        sql: include_str!("../migrations/core/0002_search_documents.sql"),
    },
    Migration {
        version: 3,
        name: "auth_token",
        sql: include_str!("../migrations/core/0003_auth_token.sql"),
    },
    Migration {
        version: 4,
        name: "extension_state",
        sql: include_str!("../migrations/core/0004_extension_state.sql"),
    },
    Migration {
        version: 5,
        name: "theme_config",
        sql: include_str!("../migrations/core/0005_theme_config.sql"),
    },
    Migration {
        version: 6,
        name: "setup_state",
        sql: include_str!("../migrations/core/0006_setup_state.sql"),
    },
];

pub async fn run_migrations(
    pool: &SqlitePool,
    extension: &str,
    migrations: &[Migration],
) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            extension TEXT NOT NULL,
            version INTEGER NOT NULL,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            PRIMARY KEY (extension, version)
        )",
    )
    .execute(pool)
    .await?;

    for m in migrations {
        let applied: Option<(i64,)> = sqlx::query_as(
            "SELECT version FROM schema_migrations WHERE extension = ? AND version = ?",
        )
        .bind(extension)
        .bind(m.version)
        .fetch_optional(pool)
        .await?;
        if applied.is_some() {
            continue;
        }
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(m.sql).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO schema_migrations (extension, version, name) VALUES (?, ?, ?)")
            .bind(extension)
            .bind(m.version)
            .bind(m.name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(
            extension,
            version = m.version,
            name = m.name,
            "migration applied"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::Migration;

    const M1: Migration = Migration {
        version: 1,
        name: "one",
        sql: "CREATE TABLE t1 (id INTEGER PRIMARY KEY);",
    };
    const M2: Migration = Migration {
        version: 2,
        name: "two",
        sql: "CREATE TABLE t2 (id INTEGER PRIMARY KEY);",
    };
    const M1_B: Migration = Migration {
        version: 1,
        name: "one",
        sql: "CREATE TABLE t1_b (id INTEGER PRIMARY KEY);",
    };

    #[tokio::test]
    async fn applies_migrations_once_and_records_them() {
        let pool = crate::db::connect_memory().await.unwrap();
        run_migrations(&pool, "test_ext", &[M1, M2]).await.unwrap();
        run_migrations(&pool, "test_ext", &[M1, M2]).await.unwrap(); // idempotent

        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM schema_migrations WHERE extension = 'test_ext'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 2);

        // tables actually exist
        sqlx::query("INSERT INTO t1 DEFAULT VALUES")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t2 DEFAULT VALUES")
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn namespaces_migrations_per_extension() {
        let pool = crate::db::connect_memory().await.unwrap();
        run_migrations(&pool, "ext_a", &[M1]).await.unwrap();
        run_migrations(&pool, "ext_b", &[M1_B]).await.unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}

use crate::model::{ActivityEvent, ActivityEventInput};
use sqlx::SqlitePool;

const COLUMNS: &str =
    "id, repo_full_name, event_type, summary, url, occurred_at, synced_at, created_at";

pub async fn upsert(
    pool: &SqlitePool,
    input: &ActivityEventInput,
) -> anyhow::Result<ActivityEvent> {
    let event = sqlx::query_as::<_, ActivityEvent>(&format!(
        "INSERT INTO activity_event (repo_full_name, event_type, summary, url, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(repo_full_name, event_type, url, occurred_at) DO UPDATE SET
            summary = excluded.summary,
            synced_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {COLUMNS}"
    ))
    .bind(&input.repo_full_name)
    .bind(&input.event_type)
    .bind(&input.summary)
    .bind(&input.url)
    .bind(&input.occurred_at)
    .fetch_one(pool)
    .await?;
    Ok(event)
}

pub async fn list(
    pool: &SqlitePool,
    repo: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<ActivityEvent>> {
    let limit = limit.clamp(1, 500);
    let events = if let Some(repo) = repo {
        sqlx::query_as::<_, ActivityEvent>(&format!(
            "SELECT {COLUMNS} FROM activity_event
             WHERE repo_full_name = ?1
             ORDER BY occurred_at DESC, id DESC LIMIT ?2"
        ))
        .bind(repo)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ActivityEvent>(&format!(
            "SELECT {COLUMNS} FROM activity_event
             ORDER BY occurred_at DESC, id DESC LIMIT ?1"
        ))
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(events)
}

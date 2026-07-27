use crate::model::{Book, BookInput, BookPatch};
use sqlx::SqlitePool;

const COLUMNS: &str = "id, source, external_id, isbn13, title, author, cover_image_url,
                       rating, review_ko, review_en, status, started_at, finished_at,
                       published_at, created_at, updated_at";

pub async fn create(pool: &SqlitePool, input: &BookInput) -> anyhow::Result<Book> {
    let row = sqlx::query_as::<_, Book>(&format!(
        "INSERT INTO book_entry
            (source, external_id, isbn13, title, author, cover_image_url,
             rating, review_ko, review_en, status, started_at, finished_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         RETURNING {COLUMNS}"
    ))
    .bind(&input.source)
    .bind(&input.external_id)
    .bind(&input.isbn13)
    .bind(&input.title)
    .bind(&input.author)
    .bind(&input.cover_image_url)
    .bind(input.rating)
    .bind(&input.review_ko)
    .bind(&input.review_en)
    .bind(&input.status)
    .bind(&input.started_at)
    .bind(&input.finished_at)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Book>> {
    let row = sqlx::query_as::<_, Book>(&format!(
        "SELECT {COLUMNS} FROM book_entry WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `status=None` → 전체. 초안(`published_at IS NULL`)은 제외.
pub async fn list(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<Book>> {
    let limit = limit.clamp(1, 200);
    let sql = if status.is_some() {
        format!(
            "SELECT {COLUMNS} FROM book_entry
             WHERE published_at IS NOT NULL AND status = ?
             ORDER BY published_at DESC LIMIT ?"
        )
    } else {
        format!(
            "SELECT {COLUMNS} FROM book_entry
             WHERE published_at IS NOT NULL
             ORDER BY published_at DESC LIMIT ?"
        )
    };
    let mut q = sqlx::query_as::<_, Book>(&sql);
    if let Some(s) = status {
        q = q.bind(s);
    }
    let rows = q.bind(limit).fetch_all(pool).await?;
    Ok(rows)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    patch: &BookPatch,
) -> anyhow::Result<Option<Book>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.source.is_some() { sets.push("source = ?"); }
    if patch.external_id.is_some() { sets.push("external_id = ?"); }
    if patch.isbn13.is_some() { sets.push("isbn13 = ?"); }
    if patch.title.is_some() { sets.push("title = ?"); }
    if patch.author.is_some() { sets.push("author = ?"); }
    if patch.cover_image_url.is_some() { sets.push("cover_image_url = ?"); }
    if patch.rating.is_some() { sets.push("rating = ?"); }
    if patch.review_ko.is_some() { sets.push("review_ko = ?"); }
    if patch.review_en.is_some() { sets.push("review_en = ?"); }
    if patch.status.is_some() { sets.push("status = ?"); }
    if patch.started_at.is_some() { sets.push("started_at = ?"); }
    if patch.finished_at.is_some() { sets.push("finished_at = ?"); }
    if sets.is_empty() {
        return find_by_id(pool, id).await;
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')");
    let set_clause = sets.join(", ");
    let sql = format!("UPDATE book_entry SET {set_clause} WHERE id = ? RETURNING {COLUMNS}");
    let mut q = sqlx::query_as::<_, Book>(&sql);
    if let Some(v) = &patch.source { q = q.bind(v); }
    if let Some(v) = &patch.external_id { q = q.bind(v); }
    if let Some(v) = &patch.isbn13 { q = q.bind(v); }
    if let Some(v) = &patch.title { q = q.bind(v); }
    if let Some(v) = &patch.author { q = q.bind(v); }
    if let Some(v) = &patch.cover_image_url { q = q.bind(v); }
    if let Some(v) = patch.rating { q = q.bind(v); }
    if let Some(v) = &patch.review_ko { q = q.bind(v); }
    if let Some(v) = &patch.review_en { q = q.bind(v); }
    if let Some(v) = &patch.status { q = q.bind(v); }
    if let Some(v) = &patch.started_at { q = q.bind(v); }
    if let Some(v) = &patch.finished_at { q = q.bind(v); }
    let row = q.bind(id).fetch_optional(pool).await?;
    Ok(row)
}

pub async fn publish(pool: &SqlitePool, id: i64) -> anyhow::Result<Book> {
    let row = sqlx::query_as::<_, Book>(&format!(
        "UPDATE book_entry
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE id = ?
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM book_entry WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

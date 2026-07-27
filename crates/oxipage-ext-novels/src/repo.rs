use crate::model::{ChapterInput, ChapterPatch, Novel, NovelChapter, NovelInput, char_count};
use sqlx::SqlitePool;

const NOVEL_COLUMNS: &str = "id, slug, title, synopsis, cover_image, status, tags,
                             published_at, created_at, updated_at";
const CHAPTER_COLUMNS: &str = "id, novel_id, chapter_order, title, body, char_count,
                               published_at, created_at, updated_at";

pub fn slugify(title: &str) -> String {
    let base: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = base.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("novel-{}", unix_ts())
    } else {
        trimmed
    }
}

fn unix_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn slug_exists(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM novel WHERE slug = ?")
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

pub async fn ensure_unique_slug(pool: &SqlitePool, base: &str) -> anyhow::Result<String> {
    if !slug_exists(pool, base).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !slug_exists(pool, &candidate).await? {
            return Ok(candidate);
        }
    }
    anyhow::bail!("could not allocate unique slug for {base}")
}

// ─── Novel ───

pub async fn create_novel(pool: &SqlitePool, input: &NovelInput, resolved_slug: &str) -> anyhow::Result<Novel> {
    let tags = serde_json::to_string(&input.tags)?;
    let novel = sqlx::query_as::<_, Novel>(&format!(
        "INSERT INTO novel (slug, title, synopsis, cover_image, status, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         RETURNING {NOVEL_COLUMNS}"
    ))
    .bind(resolved_slug)
    .bind(&input.title)
    .bind(&input.synopsis)
    .bind(&input.cover_image)
    .bind(&input.status)
    .bind(tags)
    .fetch_one(pool)
    .await?;
    Ok(novel)
}

pub async fn find_novel_by_slug(pool: &SqlitePool, slug: &str) -> anyhow::Result<Option<Novel>> {
    let n = sqlx::query_as::<_, Novel>(&format!("SELECT {NOVEL_COLUMNS} FROM novel WHERE slug = ?"))
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(n)
}

pub async fn list_novels(pool: &SqlitePool, draft: bool, limit: i64) -> anyhow::Result<Vec<Novel>> {
    let limit = limit.clamp(1, 200);
    let sql = if draft {
        format!("SELECT {NOVEL_COLUMNS} FROM novel WHERE published_at IS NULL ORDER BY created_at DESC LIMIT ?")
    } else {
        format!("SELECT {NOVEL_COLUMNS} FROM novel WHERE published_at IS NOT NULL ORDER BY published_at DESC LIMIT ?")
    };
    let novels = sqlx::query_as::<_, Novel>(&sql)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(novels)
}

pub async fn publish_novel(pool: &SqlitePool, slug: &str) -> anyhow::Result<Novel> {
    let n = sqlx::query_as::<_, Novel>(&format!(
        "UPDATE novel
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE slug = ?
         RETURNING {NOVEL_COLUMNS}"
    ))
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

pub async fn delete_novel(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM novel WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ─── Chapter ───

pub async fn create_chapter(
    pool: &SqlitePool,
    novel_slug: &str,
    input: &ChapterInput,
) -> anyhow::Result<NovelChapter> {
    let novel_id = novel_id(pool, novel_slug).await?;
    let cc = char_count(&input.body);
    let ch = sqlx::query_as::<_, NovelChapter>(&format!(
        "INSERT INTO novel_chapter (novel_id, chapter_order, title, body, char_count)
         VALUES (?1, ?2, ?3, ?4, ?5)
         RETURNING {CHAPTER_COLUMNS}"
    ))
    .bind(novel_id)
    .bind(input.chapter_order)
    .bind(&input.title)
    .bind(&input.body)
    .bind(cc)
    .fetch_one(pool)
    .await?;
    Ok(ch)
}

async fn novel_id(pool: &SqlitePool, novel_slug: &str) -> anyhow::Result<i64> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM novel WHERE slug = ?")
        .bind(novel_slug)
        .fetch_optional(pool)
        .await?;
    row.map(|r| r.0)
        .ok_or_else(|| anyhow::anyhow!("novel not found"))
}

pub async fn list_chapters(pool: &SqlitePool, novel_slug: &str, draft: bool) -> anyhow::Result<Vec<NovelChapter>> {
    let sql = if draft {
        format!(
            "SELECT {CHAPTER_COLUMNS} FROM novel_chapter
             WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
             ORDER BY chapter_order ASC"
        )
    } else {
        format!(
            "SELECT {CHAPTER_COLUMNS} FROM novel_chapter
             WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
               AND published_at IS NOT NULL
             ORDER BY chapter_order ASC"
        )
    };
    let chapters = sqlx::query_as::<_, NovelChapter>(&sql)
        .bind(novel_slug)
        .fetch_all(pool)
        .await?;
    Ok(chapters)
}

pub async fn find_chapter(pool: &SqlitePool, novel_slug: &str, order: i32) -> anyhow::Result<Option<NovelChapter>> {
    let ch = sqlx::query_as::<_, NovelChapter>(&format!(
        "SELECT {CHAPTER_COLUMNS} FROM novel_chapter
         WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
           AND chapter_order = ?"
    ))
    .bind(novel_slug)
    .bind(order)
    .fetch_optional(pool)
    .await?;
    Ok(ch)
}

pub async fn update_chapter(
    pool: &SqlitePool,
    novel_slug: &str,
    order: i32,
    patch: &ChapterPatch,
) -> anyhow::Result<Option<NovelChapter>> {
    let mut sets: Vec<String> = Vec::new();
    if patch.title.is_some() { sets.push("title = ?".into()); }
    if patch.body.is_some() { sets.push("body = ?".into()); sets.push("char_count = ?".into()); }
    if patch.chapter_order.is_some() { sets.push("chapter_order = ?".into()); }
    if sets.is_empty() {
        return find_chapter(pool, novel_slug, order).await;
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')".into());
    let set_clause = sets.join(", ");
    let cc = patch.body.as_ref().map(|b| char_count(b));
    let sql = format!(
        "UPDATE novel_chapter SET {set_clause}
         WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
           AND chapter_order = ?
         RETURNING {CHAPTER_COLUMNS}"
    );
    let mut q = sqlx::query_as::<_, NovelChapter>(&sql)
        .bind(novel_slug)
        .bind(order);
    if let Some(v) = &patch.title { q = q.bind(v); }
    if let Some(v) = &patch.body { q = q.bind(v); q = q.bind(cc.unwrap_or(0)); }
    if let Some(v) = patch.chapter_order { q = q.bind(v); }
    let ch = q.fetch_optional(pool).await?;
    Ok(ch)
}

pub async fn publish_chapter(pool: &SqlitePool, novel_slug: &str, order: i32) -> anyhow::Result<NovelChapter> {
    let ch = sqlx::query_as::<_, NovelChapter>(&format!(
        "UPDATE novel_chapter
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
            AND chapter_order = ?
         RETURNING {CHAPTER_COLUMNS}"
    ))
    .bind(novel_slug)
    .bind(order)
    .fetch_one(pool)
    .await?;
    Ok(ch)
}

pub async fn delete_chapter(pool: &SqlitePool, novel_slug: &str, order: i32) -> anyhow::Result<bool> {
    let res = sqlx::query(
        "DELETE FROM novel_chapter
         WHERE novel_id = (SELECT id FROM novel WHERE slug = ?)
           AND chapter_order = ?",
    )
    .bind(novel_slug)
    .bind(order)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

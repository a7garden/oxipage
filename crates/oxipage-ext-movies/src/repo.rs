use crate::model::{MovieEntry, MovieEntryInput, MovieEntryPatch, SeriesGroup, SeriesGroupInput};
use sqlx::SqlitePool;

const ENTRY_COLUMNS: &str = "id, slug, tmdb_id, media_type, title, poster_path, release_year,
                              watched_at, rating, review_ko, review_en, rewatch,
                              series_group_id, series_order, published_at,
                              created_at, updated_at";

const GROUP_COLUMNS: &str = "id, slug, title_ko, title_en, cover_image,
                              group_rating, group_review_ko, group_review_en,
                              created_at, updated_at";

// ─── Slug helpers ───

/// 제목 → slug. 영문/숫자는 그대로, 그 외는 '-'. 양끝 '-' 제거.
/// 한글이면 전부 '-' 가 되니 폴백(`movie-<ts>`)이 발동된다.
pub fn slugify(title: &str) -> String {
    let base: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = base.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("movie-{}", unix_ts())
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

async fn entry_slug_exists(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM movie_entry WHERE slug = ?")
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

async fn group_slug_exists(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM series_group WHERE slug = ?")
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

pub async fn ensure_unique_entry_slug(pool: &SqlitePool, base: &str) -> anyhow::Result<String> {
    if !entry_slug_exists(pool, base).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !entry_slug_exists(pool, &candidate).await? {
            return Ok(candidate);
        }
    }
    anyhow::bail!("could not allocate unique entry slug for {base}")
}

pub async fn ensure_unique_group_slug(pool: &SqlitePool, base: &str) -> anyhow::Result<String> {
    if !group_slug_exists(pool, base).await? {
        return Ok(base.to_string());
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !group_slug_exists(pool, &candidate).await? {
            return Ok(candidate);
        }
    }
    anyhow::bail!("could not allocate unique group slug for {base}")
}

// ─── MovieEntry ───

#[allow(clippy::too_many_arguments)]
pub async fn create_entry(
    pool: &SqlitePool,
    input: &MovieEntryInput,
    resolved_slug: &str,
    tmdb_id: Option<i64>,
    title: String,
    poster_path: Option<String>,
    release_year: Option<i32>,
) -> anyhow::Result<MovieEntry> {
    let rewatch: i8 = if input.rewatch { 1 } else { 0 };
    let entry = sqlx::query_as::<_, MovieEntry>(&format!(
        "INSERT INTO movie_entry
            (slug, tmdb_id, media_type, title, poster_path, release_year,
             watched_at, rating, review_ko, review_en, rewatch,
             series_group_id, series_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         RETURNING {ENTRY_COLUMNS}"
    ))
    .bind(resolved_slug)
    .bind(tmdb_id)
    .bind(&input.media_type)
    .bind(&title)
    .bind(poster_path.as_deref())
    .bind(release_year)
    .bind(input.watched_at.as_deref())
    .bind(input.rating)
    .bind(input.review_ko.as_deref())
    .bind(input.review_en.as_deref())
    .bind(rewatch)
    .bind(input.series_group_id)
    .bind(input.series_order)
    .fetch_one(pool)
    .await?;
    Ok(entry)
}

pub async fn find_entry_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> anyhow::Result<Option<MovieEntry>> {
    let entry = sqlx::query_as::<_, MovieEntry>(&format!(
        "SELECT {ENTRY_COLUMNS} FROM movie_entry WHERE slug = ?"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(entry)
}

/// 발행본만. series_group_slug로 필터 가능.
/// 정렬: 최신 watched_at 우선, 없으면 최신 created_at. NULL은 뒤로.
pub async fn list_entries_published(
    pool: &SqlitePool,
    series_group_slug: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<MovieEntry>> {
    let limit = limit.clamp(1, 200);
    let entries = if let Some(slug) = series_group_slug {
        sqlx::query_as::<_, MovieEntry>(&format!(
            "SELECT {ENTRY_COLUMNS} FROM movie_entry
             WHERE published_at IS NOT NULL
               AND series_group_id = (SELECT id FROM series_group WHERE slug = ?)
             ORDER BY COALESCE(watched_at, created_at) DESC, id DESC
             LIMIT ?"
        ))
        .bind(slug)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, MovieEntry>(&format!(
            "SELECT {ENTRY_COLUMNS} FROM movie_entry
             WHERE published_at IS NOT NULL
             ORDER BY COALESCE(watched_at, created_at) DESC, id DESC
             LIMIT ?"
        ))
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(entries)
}

/// 시리즈에 속한 entry (group_id 기반).
/// 정렬은 series_order ASC, NULLs last, id ASC.
/// 공개 API에서는 published_only=true로 호출 (초안 숨김).
pub async fn list_entries_by_group_id(
    pool: &SqlitePool,
    group_id: i64,
    published_only: bool,
) -> anyhow::Result<Vec<MovieEntry>> {
    let entries = if published_only {
        sqlx::query_as::<_, MovieEntry>(&format!(
            "SELECT {ENTRY_COLUMNS} FROM movie_entry
             WHERE series_group_id = ?
               AND published_at IS NOT NULL
             ORDER BY CASE WHEN series_order IS NULL THEN 1 ELSE 0 END,
                      series_order ASC, id ASC"
        ))
        .bind(group_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, MovieEntry>(&format!(
            "SELECT {ENTRY_COLUMNS} FROM movie_entry
             WHERE series_group_id = ?
             ORDER BY CASE WHEN series_order IS NULL THEN 1 ELSE 0 END,
                      series_order ASC, id ASC"
        ))
        .bind(group_id)
        .fetch_all(pool)
        .await?
    };
    Ok(entries)
}

pub async fn publish_entry(pool: &SqlitePool, slug: &str) -> anyhow::Result<MovieEntry> {
    let entry = sqlx::query_as::<_, MovieEntry>(&format!(
        "UPDATE movie_entry
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE slug = ?
         RETURNING {ENTRY_COLUMNS}"
    ))
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(entry)
}

/// 부분 갱신. None은 미변경.
pub async fn update_entry(
    pool: &SqlitePool,
    slug: &str,
    patch: &MovieEntryPatch,
) -> anyhow::Result<Option<MovieEntry>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.tmdb_id.is_some() {
        sets.push("tmdb_id = ?");
    }
    if patch.media_type.is_some() {
        sets.push("media_type = ?");
    }
    if patch.title.is_some() {
        sets.push("title = ?");
    }
    if patch.poster_path.is_some() {
        sets.push("poster_path = ?");
    }
    if patch.release_year.is_some() {
        sets.push("release_year = ?");
    }
    if patch.watched_at.is_some() {
        sets.push("watched_at = ?");
    }
    if patch.rating.is_some() {
        sets.push("rating = ?");
    }
    if patch.review_ko.is_some() {
        sets.push("review_ko = ?");
    }
    if patch.review_en.is_some() {
        sets.push("review_en = ?");
    }
    if patch.rewatch.is_some() {
        sets.push("rewatch = ?");
    }
    if patch.series_group_id.is_some() {
        sets.push("series_group_id = ?");
    }
    if patch.series_order.is_some() {
        sets.push("series_order = ?");
    }
    if sets.is_empty() {
        return find_entry_by_slug(pool, slug).await;
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')");
    let set_clause = sets.join(", ");

    let sql =
        format!("UPDATE movie_entry SET {set_clause} WHERE slug = ? RETURNING {ENTRY_COLUMNS}");
    let mut q = sqlx::query_as::<_, MovieEntry>(&sql);
    if let Some(v) = &patch.tmdb_id {
        q = q.bind(*v);
    }
    if let Some(v) = &patch.media_type {
        q = q.bind(v);
    }
    if let Some(v) = &patch.title {
        q = q.bind(v);
    }
    if let Some(v) = &patch.poster_path {
        q = q.bind(v);
    }
    if let Some(v) = patch.release_year {
        q = q.bind(v);
    }
    if let Some(v) = &patch.watched_at {
        q = q.bind(v);
    }
    if let Some(v) = patch.rating {
        q = q.bind(v);
    }
    if let Some(v) = &patch.review_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.review_en {
        q = q.bind(v);
    }
    if let Some(v) = patch.rewatch {
        q = q.bind(if v { 1i8 } else { 0i8 });
    }
    if let Some(v) = patch.series_group_id {
        q = q.bind(v);
    }
    if let Some(v) = patch.series_order {
        q = q.bind(v);
    }
    let entry = q.bind(slug).fetch_optional(pool).await?;
    Ok(entry)
}

pub async fn delete_entry(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM movie_entry WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ─── SeriesGroup ───

pub async fn create_group(
    pool: &SqlitePool,
    input: &SeriesGroupInput,
    resolved_slug: &str,
) -> anyhow::Result<SeriesGroup> {
    let group = sqlx::query_as::<_, SeriesGroup>(&format!(
        "INSERT INTO series_group
            (slug, title_ko, title_en, cover_image,
             group_rating, group_review_ko, group_review_en)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         RETURNING {GROUP_COLUMNS}"
    ))
    .bind(resolved_slug)
    .bind(input.title_ko.as_deref())
    .bind(input.title_en.as_deref())
    .bind(input.cover_image.as_deref())
    .bind(input.group_rating)
    .bind(input.group_review_ko.as_deref())
    .bind(input.group_review_en.as_deref())
    .fetch_one(pool)
    .await?;
    Ok(group)
}

pub async fn find_group_by_slug(
    pool: &SqlitePool,
    slug: &str,
) -> anyhow::Result<Option<SeriesGroup>> {
    let group = sqlx::query_as::<_, SeriesGroup>(&format!(
        "SELECT {GROUP_COLUMNS} FROM series_group WHERE slug = ?"
    ))
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(group)
}

pub async fn list_groups(pool: &SqlitePool) -> anyhow::Result<Vec<SeriesGroup>> {
    let groups = sqlx::query_as::<_, SeriesGroup>(&format!(
        "SELECT {GROUP_COLUMNS} FROM series_group ORDER BY created_at DESC"
    ))
    .fetch_all(pool)
    .await?;
    Ok(groups)
}

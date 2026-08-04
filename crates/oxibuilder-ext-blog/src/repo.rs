use crate::model::{BlogPatch, BlogPost, BlogPostInput};
use sqlx::SqlitePool;

const COLUMNS: &str = "id, slug, title, body, lang, translation_group_id, tags,
                       published_at, created_at, updated_at";

/// title 기반 slug 자동 생성.
/// 영문/숫자는 그대로 소문자, 공백/구두점은 '-', 한글 등은 그대로 (URL에 percent-encoded).
/// 결과가 빈 문자열이면 `post-<unix_ts>` 폴백.
pub fn slugify(title: &str) -> String {
    let base: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = base.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("post-{}", unix_ts())
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

/// slug 충돌 회피: 이미 존재하면 -2, -3, ... suffix.
pub async fn ensure_unique_slug(pool: &SqlitePool, base: &str) -> anyhow::Result<String> {
    let exists = |s: &str| -> bool {
        // 동기 풀이 어려우니 단순 카운트 쿼리를 직렬로. 1인 규모에서 충분.
        s.is_empty()
    };
    let _ = exists;
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

async fn slug_exists(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blog_post WHERE slug = ?")
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row.0 > 0)
}

pub async fn create(
    pool: &SqlitePool,
    input: &BlogPostInput,
    resolved_slug: &str,
) -> anyhow::Result<BlogPost> {
    let tags = serde_json::to_string(&input.tags)?;
    let post = sqlx::query_as::<_, BlogPost>(&format!(
        "INSERT INTO blog_post (slug, title, body, lang, translation_group_id, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         RETURNING {COLUMNS}"
    ))
    .bind(resolved_slug)
    .bind(&input.title)
    .bind(&input.body)
    .bind(&input.lang)
    .bind(input.translation_group_id)
    .bind(tags)
    .fetch_one(pool)
    .await?;
    Ok(post)
}

pub async fn find_by_slug(pool: &SqlitePool, slug: &str) -> anyhow::Result<Option<BlogPost>> {
    let post =
        sqlx::query_as::<_, BlogPost>(&format!("SELECT {COLUMNS} FROM blog_post WHERE slug = ?"))
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    Ok(post)
}

/// draft=true → 초안(published_at IS NULL)만.
/// draft=false → 발행본(published_at NOT NULL)만.
pub async fn list(
    pool: &SqlitePool,
    draft: bool,
    lang: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<BlogPost>> {
    let limit = limit.clamp(1, 200);
    let (sql, has_lang) = match (draft, lang.is_some()) {
        (true, true) => (
            format!(
                "SELECT {COLUMNS} FROM blog_post
                 WHERE published_at IS NULL AND lang = ?
                 ORDER BY created_at DESC LIMIT ?"
            ),
            true,
        ),
        (true, false) => (
            format!(
                "SELECT {COLUMNS} FROM blog_post
                 WHERE published_at IS NULL
                 ORDER BY created_at DESC LIMIT ?"
            ),
            false,
        ),
        (false, true) => (
            format!(
                "SELECT {COLUMNS} FROM blog_post
                 WHERE published_at IS NOT NULL AND lang = ?
                 ORDER BY published_at DESC LIMIT ?"
            ),
            true,
        ),
        (false, false) => (
            format!(
                "SELECT {COLUMNS} FROM blog_post
                 WHERE published_at IS NOT NULL
                 ORDER BY published_at DESC LIMIT ?"
            ),
            false,
        ),
    };
    let mut q = sqlx::query_as::<_, BlogPost>(&sql);
    if has_lang && let Some(l) = lang {
        q = q.bind(l);
    }
    let posts = q.bind(limit).fetch_all(pool).await?;
    Ok(posts)
}

pub async fn publish(pool: &SqlitePool, slug: &str) -> anyhow::Result<BlogPost> {
    let post = sqlx::query_as::<_, BlogPost>(&format!(
        "UPDATE blog_post
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE slug = ?
         RETURNING {COLUMNS}"
    ))
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(post)
}

pub async fn update(
    pool: &SqlitePool,
    slug: &str,
    patch: &BlogPatch,
) -> anyhow::Result<Option<BlogPost>> {
    // 부분 갱신 — 제공된 필드만.
    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = ?title");
    }
    if patch.body.is_some() {
        sets.push("body = ?body");
    }
    if patch.lang.is_some() {
        sets.push("lang = ?lang");
    }
    if patch.tags.is_some() {
        sets.push("tags = ?tags");
    }
    if sets.is_empty() {
        return find_by_slug(pool, slug).await;
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')");
    let set_clause = sets.join(", ");
    let tags_json = match &patch.tags {
        Some(t) => Some(serde_json::to_string(t)?),
        None => None,
    };
    let query_str =
        format!("UPDATE blog_post SET {set_clause} WHERE slug = ?slug RETURNING {COLUMNS}");
    let mut q = sqlx::query_as::<_, BlogPost>(&query_str);
    if let Some(v) = &patch.title {
        q = q.bind(v);
    }
    if let Some(v) = &patch.body {
        q = q.bind(v);
    }
    if let Some(v) = &patch.lang {
        q = q.bind(v);
    }
    if let Some(v) = tags_json.as_ref() {
        q = q.bind(v);
    }
    let post = q.bind(slug).fetch_optional(pool).await?;
    Ok(post)
}

pub async fn delete(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM blog_post WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// translation_group에 속한 글(자신 제외).
pub async fn list_translations(
    pool: &SqlitePool,
    group_id: i64,
    exclude_slug: &str,
) -> anyhow::Result<Vec<BlogPost>> {
    let posts = sqlx::query_as::<_, BlogPost>(&format!(
        "SELECT {COLUMNS} FROM blog_post
         WHERE translation_group_id = ? AND slug != ?
         ORDER BY lang"
    ))
    .bind(group_id)
    .bind(exclude_slug)
    .fetch_all(pool)
    .await?;
    Ok(posts)
}

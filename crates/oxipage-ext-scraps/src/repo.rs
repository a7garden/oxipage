use crate::model::{ScrapInput, ScrapItem, ScrapPatch};
use sqlx::SqlitePool;

const COLUMNS: &str = "id, source, source_item_id, source_url, title, og_image_url,
                       note_ko, note_en, tags, scraped_at, published_at,
                       created_at, updated_at";

/// manual 입력 (즉시 발행본) 생성. caller가 `source`를 `"manual"`로 정규화했음을 가정.
/// `published_at`을 now로 설정해 list에서 바로 노출되도록 한다.
pub async fn create_published(pool: &SqlitePool, input: &ScrapInput) -> anyhow::Result<ScrapItem> {
    let tags = serde_json::to_string(&input.tags)?;
    let source = crate::model::normalize_source(input.source.as_deref());
    let item = sqlx::query_as::<_, ScrapItem>(&format!(
        "INSERT INTO scrap_item
            (source, source_item_id, source_url, title, og_image_url,
             note_ko, note_en, tags, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         RETURNING {COLUMNS}"
    ))
    .bind(source)
    .bind(&input.source_item_id)
    .bind(&input.source_url)
    .bind(&input.title)
    .bind(&input.og_image_url)
    .bind(&input.note_ko)
    .bind(&input.note_en)
    .bind(tags)
    .fetch_one(pool)
    .await?;
    Ok(item)
}

/// 큐 후보 upsert. (source, source_item_id) 유니크 — 이미 있으면 갱신, 없으면 생성.
/// `published_at`은 건드리지 않는다 (큐에 머문다).
pub async fn upsert_queue_item(
    pool: &SqlitePool,
    source: &str,
    source_item_id: &str,
    source_url: &str,
    title: &str,
    og_image_url: Option<&str>,
) -> anyhow::Result<ScrapItem> {
    let item = sqlx::query_as::<_, ScrapItem>(&format!(
        "INSERT INTO scrap_item
            (source, source_item_id, source_url, title, og_image_url, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, '[]')
         ON CONFLICT (source, source_item_id) DO UPDATE SET
            source_url = excluded.source_url,
            title = excluded.title,
            og_image_url = excluded.og_image_url,
            scraped_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         RETURNING {COLUMNS}"
    ))
    .bind(source)
    .bind(source_item_id)
    .bind(source_url)
    .bind(title)
    .bind(og_image_url)
    .fetch_one(pool)
    .await?;
    Ok(item)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<ScrapItem>> {
    let item = sqlx::query_as::<_, ScrapItem>(&format!(
        "SELECT {COLUMNS} FROM scrap_item WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(item)
}

/// `source` 필터와 limit으로 발행본(또는 큐) 조회.
/// `published` true → published_at NOT NULL, false → published_at IS NULL.
pub async fn list(
    pool: &SqlitePool,
    published: bool,
    source: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<ScrapItem>> {
    let limit = limit.clamp(1, 200);
    let base_filter = if published {
        "WHERE published_at IS NOT NULL"
    } else {
        "WHERE published_at IS NULL"
    };
    let sql = if source.is_some() {
        format!(
            "SELECT {COLUMNS} FROM scrap_item
             {base_filter} AND source = ?
             ORDER BY COALESCE(published_at, scraped_at) DESC LIMIT ?"
        )
    } else {
        format!(
            "SELECT {COLUMNS} FROM scrap_item
             {base_filter}
             ORDER BY COALESCE(published_at, scraped_at) DESC LIMIT ?"
        )
    };
    let mut q = sqlx::query_as::<_, ScrapItem>(&sql);
    if let Some(s) = source {
        q = q.bind(s);
    }
    let items = q.bind(limit).fetch_all(pool).await?;
    Ok(items)
}

pub async fn update(pool: &SqlitePool, id: i64, patch: &ScrapPatch) -> anyhow::Result<Option<ScrapItem>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.note_ko.is_some() {
        sets.push("note_ko = ?");
    }
    if patch.note_en.is_some() {
        sets.push("note_en = ?");
    }
    if patch.tags.is_some() {
        sets.push("tags = ?");
    }
    if patch.og_image_url.is_some() {
        sets.push("og_image_url = ?");
    }
    if sets.is_empty() {
        return find_by_id(pool, id).await;
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')");
    let set_clause = sets.join(", ");
    let tags_json = match &patch.tags {
        Some(t) => Some(serde_json::to_string(t)?),
        None => None,
    };
    let sql = format!("UPDATE scrap_item SET {set_clause} WHERE id = ? RETURNING {COLUMNS}");
    let mut q = sqlx::query_as::<_, ScrapItem>(&sql);
    if let Some(v) = &patch.note_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.note_en {
        q = q.bind(v);
    }
    if let Some(v) = tags_json.as_ref() {
        q = q.bind(v);
    }
    if let Some(v) = &patch.og_image_url {
        q = q.bind(v);
    }
    let item = q.bind(id).fetch_optional(pool).await?;
    Ok(item)
}

/// 큐 후보를 발행본으로 승격. 호출자가 caller(AdminAuth)를 검증했다고 가정.
/// 대상 row가 없거나 이미 발행된 경우 None을 반환해 호출자가 404/409를 만들 수 있게 한다.
pub async fn publish(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<ScrapItem>> {
    let item = sqlx::query_as::<_, ScrapItem>(&format!(
        "UPDATE scrap_item
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE id = ? AND published_at IS NULL
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(item)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM scrap_item WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
use crate::model::{LinkCard, LinkCardInput, LinkCardPatch};
use sqlx::SqlitePool;

const COLUMNS: &str = "id, title, url, description_ko, description_en, thumbnail_url,
                       tags, display_order, featured, created_at, updated_at";

pub async fn create(pool: &SqlitePool, input: &LinkCardInput) -> anyhow::Result<LinkCard> {
    let tags = serde_json::to_string(&input.tags)?;
    let card = sqlx::query_as::<_, LinkCard>(&format!(
        "INSERT INTO link_card (title, url, description_ko, description_en, thumbnail_url,
                                tags, display_order, featured)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         RETURNING {COLUMNS}"
    ))
    .bind(&input.title)
    .bind(&input.url)
    .bind(&input.description_ko)
    .bind(&input.description_en)
    .bind(&input.thumbnail_url)
    .bind(tags)
    .bind(input.display_order)
    .bind(input.featured)
    .fetch_one(pool)
    .await?;
    Ok(card)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<LinkCard>> {
    let card =
        sqlx::query_as::<_, LinkCard>(&format!("SELECT {COLUMNS} FROM link_card WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(card)
}

pub async fn list(
    pool: &SqlitePool,
    featured: Option<bool>,
    limit: i64,
) -> anyhow::Result<Vec<LinkCard>> {
    let limit = limit.clamp(1, 500);
    let sql = if featured.is_some() {
        format!(
            "SELECT {COLUMNS} FROM link_card
             WHERE featured = ?
             ORDER BY display_order ASC, created_at DESC LIMIT ?"
        )
    } else {
        format!(
            "SELECT {COLUMNS} FROM link_card
             ORDER BY display_order ASC, created_at DESC LIMIT ?"
        )
    };
    let mut q = sqlx::query_as::<_, LinkCard>(&sql);
    if let Some(f) = featured {
        q = q.bind(f);
    }
    let cards = q.bind(limit).fetch_all(pool).await?;
    Ok(cards)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    patch: &LinkCardPatch,
) -> anyhow::Result<Option<LinkCard>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.title.is_some() {
        sets.push("title = ?");
    }
    if patch.url.is_some() {
        sets.push("url = ?");
    }
    if patch.description_ko.is_some() {
        sets.push("description_ko = ?");
    }
    if patch.description_en.is_some() {
        sets.push("description_en = ?");
    }
    if patch.thumbnail_url.is_some() {
        sets.push("thumbnail_url = ?");
    }
    if patch.tags.is_some() {
        sets.push("tags = ?");
    }
    if patch.display_order.is_some() {
        sets.push("display_order = ?");
    }
    if patch.featured.is_some() {
        sets.push("featured = ?");
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
    let sql = format!("UPDATE link_card SET {set_clause} WHERE id = ? RETURNING {COLUMNS}");
    let mut q = sqlx::query_as::<_, LinkCard>(&sql);
    if let Some(v) = &patch.title {
        q = q.bind(v);
    }
    if let Some(v) = &patch.url {
        q = q.bind(v);
    }
    if let Some(v) = &patch.description_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.description_en {
        q = q.bind(v);
    }
    if let Some(v) = &patch.thumbnail_url {
        q = q.bind(v);
    }
    if let Some(v) = tags_json.as_ref() {
        q = q.bind(v);
    }
    if let Some(v) = patch.display_order {
        q = q.bind(v);
    }
    if let Some(v) = patch.featured {
        q = q.bind(v);
    }
    let card = q.bind(id).fetch_optional(pool).await?;
    Ok(card)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM link_card WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

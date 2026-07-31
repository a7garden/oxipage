use crate::model::{Project, ProjectInput, ProjectPatch, Screenshot, ScreenshotPatch};
use sqlx::SqlitePool;

const COLUMNS: &str = "id, slug, title_ko, title_en, description_ko, description_en,
                       tech_stack, status, started_at, ended_at, links, featured,
                       published_at, created_at, updated_at";

pub fn slugify(title_en: Option<&str>, title_ko: Option<&str>) -> String {
    let raw = title_en
        .filter(|s| !s.is_empty())
        .or(title_ko)
        .unwrap_or("");
    let base: String = raw
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = base.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("project-{}", unix_ts())
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

pub async fn slug_exists(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM project WHERE slug = ?")
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

pub async fn create(
    pool: &SqlitePool,
    input: &ProjectInput,
    resolved_slug: &str,
) -> anyhow::Result<Project> {
    let tech_stack = serde_json::to_string(&input.tech_stack)?;
    let project = sqlx::query_as::<_, Project>(&format!(
        "INSERT INTO project (slug, title_ko, title_en, description_ko, description_en,
                              tech_stack, status, started_at, ended_at, links, featured)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         RETURNING {COLUMNS}"
    ))
    .bind(resolved_slug)
    .bind(&input.title_ko)
    .bind(&input.title_en)
    .bind(&input.description_ko)
    .bind(&input.description_en)
    .bind(tech_stack)
    .bind(&input.status)
    .bind(&input.started_at)
    .bind(&input.ended_at)
    .bind(serde_json::to_string(&input.links)?)
    .bind(input.featured)
    .fetch_one(pool)
    .await?;
    Ok(project)
}

pub async fn find_by_slug(pool: &SqlitePool, slug: &str) -> anyhow::Result<Option<Project>> {
    let p = sqlx::query_as::<_, Project>(&format!("SELECT {COLUMNS} FROM project WHERE slug = ?"))
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(p)
}

pub async fn list(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i64,
    draft: bool,
) -> anyhow::Result<Vec<Project>> {
    let limit = limit.clamp(1, 200);
    let published_clause = if draft {
        ""
    } else {
        "published_at IS NOT NULL"
    };
    let sql = if status.is_some() {
        if draft {
            format!(
                "SELECT {COLUMNS} FROM project WHERE status = ? ORDER BY featured DESC, published_at DESC, created_at DESC LIMIT ?"
            )
        } else {
            format!(
                "SELECT {COLUMNS} FROM project WHERE {published_clause} AND status = ? ORDER BY featured DESC, published_at DESC LIMIT ?"
            )
        }
    } else if draft {
        format!(
            "SELECT {COLUMNS} FROM project ORDER BY featured DESC, published_at DESC, created_at DESC LIMIT ?"
        )
    } else {
        format!(
            "SELECT {COLUMNS} FROM project WHERE {published_clause} ORDER BY featured DESC, published_at DESC LIMIT ?"
        )
    };
    let mut q = sqlx::query_as::<_, Project>(&sql);
    if let Some(s) = status {
        q = q.bind(s);
    }
    let projects = q.bind(limit).fetch_all(pool).await?;
    Ok(projects)
}

pub async fn publish(pool: &SqlitePool, slug: &str) -> anyhow::Result<Project> {
    let p = sqlx::query_as::<_, Project>(&format!(
        "UPDATE project
            SET published_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
          WHERE slug = ?
         RETURNING {COLUMNS}"
    ))
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(p)
}

pub async fn update(
    pool: &SqlitePool,
    slug: &str,
    patch: &ProjectPatch,
) -> anyhow::Result<Option<Project>> {
    let mut sets: Vec<String> = Vec::new();
    if patch.title_ko.is_some() {
        sets.push("title_ko = ?".into());
    }
    if patch.title_en.is_some() {
        sets.push("title_en = ?".into());
    }
    if patch.description_ko.is_some() {
        sets.push("description_ko = ?".into());
    }
    if patch.description_en.is_some() {
        sets.push("description_en = ?".into());
    }
    if patch.tech_stack.is_some() {
        sets.push("tech_stack = ?".into());
    }
    if patch.status.is_some() {
        sets.push("status = ?".into());
    }
    if patch.started_at.is_some() {
        sets.push("started_at = ?".into());
    }
    if patch.ended_at.is_some() {
        sets.push("ended_at = ?".into());
    }
    if patch.links.is_some() {
        sets.push("links = ?".into());
    }
    if patch.featured.is_some() {
        sets.push("featured = ?".into());
    }

    // title_ko/title_en을 둘 다 NULL로 만드는 PATCH는 거부 (체크제약 위반 방지).
    let cur = match find_by_slug(pool, slug).await? {
        Some(p) => p,
        None => return Ok(None),
    };
    let new_ko = patch.title_ko.clone().or(cur.title_ko.clone());
    let new_en = patch.title_en.clone().or(cur.title_en.clone());
    if new_ko.is_none() && new_en.is_none() {
        anyhow::bail!("title_ko and title_en cannot both be null");
    }

    if sets.is_empty() {
        return Ok(Some(cur));
    }
    sets.push("updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')".into());
    let set_clause = sets.join(", ");
    let tech_json = match &patch.tech_stack {
        Some(t) => Some(serde_json::to_string(t)?),
        None => None,
    };
    let links_json = match &patch.links {
        Some(l) => Some(serde_json::to_string(l)?),
        None => None,
    };
    let sql = format!("UPDATE project SET {set_clause} WHERE slug = ? RETURNING {COLUMNS}");
    let mut q = sqlx::query_as::<_, Project>(&sql);
    if let Some(v) = &patch.title_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.title_en {
        q = q.bind(v);
    }
    if let Some(v) = &patch.description_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.description_en {
        q = q.bind(v);
    }
    if let Some(v) = tech_json.as_ref() {
        q = q.bind(v);
    }
    if let Some(v) = &patch.status {
        q = q.bind(v);
    }
    if let Some(v) = &patch.started_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.ended_at {
        q = q.bind(v);
    }
    if let Some(v) = links_json.as_ref() {
        q = q.bind(v);
    }
    if let Some(v) = &patch.featured {
        q = q.bind(v);
    }
    let p = q.bind(slug).fetch_optional(pool).await?;
    Ok(p)
}

pub async fn delete(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let res = sqlx::query("DELETE FROM project WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ─── screenshots ───

const SCREENSHOT_COLUMNS: &str = "id, project_id, url, alt_ko, alt_en, display_order, created_at";

pub async fn add_screenshot(
    pool: &SqlitePool,
    project_slug: &str,
    url: &str,
    alt_ko: Option<&str>,
    alt_en: Option<&str>,
    display_order: i32,
) -> anyhow::Result<Screenshot> {
    let project_id: (i64,) = sqlx::query_as("SELECT id FROM project WHERE slug = ?")
        .bind(project_slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("project not found"))?;
    let shot = sqlx::query_as::<_, Screenshot>(&format!(
        "INSERT INTO screenshot (project_id, url, alt_ko, alt_en, display_order)
         VALUES (?1, ?2, ?3, ?4, ?5)
         RETURNING {SCREENSHOT_COLUMNS}"
    ))
    .bind(project_id.0)
    .bind(url)
    .bind(alt_ko)
    .bind(alt_en)
    .bind(display_order)
    .fetch_one(pool)
    .await?;
    Ok(shot)
}

pub async fn list_screenshots(
    pool: &SqlitePool,
    project_slug: &str,
) -> anyhow::Result<Vec<Screenshot>> {
    let shots = sqlx::query_as::<_, Screenshot>(&format!(
        "SELECT {SCREENSHOT_COLUMNS} FROM screenshot
         WHERE project_id = (SELECT id FROM project WHERE slug = ?)
         ORDER BY display_order ASC, id ASC"
    ))
    .bind(project_slug)
    .fetch_all(pool)
    .await?;
    Ok(shots)
}

pub async fn delete_screenshot(
    pool: &SqlitePool,
    project_slug: &str,
    sid: i64,
) -> anyhow::Result<bool> {
    let res = sqlx::query(
        "DELETE FROM screenshot
         WHERE id = ? AND project_id = (SELECT id FROM project WHERE slug = ?)",
    )
    .bind(sid)
    .bind(project_slug)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_screenshot(
    pool: &SqlitePool,
    project_slug: &str,
    sid: i64,
    patch: &ScreenshotPatch,
) -> anyhow::Result<Option<Screenshot>> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.alt_ko.is_some() {
        sets.push("alt_ko = ?");
    }
    if patch.alt_en.is_some() {
        sets.push("alt_en = ?");
    }
    if patch.display_order.is_some() {
        sets.push("display_order = ?");
    }
    if sets.is_empty() {
        let shots = sqlx::query_as::<_, Screenshot>(&format!(
            "SELECT {SCREENSHOT_COLUMNS} FROM screenshot WHERE id = ?"
        ))
        .bind(sid)
        .fetch_optional(pool)
        .await?;
        return Ok(shots);
    }
    let set_clause = sets.join(", ");
    let sql = format!(
        "UPDATE screenshot SET {set_clause} WHERE id = ?          AND project_id = (SELECT id FROM project WHERE slug = ?)          RETURNING {SCREENSHOT_COLUMNS}"
    );
    let mut q = sqlx::query_as::<_, Screenshot>(&sql);
    if let Some(v) = &patch.alt_ko {
        q = q.bind(v);
    }
    if let Some(v) = &patch.alt_en {
        q = q.bind(v);
    }
    if let Some(v) = patch.display_order {
        q = q.bind(v);
    }
    let shot = q.bind(sid).bind(project_slug).fetch_optional(pool).await?;
    Ok(shot)
}

pub async fn reorder_screenshots(
    pool: &sqlx::SqlitePool,
    project_slug: &str,
    ids: &[i64],
) -> anyhow::Result<Vec<crate::model::Screenshot>> {
    let project_id: i64 = sqlx::query_scalar("SELECT id FROM project WHERE slug = ?")
        .bind(project_slug)
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;
    let current: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM project_screenshot WHERE project_id = ? ORDER BY display_order",
    )
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;
    let current_ids: Vec<i64> = current.into_iter().map(|(i,)| i).collect();
    if current_ids.len() != ids.len()
        || current_ids.iter().collect::<std::collections::HashSet<_>>()
            != ids.iter().collect::<std::collections::HashSet<_>>()
    {
        anyhow::bail!("stale_order: submitted IDs do not match current screenshot set");
    }
    for (idx, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE project_screenshot SET display_order = ?1 WHERE id = ?2 AND project_id = ?3",
        )
        .bind(idx as i32)
        .bind(id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    }
    let updated = sqlx::query_as::<_, crate::model::Screenshot>(&format!(
        "SELECT id, project_id, url, alt_ko, alt_en, display_order, created_at FROM project_screenshot WHERE project_id = ? ORDER BY display_order"
    ))
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(updated)
}

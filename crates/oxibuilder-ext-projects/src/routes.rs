use crate::model::{
    ListQuery, ProjectDetail, ProjectInput, ProjectPatch, ScreenshotInput, ScreenshotPatch,
};
use crate::repo;
use axum::Json;
use axum::extract::{Extension, Path, Query};

use oxibuilder_core::error::ApiError;
use oxibuilder_core::extension::DataEnvelope;
use oxibuilder_core::search;
use oxibuilder_core::state::SiteScopedDb;
use sqlx::SqlitePool;

pub async fn list(
    Extension(pool): Extension<SiteScopedDb>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<crate::model::Project>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let projects = repo::list(&pool.db, q.status.as_deref(), limit, q.draft)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: projects }))
}

pub async fn create(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<ProjectInput>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    validate_input(&input)?;
    let base_slug = input
        .slug
        .clone()
        .unwrap_or_else(|| repo::slugify(input.title_en.as_deref(), input.title_ko.as_deref()));
    let slug = repo::ensure_unique_slug(&pool.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;
    let project = repo::create(&pool.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: project }))
}

pub async fn show(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<ProjectDetail>>, ApiError> {
    let project = repo::find_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    if project.published_at.is_none() {
        return Err(not_found(&slug));
    }
    let screenshots = repo::list_screenshots(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: ProjectDetail {
            project,
            screenshots,
        },
    }))
}

pub async fn update(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(patch): Json<ProjectPatch>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    if let Some(ref status) = patch.status
        && !matches!(status.as_str(), "active" | "archived" | "wip")
    {
        return Err(ApiError::validation(
            "status",
            "status must be active|archived|wip",
        ));
    }
    let project = match repo::update(&pool.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
    {
        Some(p) => p,
        None => return Err(not_found(&slug)),
    };
    if project.published_at.is_some() {
        reindex(&pool.db, &project).await?;
    }
    Ok(Json(DataEnvelope { data: project }))
}

pub async fn delete(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&pool.db, "projects", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    if repo::find_by_slug(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let project = repo::publish(&pool.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    reindex(&pool.db, &project).await?;
    let title = project
        .title_en
        .clone()
        .or_else(|| project.title_ko.clone())
        .unwrap_or_else(|| project.slug.clone());
    let description = project
        .description_en
        .clone()
        .or_else(|| project.description_ko.clone())
        .unwrap_or_default();
    let _truncated: String = description.chars().take(200).collect();
    let _body_md = format!(
        "{title}\n\n{description}\n\ntech_stack: {}",
        project.tech_stack.join(", ")
    );
    let _snapshot_path = format!("/projects/{}", project.slug);
    Ok(Json(DataEnvelope { data: project }))
}

pub async fn add_screenshot(
    Extension(pool): Extension<SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<ScreenshotInput>,
) -> Result<Json<DataEnvelope<crate::model::Screenshot>>, ApiError> {
    if input.url.trim().is_empty() {
        return Err(ApiError::validation("url", "url must not be empty"));
    }
    let shot = repo::add_screenshot(
        &pool.db,
        &slug,
        &input.url,
        input.alt_ko.as_deref(),
        input.alt_en.as_deref(),
        input.display_order,
    )
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: shot }))
}

pub async fn delete_screenshot(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, sid)): Path<(String, i64)>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_screenshot(&pool.db, &slug, sid)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&format!("{slug}/screenshots/{sid}")));
    }
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": sid, "deleted": true }),
    }))
}

pub async fn update_screenshot(
    Extension(pool): Extension<SiteScopedDb>,
    Path((slug, sid)): Path<(String, i64)>,
    Json(patch): Json<ScreenshotPatch>,
) -> Result<Json<DataEnvelope<crate::model::Screenshot>>, ApiError> {
    let shot = repo::update_screenshot(&pool.db, &slug, sid, &patch)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&format!("{slug}/screenshots/{sid}")))?;
    Ok(Json(DataEnvelope { data: shot }))
}

async fn reindex(db: &SqlitePool, project: &crate::model::Project) -> Result<(), ApiError> {
    let title = project
        .title_en
        .clone()
        .or_else(|| project.title_ko.clone())
        .unwrap_or_default();
    let body = project
        .description_en
        .clone()
        .or_else(|| project.description_ko.clone())
        .unwrap_or_default();
    let lang = if project.title_en.is_some() || project.description_en.is_some() {
        "en"
    } else {
        "ko"
    };
    search::upsert(
        db,
        "projects",
        &project.slug,
        &title,
        &body,
        Some(lang),
        project.published_at.as_deref(),
    )
    .await
    .map_err(ApiError::internal)
}

fn validate_input(input: &ProjectInput) -> Result<(), ApiError> {
    let ko_empty = input
        .title_ko
        .as_deref()
        .is_none_or(|s| s.trim().is_empty());
    let en_empty = input
        .title_en
        .as_deref()
        .is_none_or(|s| s.trim().is_empty());
    if ko_empty && en_empty {
        return Err(ApiError::validation(
            "title_ko",
            "title_ko or title_en must be non-empty",
        ));
    }
    if !matches!(input.status.as_str(), "active" | "archived" | "wip") {
        return Err(ApiError::validation(
            "status",
            "status must be active|archived|wip",
        ));
    }
    if let (Some(s), Some(e)) = (&input.started_at, &input.ended_at)
        && !s.is_empty()
        && !e.is_empty()
        && s > e
    {
        return Err(ApiError::validation(
            "ended_at",
            "ended_at must not precede started_at",
        ));
    }
    Ok(())
}

fn not_found(slug: &str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        &format!("project '{slug}' not found"),
    )
}

pub async fn reorder_screenshots(
    Extension(pool): Extension<oxibuilder_core::state::SiteScopedDb>,
    Path(slug): Path<String>,
    Json(input): Json<crate::model::ScreenshotOrderInput>,
) -> Result<Json<DataEnvelope<Vec<crate::model::Screenshot>>>, ApiError> {
    use std::collections::HashSet;
    if input.screenshot_ids.iter().collect::<HashSet<_>>().len() != input.screenshot_ids.len() {
        return Err(ApiError::validation(
            "screenshot_ids",
            "screenshot_ids contains duplicates",
        ));
    }
    let shots = repo::reorder_screenshots(&pool.db, &slug, &input.screenshot_ids)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.starts_with("stale_order") {
                ApiError::new(
                    axum::http::StatusCode::CONFLICT,
                    "stale_order",
                    "submitted IDs do not match current screenshot set",
                )
            } else {
                ApiError::internal(e)
            }
        })?;
    Ok(Json(DataEnvelope { data: shots }))
}

use crate::model::{ListQuery, ProjectDetail, ProjectInput, ProjectPatch, ScreenshotInput};
use crate::repo;
use axum::Json;
use axum::extract::{Path, Query, State};
use oxipage_core::auth::AdminAuth;
use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::search;
use oxipage_core::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<crate::model::Project>>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let projects = repo::list(&state.db, q.status.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: projects }))
}

pub async fn create(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<ProjectInput>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    validate_input(&input)?;
    let base_slug = input
        .slug
        .clone()
        .unwrap_or_else(|| repo::slugify(input.title_en.as_deref(), input.title_ko.as_deref()));
    let slug = repo::ensure_unique_slug(&state.db, &base_slug)
        .await
        .map_err(ApiError::internal)?;
    let project = repo::create(&state.db, &input, &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: project }))
}

pub async fn show(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<ProjectDetail>>, ApiError> {
    let project = repo::find_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| not_found(&slug))?;
    if project.published_at.is_none() {
        return Err(not_found(&slug));
    }
    let screenshots = repo::list_screenshots(&state.db, &slug)
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
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(patch): Json<ProjectPatch>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    if let Some(ref status) = patch.status
        && !matches!(status.as_str(), "active" | "archived" | "wip")
    {
        return Err(ApiError::validation("status", "status must be active|archived|wip"));
    }
    let project = match repo::update(&state.db, &slug, &patch)
        .await
        .map_err(ApiError::internal)?
    {
        Some(p) => p,
        None => return Err(not_found(&slug)),
    };
    if project.published_at.is_some() {
        reindex(&state, &project).await?;
    }
    Ok(Json(DataEnvelope { data: project }))
}

pub async fn delete(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&slug));
    }
    search::delete(&state.db, "projects", &slug)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "slug": slug, "deleted": true }),
    }))
}

pub async fn publish(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<DataEnvelope<crate::model::Project>>, ApiError> {
    auth.require_scope("post:publish")?;
    if repo::find_by_slug(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(not_found(&slug));
    }
    let project = repo::publish(&state.db, &slug)
        .await
        .map_err(ApiError::internal)?;
    reindex(&state, &project).await?;
    let title = project.title_en.clone()
        .or_else(|| project.title_ko.clone())
        .unwrap_or_else(|| project.slug.clone());
    let description = project.description_en.clone()
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
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<ScreenshotInput>,
) -> Result<Json<DataEnvelope<crate::model::Screenshot>>, ApiError> {
    if input.url.trim().is_empty() {
        return Err(ApiError::validation("url", "url must not be empty"));
    }
    let shot = repo::add_screenshot(
        &state.db,
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
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path((slug, sid)): Path<(String, i64)>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    let removed = repo::delete_screenshot(&state.db, &slug, sid)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(not_found(&format!("{slug}/screenshots/{sid}")));
    }
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": sid, "deleted": true }),
    }))
}

async fn reindex(state: &AppState, project: &crate::model::Project) -> Result<(), ApiError> {
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
        &state.db,
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
    let ko_empty = input.title_ko.as_deref().is_none_or(|s| s.trim().is_empty());
    let en_empty = input.title_en.as_deref().is_none_or(|s| s.trim().is_empty());
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

use crate::client::GithubClient;
use crate::model::{GithubEvent, ListQuery};
use crate::repo;
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use oxipage_core::auth::AdminAuth;
use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<crate::model::ActivityEvent>>>, ApiError> {
    let events = repo::list(&state.db, query.repo.as_deref(), query.limit.unwrap_or(30))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: events }))
}

pub async fn webhook(
    State(state): State<AppState>,
    Json(event): Json<GithubEvent>,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_event(&event)?;
    repo::upsert(&state.db, &event.into_input())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn sync(
    _auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let client = GithubClient::with_username(state.config.integrations.github_username())
        .map_err(ApiError::internal)?;
    if !client.enabled() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "integration_disabled",
            "GitHub activity sync requires integrations.github_username or OXIPAGE_GITHUB_USERNAME",
        ));
    }
    let events = client
        .fetch_public_events()
        .await
        .map_err(ApiError::internal)?;
    let count = events.len();
    for event in events {
        validate_event(&event)?;
        repo::upsert(&state.db, &event.into_input())
            .await
            .map_err(ApiError::internal)?;
    }
    Ok(Json(serde_json::json!({ "status": "ok", "synced": count })))
}

fn validate_event(event: &GithubEvent) -> Result<(), ApiError> {
    if event.kind.trim().is_empty() {
        return Err(ApiError::validation("type", "event type must not be empty"));
    }
    if event.repo.name.trim().is_empty() {
        return Err(ApiError::validation(
            "repo.name",
            "repository name must not be empty",
        ));
    }
    if event.created_at.trim().is_empty() {
        return Err(ApiError::validation(
            "created_at",
            "occurred time must not be empty",
        ));
    }
    Ok(())
}

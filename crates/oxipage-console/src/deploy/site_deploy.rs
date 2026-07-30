//! `POST /api/console/s/{slug}/deploy` — stub. The full deployment pipeline
//! lives in the `oxipage deploy` CLI (git worktree, gh-pages, etc.) — this
//! route is a placeholder pending module integration.

use crate::sites_runtime::SiteRegistry;
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct DeployResponse {
    pub data: DeployOutput,
}

#[derive(Serialize)]
pub struct DeployOutput {
    pub slug: String,
    pub status: &'static str,
    pub note: &'static str,
}

pub(crate) async fn deploy_handler(
    State(_registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Json<DeployResponse> {
    Json(DeployResponse {
        data: DeployOutput {
            slug,
            status: "queued",
            note: "Deploy is currently invoked via `oxipage deploy --site <slug>`; console route is a stub pending module integration.",
        },
    })
}

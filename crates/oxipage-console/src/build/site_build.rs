//! `POST /api/console/s/{slug}/build` — trigger SSG build for one site.

use crate::sites_runtime::SiteRegistry;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct BuildResult {
    pub data: BuildOutput,
}

#[derive(Serialize)]
pub struct BuildOutput {
    pub out_dir: String,
    pub page_count: usize,
}

pub fn router() -> Router<Arc<SiteRegistry>> {
    Router::new().route("/build", post(build_handler))
}

pub(crate) async fn build_handler(
    State(registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Result<Json<BuildResult>, (axum::http::StatusCode, String)> {
    let ctx = registry.ctx_for(&slug).await.ok_or((
        axum::http::StatusCode::NOT_FOUND,
        "site_not_found".to_string(),
    ))?;
    let out_dir = ctx.path.join("out");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let output = oxipage_core::build::build_site(&ctx.db, &ctx.builders)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let media_dir = ctx.config.server.data_dir.join("media");
    oxipage_core::build_writer::write_build_output(&output, &out_dir, &media_dir)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(BuildResult {
        data: BuildOutput {
            out_dir: out_dir.to_string_lossy().into_owned(),
            page_count: output.pages.len(),
        },
    }))
}

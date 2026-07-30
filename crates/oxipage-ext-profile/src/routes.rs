use crate::model::{Profile, ProfileInput};
use crate::repo;
use axum::Json;
use axum::extract::Extension;

use oxipage_core::error::ApiError;
use oxipage_core::state::SiteScopedDb;

#[derive(serde::Serialize)]
pub struct DataEnvelope<T: serde::Serialize> {
    pub data: T,
}

pub async fn get_profile(
    Extension(pool): Extension<SiteScopedDb>,
) -> Result<Json<DataEnvelope<Profile>>, ApiError> {
    let profile = repo::get(&pool.db).await.map_err(ApiError::internal)?;
    match profile {
        Some(p) => Ok(Json(DataEnvelope { data: p })),
        None => Err(ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "profile is not initialized",
        )),
    }
}

pub async fn put_profile(
    Extension(pool): Extension<SiteScopedDb>,
    Json(input): Json<ProfileInput>,
) -> Result<Json<DataEnvelope<Profile>>, ApiError> {
    if input.display_name.trim().is_empty() {
        return Err(ApiError::validation(
            "display_name",
            "display_name must not be empty",
        ));
    }
    let profile = repo::upsert(&pool.db, &input)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: profile }))
}

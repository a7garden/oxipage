use crate::model::{Profile, ProfileInput};
use crate::repo;
use axum::Json;
use axum::extract::Extension;
use axum::http::StatusCode;

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
            StatusCode::NOT_FOUND,
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
    if let Some(email) = &input.email
        && !email.is_empty()
        && !crate::model::validate_email(email)
    {
        return Err(ApiError::validation(
            "email",
            "email is not a valid address",
        ));
    }
    for e in &input.education {
        if !crate::model::validate_year_range(e.start_year, e.end_year) {
            return Err(ApiError::validation(
                "education",
                "education end_year must be >= start_year",
            ));
        }
    }
    // On stale detection, read the current remote row so the client can
    // offer Reload/Compare instead of overwriting silently. The 409 body
    // carries `{ error: {...}, data: <current Profile> }`.
    let profile = match repo::upsert_if_unchanged(&pool.db, &input).await {
        Err(repo::UpsertError::Stale { expected: _ }) => {
            let remote = match repo::get(&pool.db).await {
                Ok(Some(p)) => p,
                _ => {
                    return Err(ApiError::internal(anyhow::anyhow!(
                        "profile vanished during stale write"
                    )));
                }
            };
            return Err(ApiError::with_data(
                StatusCode::CONFLICT,
                "stale_profile",
                "profile changed since your last load; reload to see remote changes",
                &remote,
            ));
        }
        Err(repo::UpsertError::Db(err)) => return Err(ApiError::internal(err)),
        Ok(p) => p,
    };
    Ok(Json(DataEnvelope { data: profile }))
}

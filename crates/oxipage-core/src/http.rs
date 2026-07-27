use crate::auth::{self, AdminAuth, PatRow};
use crate::error::ApiError;
use crate::extension::Lang;
use crate::search::SearchHit;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use tower_http::trace::TraceLayer;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

#[derive(serde::Serialize)]
struct DataEnvelope<T: serde::Serialize> {
    data: T,
}

#[derive(serde::Serialize)]
struct ManifestSite {
    name: String,
    base_url: String,
    default_lang: String,
    languages: Vec<String>,
}

#[derive(serde::Serialize)]
struct ManifestLocalized {
    ko: String,
    en: String,
}

#[derive(serde::Serialize)]
struct ManifestExtension {
    id: &'static str,
    display_name: ManifestLocalized,
    lobby: LobbyConfigInfo,
}

#[derive(serde::Serialize, Clone, Default)]
struct LobbyConfigInfo {
    enabled: bool,
    display_mode: String,
    display_order: i64,
    style_params: serde_json::Value,
}

#[derive(serde::Serialize)]
struct Manifest {
    site: ManifestSite,
    extensions: Vec<ManifestExtension>,
}

pub fn build_app(state: AppState) -> Router {
    let mut api = Router::new()
        .route("/lobby/manifest", get(lobby_manifest))
        .route("/lobby/config", get(lobby_config_list))
        .route("/lobby/config/{ext_id}", axum::routing::put(lobby_config_update))
        .route("/auth/tokens", get(auth_tokens_list).post(auth_tokens_create))
        .route("/auth/tokens/{id}", axum::routing::delete(auth_tokens_revoke))
        .route("/search", get(search_handler))
        .route("/docs", get(docs_ui))
        .route("/docs/openapi.json", get(docs_spec));
    for ext in state.registry.iter() {
        api = api.nest(&format!("/{}", ext.id()), ext.routes());
    }
    let api = api.fallback(api_not_found);

    let limiter = crate::rate_limit::RateLimiter::new(120); // IP당 120/min
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api)
        .fallback(static_handler)
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(
            limiter,
            crate::rate_limit::rate_limit_middleware,
        ))
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}


async fn docs_spec(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(crate::openapi::openapi_spec(&state.config.site.base_url))
}

async fn docs_ui() -> axum::response::Response {
    let html = crate::openapi::swagger_ui_html("/api/v1/docs/openapi.json");
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}
async fn api_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "resource not found")
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
    lang: Option<String>,
    limit: Option<i64>,
}

async fn search_handler(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<DataEnvelope<Vec<SearchHit>>>, ApiError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let hits = crate::search::search(&state.db, &q.q, q.lang.as_deref(), limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: hits }))
}

async fn lobby_manifest(State(state): State<AppState>) -> Json<DataEnvelope<Manifest>> {
    let mut extensions = Vec::new();
    for (idx, e) in state.registry.iter().enumerate() {
        let lobby = lobby_config_for(&state, e.id(), idx as i64).await;
        extensions.push(ManifestExtension {
            id: e.id(),
            display_name: ManifestLocalized {
                ko: e.display_name(Lang::Ko),
                en: e.display_name(Lang::En),
            },
            lobby,
        });
    }
    Json(DataEnvelope {
        data: Manifest {
            site: ManifestSite {
                name: state.config.site.name.clone(),
                base_url: state.config.site.base_url.clone(),
                default_lang: state.config.site.default_lang.clone(),
                languages: state.config.site.languages.clone(),
            },
            extensions,
        },
    })
}

async fn lobby_config_for(state: &AppState, ext_id: &str, default_order: i64) -> LobbyConfigInfo {
    let row: Option<(bool, String, i64, String)> = sqlx::query_as(
        "SELECT enabled, display_mode, display_order, style_params
         FROM lobby_config WHERE extension_id = ?",
    )
    .bind(ext_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    match row {
        Some((enabled, mode, order, params)) => LobbyConfigInfo {
            enabled,
            display_mode: mode,
            display_order: order,
            style_params: serde_json::from_str(&params).unwrap_or_default(),
        },
        None => LobbyConfigInfo {
            enabled: true,
            display_mode: state.config.lobby.default_mode.clone(),
            display_order: default_order,
            style_params: serde_json::json!({}),
        },
    }
}

async fn lobby_config_list(
    State(state): State<AppState>,
) -> Json<DataEnvelope<Vec<LobbyConfigEntry>>> {
    let mut entries = Vec::new();
    for (idx, e) in state.registry.iter().enumerate() {
        let info = lobby_config_for(&state, e.id(), idx as i64).await;
        entries.push(LobbyConfigEntry {
            extension_id: e.id().to_string(),
            enabled: info.enabled,
            display_mode: info.display_mode,
            display_order: info.display_order,
            style_params: info.style_params,
        });
    }
    Json(DataEnvelope { data: entries })
}

#[derive(serde::Serialize)]
struct LobbyConfigEntry {
    extension_id: String,
    enabled: bool,
    display_mode: String,
    display_order: i64,
    style_params: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct LobbyConfigUpdate {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    display_mode: Option<String>,
    #[serde(default)]
    display_order: Option<i64>,
    #[serde(default)]
    style_params: Option<serde_json::Value>,
}

async fn lobby_config_update(
    _auth: AdminAuth,
    State(state): State<AppState>,
    Path(ext_id): Path<String>,
    Json(input): Json<LobbyConfigUpdate>,
) -> Result<Json<DataEnvelope<LobbyConfigEntry>>, ApiError> {
    if state.registry.find(&ext_id).is_none() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "extension not registered",
        ));
    }
    if let Some(ref mode) = input.display_mode
        && !matches!(mode.as_str(), "canvas" | "grid" | "list")
    {
        return Err(ApiError::validation(
            "display_mode",
            "display_mode must be canvas|grid|list",
        ));
    }
    let info = lobby_config_for(&state, &ext_id, 0).await;
    let enabled = input.enabled.unwrap_or(info.enabled);
    let mode = input.display_mode.unwrap_or(info.display_mode);
    let order = input.display_order.unwrap_or(info.display_order);
    let params = input.style_params.unwrap_or(info.style_params);
    let params_str = serde_json::to_string(&params).unwrap_or_else(|_| "{}".into());

    sqlx::query(
        "INSERT INTO lobby_config (extension_id, enabled, display_mode, display_order, style_params)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (extension_id) DO UPDATE SET
            enabled = ?2, display_mode = ?3, display_order = ?4, style_params = ?5",
    )
    .bind(&ext_id)
    .bind(enabled)
    .bind(&mode)
    .bind(order)
    .bind(&params_str)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;

    Ok(Json(DataEnvelope {
        data: LobbyConfigEntry {
            extension_id: ext_id,
            enabled,
            display_mode: mode,
            display_order: order,
            style_params: params,
        },
    }))
}

// ─── auth tokens (PAT, doc/01 §1.8, doc/04 §4.2) ───

#[derive(serde::Deserialize)]
struct PatCreate {
    label: String,
    #[serde(default)]
    scopes: Vec<String>,
}

#[derive(serde::Serialize)]
struct PatCreated {
    plain_token: String,
    id: i64,
    label: String,
    token_prefix: String,
    scopes: Vec<String>,
}

async fn auth_tokens_create(
    auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<PatCreate>,
) -> Result<Json<DataEnvelope<PatCreated>>, ApiError> {
    auth.require_scope("admin")?;
    if input.label.trim().is_empty() {
        return Err(ApiError::validation("label", "label must not be empty"));
    }
    for s in &input.scopes {
        if !matches!(s.as_str(), "post:write" | "post:publish" | "read" | "admin") {
            return Err(ApiError::validation(
                "scopes",
                "scope must be post:write|post:publish|read|admin",
            ));
        }
    }
    let (plain, row) = auth::create_pat(&state, &input.label, &input.scopes)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope {
        data: PatCreated {
            plain_token: plain,
            id: row.id,
            label: row.label,
            token_prefix: row.token_prefix,
            scopes: input.scopes,
        },
    }))
}

async fn auth_tokens_list(
    auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<Vec<PatRow>>>, ApiError> {
    auth.require_scope("admin")?;
    let rows = auth::list_pats(&state)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: rows }))
}

async fn auth_tokens_revoke(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    auth.require_scope("admin")?;
    let removed = auth::revoke_pat(&state, id)
        .await
        .map_err(ApiError::internal)?;
    if !removed {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "active token with that id not found",
        ));
    }
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "id": id, "revoked": true }),
    }))
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    serve_asset(path)
        .or_else(|| serve_asset("index.html"))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn serve_asset(path: &str) -> Option<Response> {
    Assets::get(path).map(|content| {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    })
}

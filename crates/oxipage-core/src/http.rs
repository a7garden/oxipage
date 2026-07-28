use crate::auth::{self, AdminAuth, PatRow};
use crate::error::ApiError;
use crate::extension::{Extension, Lang};
use crate::search::SearchHit;
use crate::state::AppState;
use std::sync::Arc;
use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use axum::middleware::Next;
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
    id: String,
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
        .route("/docs/openapi.json", get(docs_spec))
        .route("/extensions", get(extensions_list))
        .route("/extensions/{id}/enable", axum::routing::post(extension_enable))
        .route("/extensions/{id}/disable", axum::routing::post(extension_disable))
        .route("/extensions/{id}", axum::routing::delete(extension_purge))
        .route("/extensions/install", axum::routing::post(extension_install));
    for ext in state.registry.iter() {
        api = api.nest(&format!("/{}", ext.id()), ext.routes());
    }
    let api = api
        .fallback(api_not_found)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            extension_gate,
        ));

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
        if !state.registry.is_active(e.id()).await {
            continue;
        }
        let lobby = lobby_config_for(&state, e.id(), idx as i64).await;
        extensions.push(ManifestExtension {
            id: e.id().to_string(),
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
        if !state.registry.is_active(e.id()).await {
            continue;
        }
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

/// SSR 스냅샷용 — `web/dist/index.html`의 UTF-8 본문을 반환한다.
/// 없으면(개발 환경 등에서 `web/dist` 미빌드) `None`.
pub fn spa_index_html() -> Option<String> {
    Assets::get("index.html")
        .and_then(|f| std::str::from_utf8(f.data.as_ref()).ok().map(str::to_owned))
}

// ─── extension lifecycle (doc/02 §2.13, doc/04 §4.3) ───

#[derive(serde::Serialize)]
struct ExtensionInfo {
    id: String,
    display_name: ManifestLocalized,
    enabled: bool,
    purged: bool,
}

async fn extension_info(state: &AppState, ext: &Arc<dyn Extension>) -> ExtensionInfo {
    let s = state.registry.status_of(ext.id()).await;
    ExtensionInfo {
        id: ext.id().to_string(),
        display_name: ManifestLocalized {
            ko: ext.display_name(Lang::Ko),
            en: ext.display_name(Lang::En),
        },
        enabled: s.map(|s| s.enabled).unwrap_or(false),
        purged: s.map(|s| s.purged).unwrap_or(false),
    }
}

async fn extensions_list(
    auth: AdminAuth,
    State(state): State<AppState>,
) -> Result<Json<DataEnvelope<Vec<ExtensionInfo>>>, ApiError> {
    auth.require_scope("admin")?;
    let snapshot = state.registry.status_snapshot().await;
    let mut out = Vec::new();
    for e in state.registry.iter() {
        let s = snapshot.get(e.id()).copied();
        out.push(ExtensionInfo {
            id: e.id().to_string(),
            display_name: ManifestLocalized {
                ko: e.display_name(Lang::Ko),
                en: e.display_name(Lang::En),
            },
            enabled: s.map(|s| s.enabled).unwrap_or(false),
            purged: s.map(|s| s.purged).unwrap_or(false),
        });
    }
    Ok(Json(DataEnvelope { data: out }))
}

async fn extension_enable(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DataEnvelope<ExtensionInfo>>, ApiError> {
    auth.require_scope("admin")?;
    let ext = state
        .registry
        .find(&id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "extension_not_found", "unknown extension id"))?;
    let prev = state.registry.status_of(&id).await;
    let was_purged = prev.map(|s| s.purged).unwrap_or(false);
    let was_enabled = prev.map(|s| s.enabled).unwrap_or(false);
    if was_purged {
        // purge 복구: 플래그 클리어. 실제 마이그레이션은 다음 부팅 시 run_migrations가
        // schema_migrations 행 부재를 감지해 재실행한다 (run_migrations future가
        // 핸들러 컨텍스트에서 non-Send라 직접 await 불가 — 부팅 시점에 해결).
        state.registry.set_purged(&state.db, &id, false).await?;
    }
    state.registry.set_enabled(&state.db, &id, true).await?;
    if !was_enabled || was_purged {
        match ext.on_startup(&state).await {
            Ok(_) => {}
            Err(e) => tracing::warn!(extension = %id, error = %e, "on_startup failed"),
        }
    }
    Ok(Json(DataEnvelope {
        data: extension_info(&state, &ext).await,
    }))
}

async fn extension_disable(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DataEnvelope<ExtensionInfo>>, ApiError> {
    auth.require_scope("admin")?;
    let ext = state
        .registry
        .find(&id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "extension_not_found", "unknown extension id"))?;
    let prev = state.registry.set_enabled(&state.db, &id, false).await?;
    if prev.map(|s| s.enabled).unwrap_or(false) {
        // enabled→disabled 전환 시에만 FTS 색인 즉시 정리 (doc/02 §2.13).
        match ext.on_disable(&state).await {
            Ok(_) => {}
            Err(e) => tracing::warn!(extension = %id, error = %e, "on_disable failed"),
        }
    }
    Ok(Json(DataEnvelope {
        data: extension_info(&state, &ext).await,
    }))
}

async fn extension_purge(
    auth: AdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    auth.require_scope("admin")?;
    let ext = state
        .registry
        .find(&id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "extension_not_found", "unknown extension id"))?;
    // 1. disable + FTS 정리.
    state.registry.set_enabled(&state.db, &id, false).await?;
    let _ = ext.on_disable(&state).await;
    // 2. 확장 테이블 DROP. table_names()는 &'static str이지만 방어적으로 식별자 검증.
    for table in ext.table_names() {
        if !is_safe_ident(table) {
            return Err(ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "unsafe_table_name",
                "extension returned an invalid table name",
            ));
        }
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(&state.db)
            .await
            .map_err(|e| ApiError::from(anyhow::anyhow!(e)))?;
    }
    // 3. 미디어 디렉토리 rm (data/media/{id}/).
    let media = state.config.server.data_dir.join("media").join(&id);
    if media.exists() {
        match std::fs::remove_dir_all(&media) {
            Ok(_) => {}
            Err(e) => tracing::warn!(path = %media.display(), error = %e, "failed to remove media dir during purge"),
        }
    }
    // 4. 마이그레이션 기록 제거 — enable-after-purge가 재실행하도록.
    sqlx::query("DELETE FROM schema_migrations WHERE extension = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| ApiError::from(anyhow::anyhow!(e)))?;
    // 5. purge 플래그 세팅 (부팅 시 마이그레이션 스킵).
    state.registry.set_purged(&state.db, &id, true).await?;
    Ok(Json(DataEnvelope {
        data: serde_json::json!({ "extension_id": id, "purged": true }),
    }))
}

// ─── wasm runtime install (doc/08 §8.4) ───
//
// install 은 wasm 바이트를 data/extensions/<name>.wasm 에 저장하고 extension_state
// 행을 추가만 한다. 실제 적재(인스턴스화)는 oxipage-wasm 이 다음 부팅 시 수행
// (server `wasm` feature). 따라서 이 핸들러 자체는 wasmtime/oxipage-wasm 에 의존하지
// 않는다 — 코어 어디서나 동작.

/// 임베드된 레지스트리 카탈로그 (빌드 시점 snapshot of registry/index.json).
const REGISTRY_INDEX_JSON: &str = include_str!("../../../registry/index.json");
/// 임베드된 데모 wasm 아티팩트 — install 오프라인 검증용 (remote 다운로드 경로와 별개).
const DEMO_WASM_BYTES: &[u8] =
    include_bytes!("../../../crates/oxipage-ext-wasm-demo/artifacts/wasm-demo.wasm");

#[derive(serde::Deserialize)]
struct RegistryIndex {
    extensions: Vec<RegistryEntry>,
}

#[derive(serde::Deserialize)]
struct RegistryEntry {
    name: String,
    #[serde(default)]
    runtime_loadable: bool,
    #[serde(default)]
    wasm_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct InstallInput {
    name: String,
}

async fn extension_install(
    auth: AdminAuth,
    State(state): State<AppState>,
    Json(input): Json<InstallInput>,
) -> Result<Json<DataEnvelope<serde_json::Value>>, ApiError> {
    auth.require_scope("admin")?;
    let name = input.name;
    if !is_safe_extension_name(&name) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "invalid extension name",
        ));
    }
    let index: RegistryIndex = serde_json::from_str(REGISTRY_INDEX_JSON)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "registry_error", &e.to_string()))?;
    let entry = index
        .extensions
        .iter()
        .find(|e| e.name == name)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "unknown extension name"))?;
    if !entry.runtime_loadable {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "not_runtime_loadable",
            "this extension is compile-time only; it cannot be installed at runtime",
        ));
    }
    // wasm 바이트 획득: 데모는 임베드, 그 외는 wasm_url 에서 다운로드.
    let bytes: Vec<u8> = if name == "wasm-demo" {
        DEMO_WASM_BYTES.to_vec()
    } else {
        let url = entry
            .wasm_url
            .as_deref()
            .ok_or_else(|| ApiError::new(StatusCode::CONFLICT, "no_wasm_url", "registry entry has no wasm_url"))?;
        let resp = reqwest::get(url)
            .await
            .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "download_failed", &e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "download_failed",
                &format!("registry returned {}", resp.status()),
            ));
        }
        resp.bytes()
            .await
            .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "download_failed", &e.to_string()))?
            .to_vec()
    };
    let dir = state.config.server.data_dir.join("extensions");
    std::fs::create_dir_all(&dir)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "fs_error", &e.to_string()))?;
    let path = dir.join(format!("{name}.wasm"));
    std::fs::write(&path, &bytes)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "fs_error", &e.to_string()))?;
    // extension_state 행 (enabled=0). 다음 부팅 시 load_all_from_dir 이 적재.
    sqlx::query(
        "INSERT INTO extension_state (extension_id, enabled, purged)
         VALUES (?1, 0, 0)
         ON CONFLICT(extension_id) DO UPDATE SET purged = 0",
    )
    .bind(&name)
    .execute(&state.db)
    .await
    .map_err(|e| ApiError::from(anyhow::anyhow!(e)))?;
    tracing::info!(
        extension = %name,
        path = %path.display(),
        bytes = bytes.len(),
        "installed wasm extension (restart to activate)"
    );
    Ok(Json(DataEnvelope {
        data: serde_json::json!({
            "name": name,
            "path": path.display().to_string(),
            "bytes": bytes.len(),
            "note": "restart oxipage-server (built with --features wasm) to activate",
        }),
    }))
}

fn is_safe_ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 런타임 설치 확장 이름 검증. is_safe_ident 와 달리 하이픈을 허용한다 —
/// 이름은 파일명(data/extensions/<name>.wasm)과 매개변수화 SQL 바인드에만 쓰이므로
/// 하이픈이 안전하다 (경로 순회 차단: `/` `\` `.` 는 여전히 거부). SQL 식별자로
/// 보간되는 table_names() 에는 이 함수를 쓰면 안 된다 — is_safe_ident 를 쓸 것.
fn is_safe_extension_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `/api/v1/{ext}/**` 경로에서 ext 세그먼트 추출. 코어 라우트(lobby/auth/search/docs)는
/// registry.find 가 None이므로 게이트 대상이 아니다.
fn extension_id_from_path(path: &str) -> Option<String> {
    // nest 내부 layer라 path는 /api/v1 prefix가 벗겨진 상태다 ("/dummy", "/lobby/manifest").
    let seg = path.trim_start_matches('/').split('/').next()?;
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// 런타임 게이트 미들웨어. registry에 있는 확장이 비활성/purged면 404.
async fn extension_gate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(ext_id) = extension_id_from_path(request.uri().path())
        && state.registry.find(&ext_id).is_some()
        && !state.registry.is_active(&ext_id).await
    {
        return ApiError::new(
            StatusCode::NOT_FOUND,
            "extension_disabled",
            "extension is disabled or purged",
        )
        .into_response();
    }
    next.run(request).await
}

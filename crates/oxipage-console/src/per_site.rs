//! Per-site console endpoints mounted at `/s/{slug}/...`.
//!
//! These expose site-scoped views of the global core handlers (`/extensions`,
//! `/theme`, `/build`, `/deploy`) so the SPA can address everything via
//! the slug. The handlers here thin-wrap core state (`SiteContext.db`) that
//! the per-site middleware injects.

use crate::build::site_build;
use crate::deploy::site_deploy;
use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Router, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

// ─── config (GET/PUT) ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ConfigResponse {
    pub data: serde_json::Value,
}

pub async fn config_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let cfg = &ctx.config;
    Ok(Json(ConfigResponse {
        data: serde_json::json!({
            "site": {
                "name": cfg.site.name,
                "base_url": cfg.site.base_url,
                "default_lang": cfg.site.default_lang,
                "languages": cfg.site.languages,
            },
            "server": {
                "host": cfg.server.host,
                "port": cfg.server.port,
                "data_dir": cfg.server.data_dir.to_string_lossy(),
            },
            "lobby": {
                "default_mode": cfg.lobby.default_mode,
            },
            "extensions": {
                "enabled": cfg.extensions.enabled,
            },
            "integrations": {
                "github_username": cfg.integrations.github_username,
                "tmdb_api_key_env": cfg.integrations.tmdb_api_key_env,
                "aladin_ttbkey_env": cfg.integrations.aladin_ttbkey_env,
            },
        }),
    }))
}

#[derive(Deserialize, Default)]
pub struct ConfigUpdate {
    pub site: Option<SiteUpdate>,
    pub lobby: Option<LobbyUpdate>,
}

#[derive(Deserialize, Default)]
pub struct SiteUpdate {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub default_lang: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct LobbyUpdate {
    pub default_mode: Option<String>,
}

pub async fn config_put(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let toml_path = ctx.path.join("oxipage.toml");
    let raw = tokio::fs::read_to_string(&toml_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read toml: {e}")))?;
    let mut doc: toml::Value = toml::from_str(&raw)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse toml: {e}")))?;

    if let Some(site) = update.site {
        let root = doc.as_table_mut().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "root toml is not a table".to_string(),
            )
        })?;
        let site_tbl = root
            .entry("site")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "site section not a table".to_string(),
                )
            })?;
        if let Some(name) = site.name {
            site_tbl.insert("name".into(), toml::Value::String(name));
        }
        if let Some(base_url) = site.base_url {
            site_tbl.insert("base_url".into(), toml::Value::String(base_url));
        }
        if let Some(lang) = site.default_lang {
            site_tbl.insert("default_lang".into(), toml::Value::String(lang));
        }
    }
    if let Some(lobby) = update.lobby {
        let root = doc.as_table_mut().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "root toml is not a table".to_string(),
            )
        })?;
        let lobby_tbl = root
            .entry("lobby")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "lobby section not a table".to_string(),
                )
            })?;
        if let Some(mode) = lobby.default_mode {
            lobby_tbl.insert("default_mode".into(), toml::Value::String(mode));
        }
    }

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize toml: {e}")))?;
    tokio::fs::write(&toml_path, serialized)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write toml: {e}")))?;

    // Reload config from disk so the response reflects what was actually saved.
    let new_cfg = oxipage_core::config::Config::load(&toml_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("reload: {e}")))?;
    Ok(Json(ConfigResponse {
        data: serde_json::json!({
            "site": {
                "name": new_cfg.site.name,
                "base_url": new_cfg.site.base_url,
                "default_lang": new_cfg.site.default_lang,
                "languages": new_cfg.site.languages,
            },
            "lobby": {
                "default_mode": new_cfg.lobby.default_mode,
            },
        }),
    }))
}

// ─── builds (GET history) ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct BuildRecord {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub page_count: Option<usize>,
    pub out_dir: Option<String>,
}

#[derive(Serialize)]
pub struct BuildsResponse {
    pub data: Vec<BuildRecord>,
}

/// Read build history from the per-site DB (`build_log` table). Returns an
/// empty list if no table exists yet (no builds have run).
pub async fn builds_list(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<BuildsResponse>, (StatusCode, String)> {
    let rows: Result<Vec<(i64, String, String, Option<i64>, Option<String>)>, _> = sqlx::query_as(
        "SELECT id, status, created_at, page_count, out_dir FROM build_log ORDER BY id DESC LIMIT 50",
    )
    .fetch_all(&ctx.db)
    .await;

    let records = match rows {
        Ok(rs) => rs
            .into_iter()
            .map(|(id, status, created_at, page_count, out_dir)| BuildRecord {
                id: id.to_string(),
                status,
                created_at,
                page_count: page_count.map(|p| p as usize),
                out_dir,
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    Ok(Json(BuildsResponse { data: records }))
}

// ─── theme (GET/PUT) ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ThemeResponse {
    pub data: serde_json::Value,
}

pub async fn theme_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ThemeResponse>, (StatusCode, String)> {
    let row: Option<(String,)> = sqlx::query_as("SELECT theme_id FROM theme_config WHERE id = 1")
        .fetch_optional(&ctx.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let theme_id = row.map(|r| r.0).unwrap_or_else(|| "paper".to_string());
    Ok(Json(ThemeResponse {
        data: serde_json::json!({ "theme_id": theme_id }),
    }))
}

#[derive(Deserialize)]
pub struct ThemePutInput {
    pub theme_id: String,
}

pub async fn theme_put(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Json(input): Json<ThemePutInput>,
) -> Result<Json<ThemeResponse>, (StatusCode, String)> {
    const VALID_THEMES: &[&str] = &["paper", "midnight", "sepia", "neon", "canvas"];
    if !VALID_THEMES.contains(&input.theme_id.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("'{}' is not a valid theme", input.theme_id),
        ));
    }
    sqlx::query(
        "INSERT INTO theme_config (id, theme_id, updated_at) VALUES (1, ?1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET theme_id = ?1, updated_at = datetime('now')",
    )
    .bind(&input.theme_id)
    .execute(&ctx.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(ThemeResponse {
        data: serde_json::json!({ "theme_id": input.theme_id }),
    }))
}

// ─── extensions (GET list) ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub purged: bool,
}

#[derive(Serialize)]
pub struct ExtensionsResponse {
    pub data: Vec<ExtensionInfo>,
}

pub async fn extensions_list(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ExtensionsResponse>, (StatusCode, String)> {
    let snapshot = ctx.registry.status_snapshot().await;
    let mut out = Vec::new();
    for ext in ctx.registry.iter() {
        let s = snapshot.get(ext.id()).copied();
        out.push(ExtensionInfo {
            id: ext.id().to_string(),
            display_name: ext.display_name(oxipage_core::extension::Lang::En),
            enabled: s.map(|s| s.enabled).unwrap_or(false),
            purged: s.map(|s| s.purged).unwrap_or(false),
        });
    }
    Ok(Json(ExtensionsResponse { data: out }))
}

// ─── extensions/{id}/enable, /disable (POST) ────────────────────────────────

async fn set_extension_enabled(
    ctx: &SiteContext,
    id: &str,
    enabled: bool,
) -> Result<Json<ExtensionInfo>, (StatusCode, String)> {
    let ext = ctx.registry.find(id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("unknown extension id: {id}"),
        )
    })?;
    let prev = ctx.registry.status_of(id).await;
    let was_purged = prev.map(|s| s.purged).unwrap_or(false);
    let was_enabled = prev.map(|s| s.enabled).unwrap_or(false);
    if was_purged && enabled {
        ctx.registry.set_purged(&ctx.db, id, false).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("set_purged: {e}"),
            )
        })?;
    }
    ctx.registry.set_enabled(&ctx.db, id, enabled).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("set_enabled: {e}"),
        )
    })?;
    if enabled && (!was_enabled || was_purged) {
        // Build a minimal AppState for the lifecycle hook. The console
        // process owns the real one for the default site; per-site the
        // hook only needs db + config + registry.
        let state = oxipage_core::state::AppState {
            db: ctx.db.clone(),
            config: ctx.config.clone(),
            registry: ctx.registry.clone(),
            wasm_loader: ctx.wasm_loader.clone(),
            site_override: Arc::new(RwLock::new(None)),
            builders: ctx.builders.clone(),
        };
        if let Err(e) = ext.on_startup(&state).await {
            tracing::warn!(extension = %id, error = %e, "on_startup failed");
        }
    }
    Ok(Json(ExtensionInfo {
        id: ext.id().to_string(),
        display_name: ext.display_name(oxipage_core::extension::Lang::En),
        enabled,
        purged: false,
    }))
}

pub async fn extension_enable(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>, (StatusCode, String)> {
    set_extension_enabled(&ctx, &id, true).await
}

pub async fn extension_disable(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(id): Path<String>,
) -> Result<Json<ExtensionInfo>, (StatusCode, String)> {
    set_extension_enabled(&ctx, &id, false).await
}

// ─── build / deploy (POST) ──────────────────────────────────────────────────


pub async fn build_post(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<site_build::BuildResult>, (StatusCode, String)> {
    let out_dir = ctx.path.join("out");
    tokio::fs::create_dir_all(&out_dir)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let output = oxipage_core::build::build_site(&ctx.db, &ctx.builders)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let media_dir = ctx.config.server.data_dir.join("media");
    oxipage_core::build_writer::write_build_output(&output, &out_dir, &media_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Record this build in the build_log table (idempotent schema setup).
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS build_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            status TEXT NOT NULL DEFAULT 'built',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            page_count INTEGER,
            out_dir TEXT
        )",
    )
    .execute(&ctx.db)
    .await;
    let _ = sqlx::query(
        "INSERT INTO build_log (status, page_count, out_dir) VALUES ('built', ?1, ?2)",
    )
    .bind(output.pages.len() as i64)
    .bind(out_dir.to_string_lossy().to_string())
    .execute(&ctx.db)
    .await;

    Ok(Json(site_build::BuildResult {
        data: site_build::BuildOutput {
            out_dir: out_dir.to_string_lossy().into_owned(),
            page_count: output.pages.len(),
        },
    }))
}

pub async fn deploy_post(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<site_deploy::DeployResponse>, (StatusCode, String)> {
    let _ = ctx;
    Ok(Json(site_deploy::DeployResponse {
        data: site_deploy::DeployOutput {
            slug: ctx.slug.clone(),
            status: "queued",
            note: "Deploy is currently invoked via `oxipage deploy --site <slug>`; console route is a stub pending module integration.",
        },
    }))
}

// ─── per-site router ────────────────────────────────────────────────────────

pub fn per_site_router() -> Router {
    Router::new()
        .route("/config", get(config_get).put(config_put))
        .route("/builds", get(builds_list))
        .route("/build", post(build_post))
        .route("/deploy", post(deploy_post))
        .route("/theme", get(theme_get).put(theme_put))
        .route("/extensions", get(extensions_list))
        .route("/extensions/{id}/enable", post(extension_enable))
        .route("/extensions/{id}/disable", post(extension_disable))
}

//! Per-site console endpoints mounted at `/s/{slug}/...`.
//!
//! These expose site-scoped views of the global core handlers (`/extensions`,
//! `/theme`, `/build`, `/deploy`) so the SPA can address everything via
//! the slug. The handlers here thin-wrap core state (`SiteContext.db`) that
//! the per-site middleware injects.

use crate::build::build_run::BuildRun;
use crate::deploy::deploy_run::DeployRun;
use crate::sites_runtime::SiteContext;
use axum::Extension;
use axum::Json;
use axum::extract::{Path, Query};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Router, http::StatusCode};
use oxipage_core::build::BuildEvent;
use oxipage_core::config::ServerConfig;
use oxipage_core::site_paths::{GitHubPagesTarget, MutableSiteSettings};
use oxipage_deploy::DeployEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

// ─── config (GET/PUT) ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ConfigResponse {
    pub data: serde_json::Value,
}

pub async fn config_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let s = ctx.settings.read().await;
    Ok(Json(ConfigResponse {
        data: config_json(&s, &ctx.startup_server),
    }))
}

/// Serialize the current mutable settings + startup-immutable server section
/// into the JSON shape consumed by the admin UI. `deploy.github_pages` is
/// inline so the UI can render the same DTO it received on PUT without a
/// second fetch.
fn config_json(settings: &MutableSiteSettings, server: &ServerConfig) -> serde_json::Value {
    let target = settings.deploy.github_pages.as_ref();
    serde_json::json!({
        "site": {
            "name": settings.site.name,
            "base_url": settings.site.base_url,
            "default_lang": settings.site.default_lang,
            "languages": settings.site.languages,
        },
        "server": {
            "host": server.host,
            "port": server.port,
            "data_dir": server.data_dir.to_string_lossy(),
        },
        "lobby": {
            "default_mode": settings.lobby.default_mode,
        },
        "extensions": {
            "enabled": settings.extensions.enabled,
        },
        "integrations": {
            "github_username": settings.integrations.github_username,
            "tmdb_api_key_env": settings.integrations.tmdb_api_key_env,
            "aladin_ttbkey_env": settings.integrations.aladin_ttbkey_env,
        },
        "deploy": {
            "github_pages": target.map(|t| serde_json::json!({
                "owner": t.owner,
                "repo": t.repo,
                "branch": t.branch,
                "pages_url": t.pages_url(),
                "base_path": t.base_path(),
            })),
        },
    })
}


#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigUpdate {
    pub site: Option<SiteUpdate>,
    pub lobby: Option<LobbyUpdate>,
    pub integrations: Option<IntegrationsUpdate>,
    pub deploy: Option<DeployUpdate>,
}

#[derive(Deserialize, Default)]
pub struct SiteUpdate {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub default_lang: Option<String>,
    pub languages: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
pub struct LobbyUpdate {
    pub default_mode: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct IntegrationsUpdate {
    pub github_username: Option<String>,
    pub tmdb_api_key_env: Option<String>,
    pub aladin_ttbkey_env: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DeployUpdate {
    pub github_pages: Option<GitHubPagesTarget>,
}

/// Apply the allowlisted `[deploy]` patch to the in-memory TOML doc.
/// Validates the target first; an invalid target rejects the whole PUT
/// (nothing is persisted). Passing `None` for `github_pages` removes the
/// existing target (explicit unset).
fn apply_deploy_patch(
    doc: &mut toml::Value,
    patch: Option<DeployUpdate>,
) -> Result<(), (StatusCode, String)> {
    let Some(patch) = patch else {
        return Ok(());
    };
    if let Some(target) = &patch.github_pages {
        target
            .validate()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    }
    let root = doc
        .as_table_mut()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "root TOML is not a table".into()))?;
    let deploy = root
        .entry("deploy")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or((StatusCode::BAD_REQUEST, "deploy must be a table".into()))?;
    match patch.github_pages {
        Some(t) => {
            deploy.insert(
                "github_pages".into(),
                toml::Value::try_from(t).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
            );
        }
        None => {
            deploy.remove("github_pages");
        }
    };
    Ok(())
}

pub async fn config_put(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let toml_path = ctx.project_dir.join("oxipage.toml");

    // Serialize concurrent config writes for this site. Held across the full
    // read-modify-write so two PUTs cannot clobber each other.
    let _guard = ctx.config_write_lock.lock().await;

    // Reread current TOML (preserves [server] and any unknown sections).
    let raw = tokio::fs::read_to_string(&toml_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read toml: {e}")))?;
    let mut doc: toml::Value = toml::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse toml: {e}"),
        )
    })?;

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
        if let Some(langs) = site.languages {
            let arr: Vec<toml::Value> = langs.into_iter().map(toml::Value::String).collect();
            site_tbl.insert("languages".into(), toml::Value::Array(arr));
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

    if let Some(integrations) = update.integrations {
        let root = doc.as_table_mut().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "root toml is not a table".to_string(),
            )
        })?;
        let int_tbl = root
            .entry("integrations")
            .or_insert(toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "integrations section not a table".to_string(),
                )
            })?;
        if let Some(v) = integrations.github_username {
            int_tbl.insert("github_username".into(), toml::Value::String(v));
        }
        if let Some(v) = integrations.tmdb_api_key_env {
            int_tbl.insert("tmdb_api_key_env".into(), toml::Value::String(v));
        }
        if let Some(v) = integrations.aladin_ttbkey_env {
            int_tbl.insert("aladin_ttbkey_env".into(), toml::Value::String(v));
        }
    }

    // Deploy target patch (validated inside; rejects the whole PUT on error).
    apply_deploy_patch(&mut doc, update.deploy)?;

    let serialized = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize toml: {e}"),
        )
    })?;

    // Validate the fully-patched config before persisting — an invalid target
    // must never reach disk.
    let parsed = oxipage_core::config::Config::from_toml_str(&serialized)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if let Some(target) = &parsed.deploy.github_pages {
        target
            .validate()
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    }

    // Atomic write: temp file in the same directory, then rename.
    let tmp = toml_path.with_extension("toml.tmp");
    tokio::fs::write(&tmp, &serialized)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write tmp: {e}")))?;
    tokio::fs::rename(&tmp, &toml_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("rename: {e}")))?;

    // Reload config from disk and replace the in-memory settings snapshot so
    // subsequent reads reflect what was actually persisted.
    let new_cfg = oxipage_core::config::Config::load(&toml_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("reload: {e}")))?;
    *ctx.settings.write().await =
        oxipage_core::site_paths::MutableSiteSettings::from_config(&new_cfg);

    let s = ctx.settings.read().await;
    Ok(Json(ConfigResponse {
        data: config_json(&s, &ctx.startup_server),
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
            .map(
                |(id, status, created_at, page_count, out_dir)| BuildRecord {
                    id: id.to_string(),
                    status,
                    created_at,
                    page_count: page_count.map(|p| p as usize),
                    out_dir,
                },
            )
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
    use oxipage_core::theme::{find_theme, ALL_THEMES};

    let row: Option<(String,)> = sqlx::query_as("SELECT theme_id FROM theme_config WHERE id = 1")
        .fetch_optional(&ctx.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let stored = row.map(|r| r.0);
    let def = match stored.as_deref().and_then(find_theme) {
        Some(d) => d,
        None => ALL_THEMES.first().expect("paper always present"),
    };

    Ok(Json(ThemeResponse {
        data: serde_json::json!({
            "theme_id": def.id,
            "definition": def,
        }),
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
    use oxipage_core::theme::{find_theme, is_known_theme};

    if !is_known_theme(&input.theme_id) {
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

    let def = find_theme(&input.theme_id)
        .expect("validation above guarantees presence");
    Ok(Json(ThemeResponse {
        data: serde_json::json!({
            "theme_id": def.id,
            "definition": def,
        }),
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
    let ext = ctx
        .registry
        .find(id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("unknown extension id: {id}")))?;
    let prev = ctx.registry.status_of(id).await;
    let was_purged = prev.map(|s| s.purged).unwrap_or(false);
    let was_enabled = prev.map(|s| s.enabled).unwrap_or(false);
    if was_purged && enabled {
        ctx.registry
            .set_purged(&ctx.db, id, false)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("set_purged: {e}"),
                )
            })?;
    }
    ctx.registry
        .set_enabled(&ctx.db, id, enabled)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("set_enabled: {e}"),
            )
        })?;
    if enabled && (!was_enabled || was_purged) {
        // Build a minimal AppState for the lifecycle hook. The console
        // process owns the real one for the default site; per-site the
        // hook only needs db + a config reconstructed from the live
        // settings snapshot + startup-immutable server section.
        let cfg = ctx.settings.read().await.to_config(&ctx.startup_server);
        let state = oxipage_core::state::AppState {
            db: ctx.db.clone(),
            config: Arc::new(cfg),
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
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let out_dir = ctx.out_dir.clone();
    tokio::fs::create_dir_all(&out_dir).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
    })?;

    let media_dir = ctx.media_dir.clone();
    let started_at = sqlx::query_scalar::<_, String>("SELECT datetime('now')")
        .fetch_one(&ctx.db)
        .await
        .unwrap_or_else(|_| "unknown".into());
    let build_id = uuid::Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel::<BuildEvent>(64);

    // Snapshot the mutable inputs the build writer needs (base_url drives
    // deployment_base; theme_id is recorded in the manifest). Read once so the
    // lazy-started build task sees a consistent value.
    let site_base_url = ctx.settings.read().await.site.base_url.clone();
    let theme_id = oxipage_core::theme::active_theme_id(&ctx.db).await;

    let run = BuildRun {
        id: build_id.clone(),
        tx,
        started: AtomicBool::new(false),
        started_at,
        db: ctx.db.clone(),
        builders: ctx.builders.clone(),
        out_dir,
        media_dir,
        slug: ctx.slug.clone(),
        site_base_url,
        theme_id,
    };
    if let Err(existing_id) = ctx.build_guard.try_start(&ctx.slug, run) {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "build_in_progress",
                "build_id": existing_id
            })),
        ));
    }

    // Watchdog: if no SSE subscriber connects within 3s, start the build
    // anyway so a dropped request never leaves the slug "in progress".
    let guard = ctx.build_guard.clone();
    let slug = ctx.slug.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        guard.ensure_build_started(&slug);
    });

    // Informational summary of the most recent completed build (if any). The
    // freshly-queued build hasn't produced output yet, so this reflects the
    // previous build's manifest when present.
    let manifest_preview = oxipage_core::build_manifest::BuildManifest::read_from(&ctx.out_dir)
        .ok()
        .flatten()
        .map(|m| {
            serde_json::json!({
                "build_id": m.build_id,
                "theme_id": m.theme_id,
                "deployment_base": m.deployment_base,
                "asset_revision": m.asset_revision,
                "ready": true,
            })
        });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "data": {
                "build_id": build_id,
                "status": "queued",
                "manifest_preview": manifest_preview,
            }
        })),
    ))
}

/// `GET /s/{slug}/build/{build_id}/stream` — Server-Sent Events stream of
/// [`BuildEvent`]s for the in-flight build. The first subscriber triggers the
/// build (lazy-start); a 3s watchdog (from `build_post`) is the fallback.
pub async fn build_stream(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(_build_id): Path<String>,
) -> Result<
    Sse<impl tokio_stream::Stream<Item = Result<Event, std::io::Error>>>,
    (StatusCode, String),
> {
    // Subscribe BEFORE starting so the first subscriber sees BuildStarted with
    // zero event loss (broadcast delivers only post-subscribe messages).
    let (_id, rx) = ctx
        .build_guard
        .subscribe(&ctx.slug)
        .ok_or((StatusCode::NOT_FOUND, "no_active_build".to_string()))?;
    ctx.build_guard.ensure_build_started(&ctx.slug);

    let stream = BroadcastStream::new(rx).map(|res| {
        Ok::<_, std::io::Error>(match res {
            Ok(ev) => Event::default().data(serde_json::to_string(&ev).unwrap_or_default()),
            // Lagged receiver (slow client) — emit an SSE comment, not data.
            Err(_) => Event::default().comment("lagged"),
        })
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub async fn deploy_post(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let out_dir = ctx.out_dir.clone();
    if !out_dir.exists() {
        // Deploy needs a prior build's output directory.
        return Err((
            StatusCode::FAILED_DEPENDENCY,
            Json(serde_json::json!({ "error": "no_build_output" })),
        ));
    }
    // Repository-scoped deploy: target from live settings, manifest from the
    // latest build. Both are snapshotted into the run so the lazy-started task
    // sees a consistent view.
    let target = ctx
        .settings
        .read()
        .await
        .deploy
        .github_pages
        .clone()
        .ok_or((
            StatusCode::PRECONDITION_FAILED,
            Json(serde_json::json!({ "error": "deploy_not_configured" })),
        ))?;
    let manifest = oxipage_core::build_manifest::BuildManifest::read_from(&ctx.out_dir)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?
        .ok_or((
            StatusCode::FAILED_DEPENDENCY,
            Json(serde_json::json!({ "error": "build_required" })),
        ))?;
    let deploy_id = uuid::Uuid::new_v4().to_string();
    let (tx, _rx) = broadcast::channel::<DeployEvent>(32);

    let run = DeployRun {
        id: deploy_id.clone(),
        tx,
        started: AtomicBool::new(false),
        repo_dir: ctx.project_dir.clone(),
        out_dir,
        target,
        manifest,
        slug: ctx.slug.clone(),
    };
    if let Err(existing_id) = ctx.deploy_guard.try_start(&ctx.slug, run) {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "deploy_in_progress",
                "deploy_id": existing_id
            })),
        ));
    }

    // Watchdog: if no SSE subscriber connects within 3s, start the deploy
    // anyway so a dropped request never leaves the slug "in progress".
    let guard = ctx.deploy_guard.clone();
    let slug = ctx.slug.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        guard.ensure_deploy_started(&slug);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "data": { "deploy_id": deploy_id, "status": "queued" } })),
    ))
}

/// `GET /s/{slug}/deploy/{deploy_id}/stream` — Server-Sent Events stream of
/// [`DeployEvent`]s for the in-flight deploy. Mirrors [`build_stream`].
pub async fn deploy_stream(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Path(_deploy_id): Path<String>,
) -> Result<
    Sse<impl tokio_stream::Stream<Item = Result<Event, std::io::Error>>>,
    (StatusCode, String),
> {
    let (_id, rx) = ctx
        .deploy_guard
        .subscribe(&ctx.slug)
        .ok_or((StatusCode::NOT_FOUND, "no_active_deploy".to_string()))?;
    ctx.deploy_guard.ensure_deploy_started(&ctx.slug);

    let stream = BroadcastStream::new(rx).map(|res| {
        Ok::<_, std::io::Error>(match res {
            Ok(ev) => Event::default().data(serde_json::to_string(&ev).unwrap_or_default()),
            Err(_) => Event::default().comment("lagged"),
        })
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

// ─── stats / recent ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RecentQuery {
    pub limit: Option<i64>,
}

/// Per-extension recent-content queries (Approach C from spec — explicit per-ext SQL).
async fn recent_for_extension(
    db: &sqlx::SqlitePool,
    ext_id: &str,
    limit: i64,
) -> Vec<serde_json::Value> {
    let sql = match ext_id {
        "blog" => Some(
            "SELECT id, title, updated_at, published_at FROM blog_post ORDER BY updated_at DESC LIMIT ?",
        ),
        "books" => Some(
            "SELECT id, title, updated_at, published_at FROM book_entry ORDER BY updated_at DESC LIMIT ?",
        ),
        "links" => Some(
            "SELECT id, title, updated_at, NULL AS published_at FROM link_card ORDER BY updated_at DESC LIMIT ?",
        ),
        "movies" => Some(
            "SELECT id, title, updated_at, published_at FROM movie_entry ORDER BY updated_at DESC LIMIT ?",
        ),
        "novels" => Some(
            "SELECT id, title, updated_at, published_at FROM novel ORDER BY updated_at DESC LIMIT ?",
        ),
        "projects" => Some(
            "SELECT id, COALESCE(title_ko, title_en) AS title, updated_at, published_at FROM project ORDER BY updated_at DESC LIMIT ?",
        ),
        "scraps" => Some(
            "SELECT id, title, updated_at, published_at FROM scrap_item ORDER BY updated_at DESC LIMIT ?",
        ),
        _ => None,
    };
    let Some(sql) = sql else { return vec![] };

    match sqlx::query_as::<_, (i64, String, String, Option<String>)>(sql)
        .bind(limit)
        .fetch_all(db)
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|(id, title, updated_at, published_at)| {
                serde_json::json!({
                    "ext": ext_id,
                    "id": id,
                    "title": title,
                    "updated_at": updated_at,
                    "published_at": published_at,
                })
            })
            .collect(),
        Err(e) => {
            tracing::warn!("recent query for {ext_id} failed: {e}");
            vec![]
        }
    }
}

pub async fn stats_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let snapshot = ctx.registry.status_snapshot().await;
    let mut counts = serde_json::Map::new();

    for ext in ctx.registry.iter() {
        let status = snapshot.get(ext.id()).copied();
        if !status.map(|s| s.enabled).unwrap_or(false) {
            continue;
        }
        let ext_id = ext.id();
        let table = match ext_id {
            "blog" => Some("blog_post"),
            "books" => Some("book_entry"),
            "links" => Some("link_card"),
            "movies" => Some("movie_entry"),
            "novels" => Some("novel"),
            "projects" => Some("project"),
            "scraps" => Some("scrap_item"),
            _ => None,
        };
        if let Some(tbl) = table {
            match sqlx::query_as::<_, (i64,)>(&format!("SELECT COUNT(*) FROM {tbl}"))
                .fetch_one(&ctx.db)
                .await
            {
                Ok((n,)) => {
                    counts.insert(ext_id.to_string(), serde_json::json!(n));
                }
                Err(e) => {
                    tracing::warn!("stats count for {tbl}: {e}");
                }
            }
        }
    }

    // Storage: recursive walk of site directory, excluding out/
    let storage_bytes = tokio::task::spawn_blocking({
        let path = ctx.project_dir.clone();
        move || -> u64 {
            fn dir_size(path: &std::path::Path) -> u64 {
                let mut total = 0u64;
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            total += p.metadata().map(|m| m.len()).unwrap_or(0);
                        } else if p.is_dir() {
                            total += dir_size(&p);
                        }
                    }
                }
                total
            }
            let out = path.join("out");
            let mut total = 0u64;
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p == out {
                        continue;
                    }
                    if p.is_file() {
                        total += p.metadata().map(|m| m.len()).unwrap_or(0);
                    } else if p.is_dir() {
                        total += dir_size(&p);
                    }
                }
            }
            total
        }
    })
    .await
    .unwrap_or(0);

    // Last build: most recent build_log entry
    let last_build = sqlx::query_as::<_, (String, String)>(
        "SELECT status, created_at FROM build_log ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(&ctx.db)
    .await
    .unwrap_or(None)
    .map(|(status, started_at)| {
        serde_json::json!({
            "status": status,
            "started_at": started_at,
        })
    });

    Ok(Json(ConfigResponse {
        data: serde_json::json!({
            "counts": counts,
            "storage_bytes": storage_bytes,
            "last_build": last_build,
        }),
    }))
}

pub async fn recent_get(
    Extension(ctx): Extension<Arc<SiteContext>>,
    Query(params): Query<RecentQuery>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(5).clamp(1, 50);
    let snapshot = ctx.registry.status_snapshot().await;
    let mut all = Vec::new();

    for ext in ctx.registry.iter() {
        let status = snapshot.get(ext.id()).copied();
        if !status.map(|s| s.enabled).unwrap_or(false) {
            continue;
        }
        let items = recent_for_extension(&ctx.db, ext.id(), limit).await;
        all.extend(items);
    }

    all.sort_by(|a, b| {
        let au = a["updated_at"].as_str().unwrap_or("");
        let bu = b["updated_at"].as_str().unwrap_or("");
        bu.cmp(au)
    });
    all.truncate(limit as usize);

    Ok(Json(ConfigResponse {
        data: serde_json::json!(all),
    }))
}

pub fn per_site_router() -> Router {
    Router::new()
        .route("/config", get(config_get).put(config_put))
        .route("/builds", get(builds_list))
        .route("/build", post(build_post))
        .route("/build/{build_id}/stream", get(build_stream))
        .route("/deploy", post(deploy_post))
        .route("/deploy/{deploy_id}/stream", get(deploy_stream))
        .route("/stats", get(stats_get))
        .route("/content/recent", get(recent_get))
        .route("/theme", get(theme_get).put(theme_put))
        .route("/extensions", get(extensions_list))
        .route("/extensions/{id}/enable", post(extension_enable))
        .route("/extensions/{id}/disable", post(extension_disable))
        // Media upload + live serve (spec §7–9).
        .merge(crate::media::router())
}

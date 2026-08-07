//! Console router — top-level routes (site CRUD + preview) plus per-site
//! extension routes. Per-site routes use middleware-injected SiteScopedDb.
//! Build/deploy live exclusively under `/s/{slug}/...` (per_site_router).

use crate::middleware::site_db::inject_site_context;
use crate::preview::handler::{preview_handler, preview_trailing, redirect_to_slash};
use crate::per_site::{atomic_write_and_reload, read_toml_doc};
use crate::sites_runtime::{SiteContext, SiteRegistry};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use directories::ProjectDirs;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct CreateSiteInput {
    pub path: String,
}

#[derive(Deserialize)]
pub struct SetDefaultInput {
    pub default_site: String,
}

static REGISTRY: OnceLock<Arc<SiteRegistry>> = OnceLock::new();

/// Build the top-level console routes. Returns `Router<Arc<SiteRegistry>>`
/// without baking state — caller passes the registry once.
///
/// Build/deploy are intentionally NOT mounted here — they live per-site at
/// `/s/{slug}/build` and `/s/{slug}/deploy` (see [`build_per_site_router`]).
pub fn build_top_level_router() -> Router<Arc<SiteRegistry>> {
    Router::new()
        .route("/sites", get(list_sites))
        .route("/sites/default", get(get_default).put(set_default))
        .route("/sites/{slug}", delete(delete_site_handler))
        .route("/preview/{slug}", get(redirect_to_slash))
        .route("/preview/{slug}/", get(preview_trailing))
        .route("/preview/{slug}/{*rest}", get(preview_handler))
        .route("/setup/create-site", post(create_site_handler))
        .route("/theme", get(get_default_theme))
        .route("/mounts", get(mounts_list).post(mounts_add))
        .route("/mounts/{id}", delete(mounts_rm))
}

/// Per-site extension nests. Returns `Router<()>`. Handlers use
/// `Extension<SiteScopedDb>` injected by middleware.
pub fn build_per_site_router(registry: &Arc<SiteRegistry>) -> Router {
    let mut api = Router::new();
    for (_slug, ctx) in registry.iter_blocking() {
        let mut nested = Router::new();
        for ext in ctx.registry.iter() {
            if ext.route_dispatcher().is_some() {
                continue;
            }
            nested = nested.nest(&format!("/{}", ext.id()), ext.routes());
        }
        // Per-site console endpoints (config, theme, builds, extensions, build, deploy).
        nested = nested.merge(crate::per_site::per_site_router());
        let scoped = nested.layer(axum::middleware::from_fn_with_state(
            ctx.clone(),
            inject_site_context,
        ));
        api = api.nest(&format!("/s/{}", ctx.slug), scoped);
    }
    api
}

/// Full console router. Returns `Router<()>` after baking state.
pub fn build_console_router(registry: Arc<SiteRegistry>) -> Router {
    let _ = REGISTRY.set(registry.clone());
    let per_site = build_per_site_router(&registry);
    let top = build_top_level_router().with_state(registry);
    top.merge(per_site)
}

// ─── Site CRUD ───

async fn list_sites(State(registry): State<Arc<SiteRegistry>>) -> Json<serde_json::Value> {
    let sites: Vec<_> = registry
        .all_sites_from_file()
        .await
        .into_iter()
        .map(|(name, path, active)| {
            serde_json::json!({
                "name": name,
                "path": path.to_string_lossy(),
                "active": active,
            })
        })
        .collect();
    Json(serde_json::json!({ "data": sites }))
}

async fn get_default(State(registry): State<Arc<SiteRegistry>>) -> Json<serde_json::Value> {
    let slug = registry.default_slug().await;
    Json(serde_json::json!({ "data": { "default_site": slug } }))
}

async fn set_default(
    State(registry): State<Arc<SiteRegistry>>,
    Json(input): Json<SetDefaultInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    registry
        .set_default(&input.default_site)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "data": { "default_site": input.default_site } }),
    ))
}

async fn create_site_handler(
    Json(input): Json<CreateSiteInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = PathBuf::from(input.path);

    // Validate path
    if path.exists() {
        if !path.join("oxibuilder.toml").exists() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "path '{}' exists but is not an oxibuilder project (no oxibuilder.toml)",
                    path.display()
                ),
            ));
        }
    } else {
        std::fs::create_dir_all(&path).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("cannot create directory: {e}"),
            )
        })?;
    }

    // Seed oxibuilder.toml if not present
    if !path.join("oxibuilder.toml").exists() {
        let toml_content = r#"[site]
name = "My Site"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile", "blog"]
"#;
        std::fs::write(path.join("oxibuilder.toml"), toml_content).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("cannot write oxibuilder.toml: {e}"),
            )
        })?;
    }

    // Derive slug from directory name
    let slug = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("site")
        .to_string();

    // Register in sites.toml (disk + in-memory)
    if let Some(registry) = REGISTRY.get() {
        let sites_path = ProjectDirs::from("dev", "oxibuilder", "oxibuilder")
            .map(|p| p.config_dir().join("sites.toml"));
        if let Some(ref sp) = sites_path {
            let mut sf = if sp.exists() {
                std::fs::read_to_string(sp)
                    .ok()
                    .and_then(|raw| toml::from_str(&raw).ok())
                    .unwrap_or_default()
            } else {
                oxibuilder_core::sites::SitesFile::default()
            };
            sf.add(slug.clone(), path.clone());
            if sf.default_site.is_none() {
                sf.set_default(&slug);
            }
            if let Some(parent) = sp.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(raw) = toml::to_string_pretty(&sf) {
                let _ = std::fs::write(sp, &raw);
            }
        }
        // Update in-memory registry
        registry.register_in_file(&slug, path.clone()).await;
    }

    Ok(Json(serde_json::json!({
        "data": { "slug": slug, "path": path.to_string_lossy() }
    })))
}

async fn delete_site_handler(
    State(registry): State<Arc<SiteRegistry>>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    registry.remove_site(&slug).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("delete site: {e}"),
        )
    })?;
    Ok(Json(
        serde_json::json!({"data": {"slug": slug, "removed": true}}),
    ))
}

/// `GET /api/console/theme` — current default site's theme definition.
///
/// Resolves the registered default site through `SiteRegistry`. With no
/// registered site, returns `paper` without touching any DB. Never reads
/// the global `AppState.db` (that handler used to read a different DB than
/// the per-site endpoint).
async fn get_default_theme(
    State(registry): State<Arc<SiteRegistry>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use oxibuilder_core::theme::{ALL_THEMES, find_theme};

    let slug = match registry.default_slug().await {
        Some(s) => s,
        None => {
            let def = ALL_THEMES
                .first()
                .expect("paper always present in ALL_THEMES");
            return Ok(Json(serde_json::json!({
                "data": {
                    "theme_id": def.id,
                    "definition": def,
                    "layout": "shell",
                }
            })));
        }
    };

    let ctx = registry.ctx_for(&slug).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("default site '{slug}' not loaded"),
        )
    })?;

    let row: Option<(String, String)> =
        sqlx::query_as("SELECT theme_id, layout FROM theme_config WHERE id = 1")
            .fetch_optional(&ctx.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let (theme_id, layout) = match row {
        Some((id, layout)) => (id, layout),
        None => ("paper".to_string(), "shell".to_string()),
    };
    let def =
        find_theme(&theme_id).unwrap_or_else(|| ALL_THEMES.first().expect("paper always present"));

    Ok(Json(serde_json::json!({
        "data": {
            "theme_id": def.id,
            "definition": def,
            "layout": layout,
        }
    })))
}


// ─── static mounts (CLI-managed; toml is the source of truth) ───────────────

/// Input for `POST /api/console/mounts` — one `[[mounts]]` entry.
#[derive(Deserialize)]
struct MountInput {
    id: String,
    source: String,
    path: String,
    title_ko: String,
    title_en: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    open_in_new_tab: bool,
}

fn mount_table_to_json(tbl: &toml::value::Table) -> serde_json::Value {
    let s = |k: &str| {
        tbl.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let opt = |k: &str| tbl.get(k).and_then(|v| v.as_str()).map(str::to_string);
    serde_json::json!({
        "id": s("id"),
        "source": s("source"),
        "path": s("path"),
        "title_ko": s("title_ko"),
        "title_en": s("title_en"),
        "description": opt("description"),
        "icon": opt("icon"),
        "open_in_new_tab": tbl.get("open_in_new_tab").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

/// Extract the `[[mounts]]` array from a raw toml doc as JSON (raw sources,
/// exactly as written — not the build-resolved absolute paths).
fn mounts_from_doc(doc: &toml::Value) -> Vec<serde_json::Value> {
    doc.get("mounts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_table().map(mount_table_to_json))
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the default site's context, or 404 when no site is loaded.
async fn default_ctx(
    registry: &SiteRegistry,
) -> Result<std::sync::Arc<SiteContext>, (StatusCode, String)> {
    let slug = registry
        .default_slug()
        .await
        .ok_or((StatusCode::NOT_FOUND, "no site registered".to_string()))?;
    registry.ctx_for(&slug).await.ok_or((
        StatusCode::NOT_FOUND,
        format!("default site '{slug}' not loaded"),
    ))
}

/// `GET /api/console/mounts` — list configured mounts (raw sources) plus the
/// load-resolved (auto-detected) source path for each, keyed by id.
async fn mounts_list(
    State(registry): State<Arc<SiteRegistry>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let ctx = default_ctx(&registry).await?;
    let doc = read_toml_doc(&ctx).await?;
    let mut mounts = mounts_from_doc(&doc);

    // ctx.settings.mounts carry the load-resolved (absolute, auto-detected)
    // sources. Map them by id and surface as `resolved_source`.
    let resolved: std::collections::HashMap<String, String> = {
        let settings = ctx.settings.read().await;
        settings
            .mounts
            .iter()
            .map(|m| (m.id.clone(), m.source.to_string_lossy().into_owned()))
            .collect()
    };
    for m in mounts.iter_mut() {
        if let Some(id) = m.get("id").and_then(|v| v.as_str())
            && let Some(r) = resolved.get(id)
        {
            m["resolved_source"] = serde_json::Value::String(r.clone());
        }
    }

    Ok(Json(serde_json::json!({ "data": { "mounts": mounts } })))
}

/// `POST /api/console/mounts` — add a mount. Patches the raw toml doc, validates
/// (parse + `validate_mounts`: reserved prefixes, duplicate ids/paths, bad
/// segments), then atomically writes + reloads. The `config_write_lock` serializes
/// the read-modify-write against concurrent config PUTs.
async fn mounts_add(
    State(registry): State<Arc<SiteRegistry>>,
    Json(input): Json<MountInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let ctx = default_ctx(&registry).await?;
    let _guard = ctx.config_write_lock.lock().await;
    let mut doc = read_toml_doc(&ctx).await?;

    let mut tbl = toml::Table::new();
    tbl.insert("id".into(), toml::Value::String(input.id));
    tbl.insert("source".into(), toml::Value::String(input.source));
    tbl.insert("path".into(), toml::Value::String(input.path));
    tbl.insert("title_ko".into(), toml::Value::String(input.title_ko));
    tbl.insert("title_en".into(), toml::Value::String(input.title_en));
    if let Some(d) = input.description {
        tbl.insert("description".into(), toml::Value::String(d));
    }
    if let Some(i) = input.icon {
        tbl.insert("icon".into(), toml::Value::String(i));
    }
    if input.open_in_new_tab {
        tbl.insert("open_in_new_tab".into(), toml::Value::Boolean(true));
    }

    let root = doc
        .as_table_mut()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "root toml not a table".to_string()))?;
    let arr = root
        .entry("mounts")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    arr.as_array_mut()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "[[mounts]] not an array".to_string()))?
        .push(toml::Value::Table(tbl));

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize toml: {e}")))?;
    let parsed = oxibuilder_core::config::Config::from_toml_str(&serialized)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    parsed
        .validate_mounts()
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    atomic_write_and_reload(&ctx, &serialized).await?;

    Ok(Json(serde_json::json!({ "data": { "mounts": mounts_from_doc(&doc) } })))
}

/// `DELETE /api/console/mounts/{id}` — remove the mount whose `id` matches.
async fn mounts_rm(
    State(registry): State<Arc<SiteRegistry>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let ctx = default_ctx(&registry).await?;
    let _guard = ctx.config_write_lock.lock().await;
    let mut doc = read_toml_doc(&ctx).await?;

    let root = doc
        .as_table_mut()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "root toml not a table".to_string()))?;
    let arr = match root.get_mut("mounts").and_then(|v| v.as_array_mut()) {
        Some(a) => a,
        None => return Err((StatusCode::NOT_FOUND, format!("mount not found: {id}"))),
    };
    let before = arr.len();
    arr.retain(|e| {
        e.as_table()
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            != Some(id.as_str())
    });
    if arr.len() == before {
        return Err((StatusCode::NOT_FOUND, format!("mount not found: {id}")));
    }

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize toml: {e}")))?;
    atomic_write_and_reload(&ctx, &serialized).await?;

    Ok(Json(serde_json::json!({ "data": { "mounts": mounts_from_doc(&doc) } })))
}

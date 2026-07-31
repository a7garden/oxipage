//! Console router — top-level routes (site CRUD + preview) plus per-site
//! extension routes. Per-site routes use middleware-injected SiteScopedDb.
//! Build/deploy live exclusively under `/s/{slug}/...` (per_site_router).

use crate::middleware::site_db::inject_site_context;
use crate::preview::handler::{preview_handler, preview_trailing, redirect_to_slash};
use crate::sites_runtime::SiteRegistry;
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
    Ok(Json(serde_json::json!({ "data": { "default_site": input.default_site } })))
}

async fn create_site_handler(
    Json(input): Json<CreateSiteInput>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = PathBuf::from(input.path);

    // Validate path
    if path.exists() {
        if !path.join("oxipage.toml").exists() {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "path '{}' exists but is not an oxipage project (no oxipage.toml)",
                    path.display()
                ),
            ));
        }
    } else {
        std::fs::create_dir_all(&path)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("cannot create directory: {e}")))?;
    }

    // Seed oxipage.toml if not present
    if !path.join("oxipage.toml").exists() {
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
        std::fs::write(path.join("oxipage.toml"), toml_content)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("cannot write oxipage.toml: {e}")))?;
    }

    // Derive slug from directory name
    let slug = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("site")
        .to_string();

    // Register in sites.toml (disk + in-memory)
    if let Some(registry) = REGISTRY.get() {
        let sites_path = ProjectDirs::from("dev", "oxipage", "oxipage")
            .map(|p| p.config_dir().join("sites.toml"));
        if let Some(ref sp) = sites_path {
            let mut sf = if sp.exists() {
                std::fs::read_to_string(sp)
                    .ok()
                    .and_then(|raw| toml::from_str(&raw).ok())
                    .unwrap_or_default()
            } else {
                oxipage_core::sites::SitesFile::default()
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
        (StatusCode::INTERNAL_SERVER_ERROR, format!("delete site: {e}"))
    })?;
    Ok(Json(serde_json::json!({"data": {"slug": slug, "removed": true}})))

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
    use oxipage_core::theme::{find_theme, ALL_THEMES};

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
                }
            })));
        }
    };

    let ctx = registry
        .ctx_for(&slug)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("default site '{slug}' not loaded")))?;

    let row: Option<(String,)> = sqlx::query_as("SELECT theme_id FROM theme_config WHERE id = 1")
        .fetch_optional(&ctx.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let theme_id = row.map(|r| r.0).unwrap_or_else(|| "paper".to_string());
    let def = find_theme(&theme_id).unwrap_or_else(|| {
        ALL_THEMES.first().expect("paper always present")
    });

    Ok(Json(serde_json::json!({
        "data": {
            "theme_id": def.id,
            "definition": def,
        }
    })))
}

//! Lobby manifest assembly — single source of truth for the shape the public SPA fetches
//! via `fetchManifest()`.
//!
//! Used by two consumers that MUST produce the identical contract:
//! - the live HTTP handler `GET /api/console/lobby/manifest` (`crate::http`), and
//! - the SSG build, which writes `data/lobby.json` for static mode (`oxipage build`).
//!
//! Keeping one assembly function prevents the static site from drifting out of sync with the
//! live API (the exact bug that left the static lobby rendering no cards).

use crate::config::Config;
use crate::extension::{Extension, Lang};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Serialize)]
pub struct ManifestSite {
    pub name: String,
    pub base_url: String,
    pub default_lang: String,
    pub languages: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct ManifestLocalized {
    pub ko: String,
    pub en: String,
}

#[derive(Serialize, Clone, Default)]
pub struct LobbyConfigInfo {
    pub enabled: bool,
    pub display_mode: String,
    pub display_order: i64,
    pub style_params: serde_json::Value,
}

#[derive(Serialize)]
pub struct ManifestExtension {
    pub id: String,
    pub display_name: ManifestLocalized,
    pub lobby: LobbyConfigInfo,
}

#[derive(Serialize)]
pub struct Manifest {
    pub site: ManifestSite,
    pub extensions: Vec<ManifestExtension>,
}

/// Per-extension lobby display config from the `lobby_config` table, falling back to the
/// config default mode + a synthesized order when no row exists yet.
pub async fn lobby_config_for(
    db: &SqlitePool,
    config: &Config,
    ext_id: &str,
    default_order: i64,
) -> LobbyConfigInfo {
    let row: Option<(bool, String, i64, String)> = sqlx::query_as(
        "SELECT enabled, display_mode, display_order, style_params
         FROM lobby_config WHERE extension_id = ?",
    )
    .bind(ext_id)
    .fetch_optional(db)
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
            display_mode: config.lobby.default_mode.clone(),
            display_order: default_order,
            style_params: serde_json::json!({}),
        },
    }
}

/// Assemble the full manifest: resolved site metadata + every active extension with its lobby
/// config.
///
/// `site_name` / `base_url` are resolved by the caller — the live handler honors a runtime
/// site override, while the build uses the config values directly (no override at build time).
/// Extensions disabled or purged in `extension_state` are omitted, mirroring the route gate.
pub async fn assemble(
    db: &SqlitePool,
    config: &Config,
    site_name: &str,
    base_url: &str,
    extensions: &[Arc<dyn Extension>],
) -> Manifest {
    let mut ext_list = Vec::with_capacity(extensions.len());
    for (idx, e) in extensions.iter().enumerate() {
        if !is_active(db, e.id()).await {
            continue;
        }
        let lobby = lobby_config_for(db, config, e.id(), idx as i64).await;
        ext_list.push(ManifestExtension {
            id: e.id().to_string(),
            display_name: ManifestLocalized {
                ko: e.display_name(Lang::Ko),
                en: e.display_name(Lang::En),
            },
            lobby,
        });
    }
    Manifest {
        site: ManifestSite {
            name: site_name.to_string(),
            base_url: base_url.to_string(),
            default_lang: config.site.default_lang.clone(),
            languages: config.site.languages.clone(),
        },
        extensions: ext_list,
    }
}

/// Whether an extension's routes are live: `extension_state.enabled && !purged`.
///
/// A missing row is treated as active: the build may run on a DB that has not yet been seeded
/// (first boot seeds rows from `[extensions].enabled`), and silently dropping content there
/// would be worse than including it. Once seeded, the row authoritatively gates inclusion.
async fn is_active(db: &SqlitePool, ext_id: &str) -> bool {
    let row: Option<(i64, i64)> =
        sqlx::query_as("SELECT enabled, purged FROM extension_state WHERE extension_id = ?")
            .bind(ext_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
    match row {
        Some((enabled, purged)) => enabled != 0 && purged == 0,
        None => true,
    }
}

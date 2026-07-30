//! Site registry — startup-loaded per-site contexts for multi-site routing.
//!
//! All valid sites are loaded once at startup into a `SiteRegistry`. Request
//! handlers resolve the per-site DB pool via `site_db_middleware` which injects
//! `SiteScopedDb` into `Request::extensions()`.
//!
//! See spec §4.3 for architecture details.

use crate::loader;
use oxipage_core::sites::SitesFile;
use oxipage_core::config::Config;
use oxipage_core::extension::WasmLoader;
use oxipage_core::builder::BuildExt;
use oxipage_core::registry::ExtensionRegistry;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-site DB pool injected into request extensions by middleware.
///
/// Handlers extract it with `Extension(pool): Extension<SiteScopedDb>` and
/// use `pool.db` instead of the old `state.db`.
#[derive(Clone)]
pub struct SiteScopedDb {
    pub db: SqlitePool,
}

/// Context for one registered oxipage site.
pub struct SiteContext {
    pub slug: String,
    pub path: PathBuf,
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub registry: Arc<ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}

/// Startup-loaded registry of all known sites.
///
/// `sites` is loaded once from `SitesFile` on construction. Adding/removing
/// sites requires a restart (v2.0 scope simplification, spec §4.3).
pub struct SiteRegistry {
    sites: RwLock<HashMap<String, Arc<SiteContext>>>,
    sites_file: RwLock<SitesFile>,
}

impl SiteRegistry {
    /// Load all valid sites from `SitesFile`. Invalid entries (missing path,
    /// missing oxipage.toml, DB connect failure) are skipped with a warning.
    pub async fn new(sites_file: SitesFile) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        for (slug, entry) in &sites_file.sites {
            if !entry.path.exists() {
                tracing::warn!(slug, path = %entry.path.display(), "site path missing; skipping");
                continue;
            }
            match loader::SiteLoader::load(slug.clone(), entry.path.clone()).await {
                Ok(ctx) => {
                    map.insert(slug.clone(), Arc::new(ctx));
                }
                Err(e) => {
                    tracing::warn!(slug, error = %e, "failed to load site; skipping");
                }
            }
        }
        Ok(Self {
            sites: RwLock::new(map),
            sites_file: RwLock::new(sites_file),
        })
    }

    /// Look up a site's DB pool by slug. Returns `None` for unknown slugs.
    pub async fn db_for(&self, slug: &str) -> Option<SqlitePool> {
        self.sites.read().await.get(slug).map(|c| c.db.clone())
    }

    /// Look up a site's full context by slug.
    pub async fn ctx_for(&self, slug: &str) -> Option<Arc<SiteContext>> {
        self.sites.read().await.get(slug).cloned()
    }

    /// Return the default site slug: `default_site` from sites.toml, or the
    /// first registered site, or `None` if no sites are registered.
    pub async fn default_slug(&self) -> Option<String> {
        let sf = self.sites_file.read().await;
        sf.default_site
            .clone()
            .or_else(|| sf.sites.keys().next().cloned())
    }

    /// Sync iteration over all loaded sites (for route construction at startup).
    pub fn iter_blocking(&self) -> Vec<(String, Arc<SiteContext>)> {
        self.sites
            .try_read()
            .map(|guard| guard.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// Return a sorted list of loaded site slugs.
    pub fn slugs(&self) -> Vec<String> {
        self.sites
            .try_read()
            .map(|guard| {
                let mut slugs: Vec<_> = guard.keys().cloned().collect();
                slugs.sort();
                slugs
            })
            .unwrap_or_default()
    }

    /// Number of loaded sites.
    pub fn len(&self) -> usize {
        self.sites.try_read().map(|g| g.len()).unwrap_or(0)
    }
}

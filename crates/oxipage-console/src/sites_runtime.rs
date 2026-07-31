//! Site registry — startup-loaded per-site contexts for multi-site routing.
//!
//! All valid sites are loaded once at startup into a `SiteRegistry`. Request
//! handlers resolve the per-site DB pool via `site_db_middleware` which injects
//! `SiteScopedDb` into `Request::extensions()`.
//!
//! See spec §4.3 for architecture details.

pub use oxipage_core::state::SiteScopedDb;

use crate::loader;
use crate::operations::SiteOperationGuard;
use oxipage_core::builder::BuildExt;
use oxipage_core::extension::WasmLoader;
use oxipage_core::registry::ExtensionRegistry;
use oxipage_core::sites::SitesFile;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Context for one registered oxipage site.
pub struct SiteContext {
    pub slug: String,
    pub project_dir: PathBuf,
    pub data_dir: PathBuf,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    /// Startup-immutable server section (host/port/data_dir). Set once at load.
    pub startup_server: oxipage_core::config::ServerConfig,
    /// Live-reloadable site settings. Reloaded atomically on config PUT.
    pub settings: Arc<RwLock<oxipage_core::site_paths::MutableSiteSettings>>,
    /// Serializes concurrent config-write requests for this site.
    pub config_write_lock: Arc<tokio::sync::Mutex<()>>,
    pub db: SqlitePool,
    pub registry: Arc<ExtensionRegistry>,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    /// One build/deploy slot per site (shared guard, registry-level).
    pub operation_guard: Arc<SiteOperationGuard>,
    pub wasm_loader: Option<Arc<dyn WasmLoader>>,
}

/// Startup-loaded registry of all known sites.
///
/// `sites` is loaded once from `SitesFile` on construction. Adding/removing
/// sites requires a restart (v2.0 scope simplification, spec §4.3).
pub struct SiteRegistry {
    sites: RwLock<HashMap<String, Arc<SiteContext>>>,
    sites_file: RwLock<SitesFile>,
    pub operation_guard: Arc<SiteOperationGuard>,
}

impl SiteRegistry {
    /// Load all valid sites from `SitesFile`. Invalid entries (missing path,
    /// missing oxipage.toml, DB connect failure) are skipped with a warning.
    pub async fn new(
        sites_file: SitesFile,
        operation_guard: Arc<SiteOperationGuard>,
    ) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        for (slug, entry) in &sites_file.sites {
            if !entry.path.exists() {
                tracing::warn!(slug, path = %entry.path.display(), "site path missing; skipping");
                continue;
            }
            match loader::SiteLoader::load(
                slug.clone(),
                entry.path.clone(),
                operation_guard.clone(),
            )
            .await
            {
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
            operation_guard,
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

    /// Returns `true` if no sites are loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of loaded sites.
    pub fn len(&self) -> usize {
        self.sites.try_read().map(|g| g.len()).unwrap_or(0)
    }
    /// Return all loaded sites as a sorted list.
    pub async fn all_sites(&self) -> Vec<(String, PathBuf, bool)> {
        let sites = self.sites.read().await;
        let sf = self.sites_file.read().await;
        let mut result: Vec<_> = sites
            .iter()
            .map(|(slug, ctx)| {
                let active = sf.default_site.as_deref() == Some(slug.as_str());
                (slug.clone(), ctx.project_dir.clone(), active)
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Return all registered sites from the file (including those not yet loaded).
    pub async fn all_sites_from_file(&self) -> Vec<(String, PathBuf, bool)> {
        let sf = self.sites_file.read().await;
        let loaded = self.sites.read().await;
        let mut result: Vec<_> = sf
            .sites
            .iter()
            .map(|(slug, entry)| {
                let active = sf.default_site.as_deref() == Some(slug.as_str());
                // Use the path from the file (always available)
                (slug.clone(), entry.path.clone(), active)
            })
            .collect();
        // Also include loaded sites that might not be in the file yet
        for (slug, ctx) in loaded.iter() {
            if !result.iter().any(|(s, _, _)| s == slug) {
                let active = sf.default_site.as_deref() == Some(slug.as_str());
                result.push((slug.clone(), ctx.project_dir.clone(), active));
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Dynamically add a site context to the in-memory registry.
    pub async fn add_site(&self, slug: &str, ctx: Arc<SiteContext>) {
        self.sites.write().await.insert(slug.to_string(), ctx);
    }

    /// Register a site slug+path in the in-memory sites file.
    pub async fn register_in_file(&self, slug: &str, path: PathBuf) {
        let mut sf = self.sites_file.write().await;
        sf.add(slug.to_string(), path);
        if sf.default_site.is_none() {
            sf.set_default(slug);
        }
    }

    /// Remove a site from memory and the persisted sites file. Files on disk
    /// are preserved (registry-only delete).
    ///
    /// Removing a slug that isn't registered is a no-op.
    pub async fn remove_site(&self, slug: &str) -> anyhow::Result<()> {
        // 1. Remove from in-memory HashMap.
        self.sites.write().await.remove(slug);

        // 2. Remove from persisted sites.toml and from the in-memory SitesFile.
        let sites_path = directories::ProjectDirs::from("dev", "oxipage", "oxipage")
            .map(|p| p.config_dir().join("sites.toml"));
        if let Some(sp) = sites_path {
            let mut sf = if sp.exists() {
                std::fs::read_to_string(&sp)
                    .ok()
                    .and_then(|raw| toml::from_str::<SitesFile>(&raw).ok())
                    .unwrap_or_default()
            } else {
                SitesFile::default()
            };
            let removed = sf.remove(slug);
            if removed.is_none() {
                // Slug not in file — nothing to persist, but treat as success.
                return Ok(());
            }
            if let Some(parent) = sp.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let raw = toml::to_string_pretty(&sf)?;
            std::fs::write(&sp, raw)?;
            // Keep the in-memory SitesFile in sync with disk.
            *self.sites_file.write().await = sf;
        }
        Ok(())
    }

    /// Set the default site slug. Validates the slug is loaded (only a working
    /// site can be the default), then persists to sites.toml and syncs the
    /// in-memory SitesFile. Mirrors [`remove_site`](Self::remove_site).
    pub async fn set_default(&self, slug: &str) -> anyhow::Result<()> {
        if !self.sites.read().await.contains_key(slug) {
            anyhow::bail!("unknown site: {slug}");
        }

        let sites_path = directories::ProjectDirs::from("dev", "oxipage", "oxipage")
            .map(|p| p.config_dir().join("sites.toml"))
            .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;
        let mut sf = if sites_path.exists() {
            std::fs::read_to_string(&sites_path)
                .ok()
                .and_then(|raw| toml::from_str::<SitesFile>(&raw).ok())
                .unwrap_or_default()
        } else {
            SitesFile::default()
        };
        sf.set_default(slug);
        if let Some(parent) = sites_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(&sf)?;
        std::fs::write(&sites_path, raw)?;
        *self.sites_file.write().await = sf;
        Ok(())
    }
}

impl SiteRegistry {
    /// Build an empty registry for tests. No sites loaded, no default slug,
    /// no-op build/deploy guards. Used by integration tests that exercise the
    /// top-level router without booting a real site.
    pub async fn empty_for_tests() -> Arc<Self> {
        let sites_file = oxipage_core::sites::SitesFile::default();
        let guard = Arc::new(crate::operations::SiteOperationGuard::new());
        Arc::new(
            SiteRegistry::new(sites_file, guard)
                .await
                .expect("empty SitesFile always loads"),
        )
    }
}

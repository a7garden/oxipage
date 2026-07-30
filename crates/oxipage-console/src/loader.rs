//! Site loader — reads oxipage.toml, connects DB, runs migrations for one site.
//!
//! Called by `SiteRegistry::new()` at startup for each valid site entry.

use crate::sites_runtime::SiteContext;
use crate::build::build_run::BuildGuard;
use crate::deploy::deploy_run::DeployGuard;

use oxipage_core::config::Config;
use oxipage_core::extension::WasmLoader;
use oxipage_core::registry::ExtensionRegistry;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SiteLoader;

impl SiteLoader {
    /// Load a single site context from its project directory.
    ///
    /// Reads `oxipage.toml`, connects to `data/oxipage.db`, builds an
    /// extension registry, runs pending migrations.
    pub async fn load(slug: String, path: PathBuf, build_guard: Arc<BuildGuard>, deploy_guard: Arc<DeployGuard>) -> anyhow::Result<SiteContext> {
        let toml_path = path.join("oxipage.toml");
        let cfg = Config::load(&toml_path)?;
        // Resolve data_dir relative to the site project directory, not CWD.
        let data_dir = if cfg.server.data_dir.is_relative() {
            path.join(&cfg.server.data_dir)
        } else {
            cfg.server.data_dir.clone()
        };
        tokio::fs::create_dir_all(&data_dir).await?;
        let db_path = data_dir.join("oxipage.db");
        let db = oxipage_core::db::connect(&db_path).await?;
        let toml_enabled = cfg.extensions.enabled.clone();
        let extensions = crate::all_extensions();
        let registry = Arc::new(ExtensionRegistry::new(extensions));
        registry.run_migrations(&db, &toml_enabled).await?;
        let wasm_loader: Option<Arc<dyn WasmLoader>> = None;
        Ok(SiteContext {
            slug,
            path,
            config: Arc::new(cfg),
            db,
            registry,
            builders: Arc::new(crate::all_builders()),
            build_guard,
            deploy_guard,
            wasm_loader,
        })
    }
}

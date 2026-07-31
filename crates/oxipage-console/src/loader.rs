//! Site loader — reads oxipage.toml, connects DB, runs migrations for one site.
//!
//! Called by `SiteRegistry::new()` at startup for each valid site entry.

use crate::build::build_run::BuildGuard;
use crate::deploy::deploy_run::DeployGuard;
use crate::sites_runtime::SiteContext;

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
    pub async fn load(
        slug: String,
        path: PathBuf,
        build_guard: Arc<BuildGuard>,
        deploy_guard: Arc<DeployGuard>,
    ) -> anyhow::Result<SiteContext> {
        let toml_path = path.join("oxipage.toml");
        let cfg = Config::load(&toml_path)?;
        // Canonicalize project_dir so all derived paths share a baseline.
        let project_dir = path.canonicalize().unwrap_or(path);
        // Resolve data_dir relative to the site project directory, not CWD.
        let data_dir = if cfg.server.data_dir.is_absolute() {
            cfg.server.data_dir.clone()
        } else {
            project_dir.join(&cfg.server.data_dir)
        };
        tokio::fs::create_dir_all(&data_dir).await?;
        let out_dir = data_dir.join("out");
        let media_dir = data_dir.join("media");
        let db_path = data_dir.join("oxipage.db");
        let db = oxipage_core::db::connect(&db_path).await?;
        let toml_enabled = cfg.extensions.enabled.clone();
        let extensions = crate::all_extensions();
        let registry = Arc::new(ExtensionRegistry::new(extensions));
        registry.run_migrations(&db, &toml_enabled).await?;
        let wasm_loader: Option<Arc<dyn WasmLoader>> = None;
        Ok(SiteContext {
            slug,
            project_dir,
            data_dir,
            out_dir,
            media_dir,
            startup_server: cfg.server.clone(),
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

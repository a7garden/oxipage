//! Create-site handler — seeds a new oxipage project directory and
//! registers it in sites.toml. Called by the setup wizard and the
//! "new site" UI.

use oxipage_core::sites::SitesFile;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

use crate::loader::SiteLoader;
use crate::sites_runtime::SiteRegistry;

#[derive(Deserialize)]
pub struct CreateSiteInput {
    pub path: String,
}

/// Resolve ~/.config/oxipage/sites.toml path.
fn sites_path() -> anyhow::Result<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "oxipage", "oxipage")
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;
    Ok(proj.config_dir().join("sites.toml"))
}

/// Load SitesFile from disk. Returns empty if file missing.
fn load_sites() -> SitesFile {
    let path = match sites_path() {
        Ok(p) => p,
        Err(_) => return SitesFile::default(),
    };
    if !path.exists() {
        return SitesFile::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| toml::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Save SitesFile to disk.
fn save_sites(sf: &SitesFile) -> Result<(), String> {
    let path = sites_path().map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = toml::to_string_pretty(sf).map_err(|e| e.to_string())?;
    std::fs::write(&path, &raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
    Ok(())
}

/// Create a site on disk and optionally register it in the in-memory registry.
/// Returns (slug, path).
pub async fn create_site(
    path: PathBuf,
    registry: Option<&Arc<SiteRegistry>>,
) -> Result<(String, String), String> {
    // Validate path
    if path.exists() {
        if !path.join("oxipage.toml").exists() {
            return Err(format!(
                "path '{}' exists but is not an oxipage project (no oxipage.toml)",
                path.display()
            ));
        }
    } else {
        std::fs::create_dir_all(&path).map_err(|e| format!("cannot create directory: {e}"))?;
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
            .map_err(|e| format!("cannot write oxipage.toml: {e}"))?;
    }

    // Derive slug from directory name
    let slug = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("site")
        .to_string();

    // Register in sites.toml
    let mut sf = load_sites();
    sf.add(slug.clone(), path.clone());
    if sf.default_site.is_none() {
        sf.set_default(&slug);
    }
    save_sites(&sf)?;

    // Register in in-memory registry if available
    if let Some(registry) = registry {
        match SiteLoader::load(slug.clone(), path.clone()).await {
            Ok(ctx) => {
                registry.add_site(&slug, Arc::new(ctx)).await;
            }
            Err(e) => {
                tracing::warn!(slug, path = %path.display(), error = %e, "create-site: registry init skipped");
            }
        }
    }

    Ok((slug, path.to_string_lossy().into_owned()))
}

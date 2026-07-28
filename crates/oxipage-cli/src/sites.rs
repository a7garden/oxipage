//! Site profiles — named connection profiles (endpoint + token) stored in
//! `~/.config/oxipage/sites.toml` (doc/09).
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// ──────────────────────────── data model ────────────────────────────

/// Top-level `~/.config/oxipage/sites.toml` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitesFile {
    #[serde(default)]
    pub default_site: Option<String>,
    #[serde(default)]
    pub sites: BTreeMap<String, SiteEntry>,
}

/// One named site entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl SitesFile {
    /// Load sites from disk. If the file doesn't exist or is empty, return
    /// an empty `SitesFile` — not an error, so existing users without a
    /// sites file still work (legacy fallback).
    pub fn load() -> Self {
        let path = match sites_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };
        if !path.exists() {
            return Self::default();
        }
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        if raw.trim().is_empty() {
            return Self::default();
        }
        toml::from_str(&raw).unwrap_or_else(|e| {
            // Corrupt file → warn and fall back to empty. Never crash CLI
            // because the sites file is malformed — the user can fix it
            // with `oxipage site add/list/rm`.
            eprintln!("warning: ~/.config/oxipage/sites.toml is corrupt: {e}");
            Self::default()
        })
    }

    /// Save sites to disk (0600 permissions).
    pub fn save(&self) -> Result<()> {
        let path = sites_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)
            .context("serializing sites")?;
        fs::write(&path, &raw)
            .with_context(|| format!("writing {}", path.display()))?;
        set_mode_0600(&path)?;
        Ok(())
    }

    /// Resolve the effective site name: `--site` flag → OXIPAGE_SITE env →
    /// `default_site` → `None`.
    ///
    /// Caller (`commands::resolve_site_name`) validates `--site` existence
    /// and bails on unknown names before calling this. This function
    /// assumes the input has been validated.
    pub fn resolve_name<'a>(&'a self, cli_site: Option<&'a str>) -> Option<&'a str> {
        // 1. --site flag (already validated by caller)
        if let Some(name) = cli_site {
            if !name.is_empty() && self.sites.contains_key(name) {
                return Some(name);
            }
        }
        // 2. OXIPAGE_SITE env
        if let Ok(env) = std::env::var("OXIPAGE_SITE") {
            if !env.is_empty() && self.sites.contains_key(&env) {
                return self.sites.get_key_value(&env).map(|(k, _)| k.as_str());
            }
        }
        // 3. default_site from file
        self.default_site.as_deref()
            .and_then(|name| self.sites.contains_key(name).then_some(name))
    }

    /// Check whether a given site name exists.
    pub fn exists(&self, name: &str) -> bool {
        self.sites.contains_key(name)
    }

    /// Get a site entry by name.
    pub fn get(&self, name: &str) -> Option<&SiteEntry> {
        self.sites.get(name)
    }

    /// Resolve the endpoint for a resolved site name. If the site name is
    /// `None` or the named site isn't found, returns `None` (caller falls
    /// through to legacy chain).
    pub fn resolve_endpoint(&self, site_name: Option<&str>) -> Option<String> {
        let name = site_name?;
        self.sites.get(name).map(|s| s.endpoint.clone())
    }

    /// Resolve the token for a resolved site name. If the site has no token
    /// (`None`/empty), returns `None` so the caller can fall through to env
    /// / credentials (doc/09 §9.5 — independent fallthrough).
    pub fn resolve_token(&self, site_name: Option<&str>) -> Option<String> {
        let name = site_name?;
        self.sites.get(name)
            .and_then(|s| s.token.as_ref())
            .filter(|t| !t.is_empty())
            .cloned()
    }

    /// Return a sorted list of site names (for `list` display).
    pub fn site_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.sites.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for SitesFile {
    fn default() -> Self {
        SitesFile {
            default_site: None,
            sites: BTreeMap::new(),
        }
    }
}

// ──────────────────────────── path resolution ────────────────────────────

/// ~/.config/oxipage/sites.toml
fn sites_path() -> Result<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "oxipage", "oxipage")
        .context("could not determine config directory")?;
    Ok(proj.config_dir().join("sites.toml"))
}

// ──────────────────────────── permissions ────────────────────────────

#[cfg(unix)]
fn set_mode_0600(p: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(p, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode_0600(_p: &std::path::Path) -> Result<()> {
    Ok(())
}

// ──────────────────────────── tests ────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sites() -> SitesFile {
        let mut sites = BTreeMap::new();
        sites.insert(
            "selfhost".into(),
            SiteEntry {
                endpoint: "http://localhost:8787".into(),
                token: Some("tok_self".into()),
            },
        );
        sites.insert(
            "flyio".into(),
            SiteEntry {
                endpoint: "https://oxipage.fly.dev".into(),
                token: None,
            },
        );
        SitesFile {
            default_site: Some("selfhost".into()),
            sites,
        }
    }

    #[test]
    fn test_resolve_name_flag() {
        let sf = sample_sites();
        // --site flyio
        assert_eq!(sf.resolve_name(Some("flyio")), Some("flyio"));
    }

    #[test]
    fn test_resolve_name_default() {
        let sf = sample_sites();
        // no flag, no env → default_site
        assert_eq!(sf.resolve_name(None), Some("selfhost"));
    }

    #[test]
    fn test_resolve_name_none() {
        let sf = SitesFile::default();
        assert_eq!(sf.resolve_name(None), None);
    }

    #[test]
    fn test_resolve_endpoint() {
        let sf = sample_sites();
        assert_eq!(
            sf.resolve_endpoint(Some("selfhost")).as_deref(),
            Some("http://localhost:8787")
        );
        assert_eq!(sf.resolve_endpoint(None), None);
    }

    #[test]
    fn test_resolve_token_with_token() {
        let sf = sample_sites();
        assert_eq!(
            sf.resolve_token(Some("selfhost")).as_deref(),
            Some("tok_self")
        );
    }

    #[test]
    fn test_resolve_token_without_token() {
        let sf = sample_sites();
        // flyio has no token → fall through (None)
        assert_eq!(sf.resolve_token(Some("flyio")), None);
    }

    #[test]
    fn test_resolve_token_none() {
        let sf = sample_sites();
        assert_eq!(sf.resolve_token(None), None);
    }

    #[test]
    fn test_exists() {
        let sf = sample_sites();
        assert!(sf.exists("selfhost"));
        assert!(!sf.exists("nonexistent"));
    }

    #[test]
    fn test_site_names() {
        let sf = sample_sites();
        let names = sf.site_names();
        assert_eq!(names, vec!["flyio", "selfhost"]);
    }

    #[test]
    fn test_roundtrip_serialize() {
        let sf = sample_sites();
        let raw = toml::to_string_pretty(&sf).unwrap();
        let deser: SitesFile = toml::from_str(&raw).unwrap();
        assert_eq!(deser.default_site, sf.default_site);
        assert_eq!(deser.sites.len(), 2);
        assert_eq!(
            deser.sites.get("selfhost").unwrap().endpoint,
            "http://localhost:8787"
        );
        // flyio should have no token field
        assert!(deser.sites.get("flyio").unwrap().token.is_none());
    }
}

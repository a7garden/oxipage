//! Site profile data model — used by both CLI and console for `sites.toml`.
//!
//! File I/O (`load`/`save` with permissions) lives in `oxipage` (CLI crate)
//! under `crate::sites`. This module provides only the type definitions plus
//! in-memory data access.
//!
//! v2 SSG: each site = local directory with its own `oxipage.toml` + `oxipage.db`.
//! Remote endpoint/token model removed (spec D1, D7).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ──────────────────────────── data model ────────────────────────────

/// Top-level `~/.config/oxipage/sites.toml` file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SitesFile {
    #[serde(default)]
    pub default_site: Option<String>,
    #[serde(default)]
    pub sites: BTreeMap<String, SiteEntry>,
}

/// One named site entry — a local oxipage project directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    pub path: PathBuf,
}

// ──────────────────────────── pure-data methods ────────────────────────────

impl SitesFile {
    /// Resolve the effective site name: `--site` flag → OXIPAGE_SITE env →
    /// `default_site` → `None`.
    ///
    /// Caller validates `--site` existence before passing to this function.
    /// This function assumes the input has been validated.
    pub fn resolve_name<'a>(&'a self, cli_site: Option<&'a str>) -> Option<&'a str> {
        if let Some(name) = cli_site
            && !name.is_empty()
            && self.sites.contains_key(name)
        {
            return Some(name);
        }
        if let Ok(env) = std::env::var("OXIPAGE_SITE")
            && !env.is_empty()
            && self.sites.contains_key(&env)
        {
            return self.sites.get_key_value(&env).map(|(k, _)| k.as_str());
        }
        self.default_site
            .as_deref()
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

    /// Return a sorted list of site names (for `list` display).
    pub fn site_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.sites.keys().cloned().collect();
        names.sort();
        names
    }

    /// Add or replace a site entry. Returns the old entry if one existed.
    pub fn add(&mut self, slug: String, path: PathBuf) -> Option<SiteEntry> {
        self.sites.insert(slug, SiteEntry { path })
    }

    /// Remove a site entry. Returns the removed entry if one existed.
    pub fn remove(&mut self, slug: &str) -> Option<SiteEntry> {
        self.sites.remove(slug)
    }

    /// Set the default site slug. No-op if slug is empty.
    pub fn set_default(&mut self, slug: &str) {
        if !slug.is_empty() {
            self.default_site = Some(slug.to_string());
        }
    }
}

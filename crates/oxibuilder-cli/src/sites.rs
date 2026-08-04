//! Site profiles — file I/O for `~/.config/oxibuilder/sites.toml` (doc/09).
//!
//! The data model (`SitesFile`, `SiteEntry`) lives in `oxibuilder-core` and is
//! re-exported here for backward compat within the binary-only CLI crate.
//! This module handles on-disk persistence: load, save, path resolution,
//! and Unix permissions.
//!
//! # Orphan-rule note
//!
//! `SitesFile` / `SiteEntry` are defined in `oxibuilder-core`, so `impl` blocks
//! for them can only live there. File I/O (`load`, `save`) is provided as
//! free functions (`load_sites`, `save_sites`).

pub use oxibuilder_core::sites::{SiteEntry, SitesFile};

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

// ──────────────────────────── file I/O ────────────────────────────

/// Load sites from disk. If the file doesn't exist or is empty, return
/// an empty `SitesFile` — not an error, so existing users without a
/// sites file still work (legacy fallback).
pub fn load_sites() -> SitesFile {
    let path = match sites_path() {
        Ok(p) => p,
        Err(_) => return SitesFile::default(),
    };
    if !path.exists() {
        return SitesFile::default();
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return SitesFile::default(),
    };
    if raw.trim().is_empty() {
        return SitesFile::default();
    }
    toml::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("warning: ~/.config/oxibuilder/sites.toml is corrupt: {e}");
        SitesFile::default()
    })
}

/// Save sites to disk (0600 permissions).
pub fn save_sites(sf: &SitesFile) -> Result<()> {
    let path = sites_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let raw = toml::to_string_pretty(sf).context("serializing sites")?;
    fs::write(&path, &raw).with_context(|| format!("writing {}", path.display()))?;
    set_mode_0600(&path)?;
    Ok(())
}

// ──────────────────────────── path resolution ────────────────────────────

/// ~/.config/oxibuilder/sites.toml
pub fn sites_path() -> Result<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "oxibuilder", "oxibuilder")
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
    use std::collections::BTreeMap;

    fn sample_sites() -> SitesFile {
        let mut sites = BTreeMap::new();
        sites.insert(
            "selfhost".into(),
            SiteEntry {
                path: PathBuf::from("/tmp/oxibuilder/selfhost"),
            },
        );
        sites.insert(
            "flyio".into(),
            SiteEntry {
                path: PathBuf::from("/tmp/oxibuilder/flyio"),
            },
        );
        SitesFile {
            default_site: Some("selfhost".into()),
            sites,
        }
    }

    #[test]
    fn site_entry_round_trips_path_only() {
        let sf = SitesFile {
            default_site: Some("blog".into()),
            sites: BTreeMap::from([(
                "blog".into(),
                SiteEntry {
                    path: PathBuf::from("/tmp/oxibuilder/test-blog"),
                },
            )]),
        };
        let raw = toml::to_string(&sf).unwrap();
        assert!(raw.contains("path"));
        assert!(!raw.contains("endpoint"));
        let back: SitesFile = toml::from_str(&raw).unwrap();
        assert_eq!(
            back.sites["blog"].path,
            PathBuf::from("/tmp/oxibuilder/test-blog")
        );
    }

    #[test]
    fn test_resolve_name_flag() {
        let sf = sample_sites();
        assert_eq!(sf.resolve_name(Some("flyio")), Some("flyio"));
    }

    #[test]
    fn test_resolve_name_default() {
        let sf = sample_sites();
        assert_eq!(sf.resolve_name(None), Some("selfhost"));
    }

    #[test]
    fn test_resolve_name_none() {
        let sf = SitesFile::default();
        assert_eq!(sf.resolve_name(None), None);
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
    fn test_add_remove() {
        let mut sf = SitesFile::default();
        sf.add("blog".into(), PathBuf::from("/tmp/blog"));
        assert!(sf.exists("blog"));
        sf.remove("blog");
        assert!(!sf.exists("blog"));
    }

    #[test]
    fn test_set_default() {
        let mut sf = SitesFile::default();
        sf.set_default("blog");
        assert_eq!(sf.default_site.as_deref(), Some("blog"));
    }

    #[test]
    fn test_roundtrip_serialize() {
        let sf = sample_sites();
        let raw = toml::to_string_pretty(&sf).unwrap();
        let deser: SitesFile = toml::from_str(&raw).unwrap();
        assert_eq!(deser.default_site, sf.default_site);
        assert_eq!(deser.sites.len(), 2);
        assert_eq!(
            deser.sites.get("selfhost").unwrap().path,
            PathBuf::from("/tmp/oxibuilder/selfhost")
        );
    }
}

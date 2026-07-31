//! Build manifest — single typed source of truth for "what did the build produce?".
//!
//! Written to `<out_dir>/.oxipage-build.json` by `build_writer` after every
//! successful build. Consumed by:
//! - `oxipage_console::preview::handler` (to decide 424 vs serve, and to write
//!   the per-request `<base href>`),
//! - the per-site `build_post` status response (so the UI can render build ID,
//!   theme, deployment base),
//! - `oxipage-deploy` (deployment base + asset revision for GitHub Pages).
//!
//! `deployment_base` is always derived from `MutableSiteSettings::site.base_url`
//! via [`derive_deployment_base`] — it is the canonical "where the deployed
//! artifact will live" base for the build. The preview handler OVERRIDES the
//! served `<base href>` at request time to inject the live preview prefix
//! (the persisted manifest value is still the artifact's canonical base).
//!
//! See spec §4.2, §5, §6.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Manifest filename written inside `out_dir`. Leading dot keeps it adjacent
/// to the deploy artifact without competing with user-facing routes.
pub const MAG_FILENAME: &str = ".oxipage-build.json";

/// One build's metadata. Serialized to `out_dir/.oxipage-build.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildManifest {
    /// UUIDv4 assigned at write time.
    pub build_id: String,
    /// Base URL prefix the deployed static files will be served under.
    /// Always starts with `/` and ends with `/`. Empty → `/`.
    /// Derived from `site.base_url` via [`derive_deployment_base`] at build time.
    pub deployment_base: String,
    /// Theme id active at build time (e.g. `"paper"`).
    pub theme_id: String,
    /// SHA-256 hash of the materialized asset set (hex prefix).
    pub asset_revision: String,
    /// RFC3339 timestamp the build finished.
    pub built_at: String,
}

impl BuildManifest {
    /// Low-level constructor. The caller is responsible for `deployment_base`
    /// being already normalized (leading + trailing slash). Prefer
    /// [`BuildManifest::from_site_base`] for the production path.
    pub fn new(
        deployment_base: impl Into<String>,
        theme_id: impl Into<String>,
        asset_revision: impl Into<String>,
    ) -> Self {
        let base = deployment_base.into();
        Self {
            build_id: Uuid::new_v4().to_string(),
            deployment_base: normalize_base(base),
            theme_id: theme_id.into(),
            asset_revision: asset_revision.into(),
            built_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }
    }

    /// Production constructor. Derives `deployment_base` from the site's
    /// `base_url` (a full URL like `https://example.com/repo/` or
    /// `http://127.0.0.1:8787/`) using the single derivation rule.
    pub fn from_site_base(
        site_base_url: &str,
        theme_id: impl Into<String>,
        asset_revision: impl Into<String>,
    ) -> Self {
        Self::new(
            derive_deployment_base(site_base_url),
            theme_id,
            asset_revision,
        )
    }

    /// Read the manifest from `<out_dir>/.oxipage-build.json`. Returns `Ok(None)`
    /// if the file is absent (not an error — the build hasn't run yet).
    pub fn read_from(out_dir: &Path) -> Result<Option<Self>, ManifestError> {
        let path = out_dir.join(MAG_FILENAME);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)?;
        let parsed: Self = serde_json::from_str(&raw)?;
        Ok(Some(parsed))
    }

    /// Atomically write the manifest to `<out_dir>/.oxipage-build.json`.
    /// Creates the directory if missing. Writes to a temp file in the same
    /// directory then renames — a read on the live path never sees a partial
    /// payload.
    pub fn write_to(&self, out_dir: &Path) -> Result<(), ManifestError> {
        fs::create_dir_all(out_dir)?;
        let final_path = out_dir.join(MAG_FILENAME);
        let tmp = out_dir.join(format!("{}.tmp", MAG_FILENAME));
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &final_path)?;
        Ok(())
    }
}

/// Single derivation rule for `deployment_base` from `site.base_url`.
///
/// - Parse the URL. On failure, return `/`.
/// - Take the URL pathname. Strip a trailing slash (if any).
/// - Prepend `/`. If the resulting path is just `/`, return `/`.
/// - Append a trailing `/`. Always return a path that starts with `/` and ends with `/`.
///
/// Examples:
///   `https://a7garden.github.io/`        → `/`
///   `https://a7garden.github.io/blog/`   → `/blog/`
///   `https://example.com/deep/nested/`   → `/deep/nested/`
///   `http://127.0.0.1:8787/`             → `/`
///   `not a url`                          → `/`
pub fn derive_deployment_base(base_url: &str) -> String {
    let parsed = url::Url::parse(base_url).ok();
    let raw_path = match parsed.as_ref() {
        Some(u) => u.path().to_string(),
        None => return "/".to_string(),
    };
    let trimmed = raw_path.trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    if !trimmed.starts_with('/') {
        // Should not happen for a Url::parse result, but defensive.
        return format!("/{trimmed}/");
    }
    format!("{trimmed}/")
}

/// Internal helper that ensures a leading + trailing slash so the manifest
/// is always shape-stable. Empty becomes `/`.
fn normalize_base(s: String) -> String {
    if s.is_empty() {
        return "/".to_string();
    }
    let mut out = s;
    if !out.starts_with('/') {
        out.insert(0, '/');
    }
    if !out.ends_with('/') {
        out.push('/');
    }
    out
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid manifest json: {0}")]
    Json(#[from] serde_json::Error),
}

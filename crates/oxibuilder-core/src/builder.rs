//! Build pipeline types and trait for Static Site Generator (v2).
//!
//! Each extension implements `BuildExt` to generate:
//! - Static HTML pages (per-content, with OG metas for SEO)
//! - Client-side data (JSON for React SPA fetches)
//! - Search index documents (for client-side search)
//!
//! Build is CPU-bound and synchronous. Extensions are independent and
//! processed in parallel via rayon.

use std::error::Error;
use std::path::PathBuf;

use erased_serde::Serialize;
use sqlx::SqlitePool;

use crate::media::ImageManifest;

/// A single static HTML page produced during build.
pub struct StaticPage {
    /// Relative URL path, e.g. `"blog/hello-world/index.html"`.
    pub path: String,
    /// Full HTML content (including `<!DOCTYPE html>`, OG metas, etc.).
    pub content: String,
}

/// A document for the client-side search index.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchDoc {
    /// Unique document id, e.g. `"blog/hello-world"`.
    pub id: String,
    pub title: String,
    pub body_preview: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub url: String,
    pub published_at: Option<String>,
}

/// Output from a single extension's build.
pub struct ExtBuildOutput {
    pub ext_id: String,
    pub pages: Vec<StaticPage>,
    pub data: Box<dyn erased_serde::Serialize + Send>,
    pub search_docs: Vec<SearchDoc>,
}

/// Aggregated build output from all extensions.
pub struct BuildOutput {
    pub pages: Vec<StaticPage>,
    pub search_docs: Vec<SearchDoc>,
    pub extensions_data: Vec<(String, Box<dyn erased_serde::Serialize + Send>)>,
}

/// Each extension implements this to participate in static site generation.
///
/// Implementors must be `Send + Sync` so rayon can process them in parallel.
///
/// Methods are synchronous but perform async DB I/O via `rt.block_on(...)`. The
/// `Handle` is captured once on the Tokio runtime thread (before rayon) and passed
/// in, because `tokio::runtime::Handle::current()` panics on a rayon worker thread
/// (no runtime is bound there). `Handle::block_on` is safe to call from any thread.
pub trait BuildExt: Send + Sync {
    /// Extension identifier, e.g. `"blog"`, `"projects"`.
    fn ext_id(&self) -> &'static str;

    /// Generate static HTML pages for published content.
    ///
    /// URL path convention: `{ext_id}/{slug}/index.html`.
    fn build_pages(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>>;

    /// Generate client-side data as a serializable object.
    ///
    /// Will be written to `out/data/{ext_id}.json`.
    fn build_data(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn Serialize + Send>, Box<dyn Error + Send + Sync>>;

    /// Generate search index documents for this extension's published content.
    fn build_search_docs(
        &self,
        db: &SqlitePool,
        rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn Error + Send + Sync>>;
 }

impl BuildOutput {
    /// Merge multiple `ExtBuildOutput` values into a single `BuildOutput`.
    pub fn merge(results: impl IntoIterator<Item = ExtBuildOutput>) -> Self {
        let mut pages = Vec::new();
        let mut search_docs = Vec::new();
        let mut extensions_data = Vec::new();

        for r in results {
            pages.extend(r.pages);
            search_docs.extend(r.search_docs);
            extensions_data.push((r.ext_id, r.data));
        }

        BuildOutput {
            pages,
            search_docs,
            extensions_data,
        }
    }
}

/// Inputs to the build writer that aren't part of the per-extension output.
///
/// `deployment_base` is NOT passed here. It is derived from `site_base_url`
/// inside `write_build_output` via `BuildManifest::from_site_base` so the
/// single derivation rule is enforced at exactly one site.
#[derive(Debug, Clone)]
pub struct BuildInputs {
    /// Full site URL from `MutableSiteSettings::site.base_url`. Used to derive
    /// `deployment_base` (e.g. `https://a7garden.github.io/blog/` → `/blog/`).
    pub site_base_url: String,
    /// Theme id active at build time.
    pub theme_id: String,
    /// Caller-supplied seed for the asset revision. The build writer hashes
    /// it with the materialized file list to produce a stable revision.
    pub asset_revision_seed: String,
    /// Directory under which the image pre-pass writes derived WebP variants
    /// (`<staging>/media/_derived/...`). OUTSIDE `out/` so the
    /// `write_build_output` wipe doesn't destroy the optimized files; the
    /// writer copies them into `out/media/_derived/` after the wipe.
    /// `None` skips the copy entirely (no images to stage).
    pub image_staging_dir: Option<PathBuf>,
    /// Manifest produced by `media::optimize` from the staged images. Copied
    /// to `out/data/image-manifest.json` for the static-mode SPA plugin
    /// (Task 6) to read. `None` skips manifest emission.
    pub image_manifest: Option<ImageManifest>,
}

impl BuildInputs {
    pub fn new(
        site_base_url: impl Into<String>,
        theme_id: impl Into<String>,
        asset_revision_seed: impl Into<String>,
    ) -> Self {
        Self {
            site_base_url: site_base_url.into(),
            theme_id: theme_id.into(),
            asset_revision_seed: asset_revision_seed.into(),
            image_staging_dir: None,
            image_manifest: None,
        }
    }
}

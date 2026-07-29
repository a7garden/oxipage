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

use erased_serde::Serialize;
use sqlx::SqlitePool;

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
/// All methods are synchronous — build is CPU-bound.
pub trait BuildExt: Send + Sync {
    /// Extension identifier, e.g. `"blog"`, `"projects"`.
    fn ext_id(&self) -> &'static str;

    /// Generate static HTML pages for published content.
    ///
    /// URL path convention: `{ext_id}/{slug}/index.html`.
    fn build_pages(&self, db: &SqlitePool)
    -> Result<Vec<StaticPage>, Box<dyn Error + Send + Sync>>;

    /// Generate client-side data as a serializable object.
    ///
    /// Will be written to `out/data/{ext_id}.json`.
    fn build_data(
        &self,
        db: &SqlitePool,
    ) -> Result<Box<dyn Serialize + Send>, Box<dyn Error + Send + Sync>>;

    /// Generate search index documents for this extension's published content.
    fn build_search_docs(
        &self,
        db: &SqlitePool,
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

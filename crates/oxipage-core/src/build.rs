//! Parallel build pipeline for static site generation (v2).
//!
//! Uses rayon to process all extension builders concurrently.
//! Each extension independently produces pages, data, and search docs.

use std::error::Error;

use crate::builder::{BuildExt, BuildOutput, ExtBuildOutput};
use sqlx::SqlitePool;

/// Run all extension builders in parallel via rayon.
///
/// Each extension produces pages, data, and search docs independently.
/// Errors are collected per-extension and reported with context.
pub fn build_site(
    db: &SqlitePool,
    builders: &[Box<dyn BuildExt>],
) -> Result<BuildOutput, Box<dyn Error + Send + Sync>> {
    use rayon::prelude::*;

    let results: Vec<Result<ExtBuildOutput, String>> = builders
        .par_iter()
        .map(|ext| {
            let ext_id = ext.ext_id();
            let pages = ext
                .build_pages(db)
                .map_err(|e| format!("[{}] build_pages: {}", ext_id, e))?;
            let data = ext
                .build_data(db)
                .map_err(|e| format!("[{}] build_data: {}", ext_id, e))?;
            let search_docs = ext
                .build_search_docs(db)
                .map_err(|e| format!("[{}] build_search_docs: {}", ext_id, e))?;
            Ok(ExtBuildOutput {
                ext_id: ext_id.to_string(),
                pages,
                data,
                search_docs,
            })
        })
        .collect();

    // Collect errors or merge outputs
    let mut outputs = Vec::with_capacity(results.len());
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(output) => outputs.push(output),
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        return Err(format!("Build errors:\n{}", errors.join("\n")).into());
    }

    Ok(BuildOutput::merge(outputs))
}

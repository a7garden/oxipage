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

    // Capture the Tokio runtime handle ONCE here, on the runtime thread (this fn is
    // called from the async `build` command). Rayon worker threads have no runtime
    // bound, so `Handle::current()` inside the closure would panic. Passing the
    // captured handle lets each builder `block_on` its async DB work from any thread.
    let rt = tokio::runtime::Handle::current();

    let results: Vec<Result<ExtBuildOutput, String>> = builders
        .par_iter()
        .map(|ext| {
            let ext_id = ext.ext_id();
            let pages = ext
                .build_pages(db, &rt)
                .map_err(|e| format!("[{}] build_pages: {}", ext_id, e))?;
            let data = ext
                .build_data(db, &rt)
                .map_err(|e| format!("[{}] build_data: {}", ext_id, e))?;
            let search_docs = ext
                .build_search_docs(db, &rt)
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

/// Progress events emitted during a streaming build. Relayed to SSE clients
/// and to the build-log recorder. Serialized as `{"event":"<variant>", ...}`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum BuildEvent {
    BuildStarted { total: usize },
    ExtensionStart { ext_id: String },
    ExtensionDone { ext_id: String, pages: usize },
    BuildComplete { total_pages: usize },
    BuildFailed { error: String },
}

/// Streaming variant of [`build_site`].
///
/// Identical build semantics (rayon `par_iter`, per-extension error collection),
/// but emits [`BuildEvent`]s to `tx` as each extension starts/finishes and on
/// completion/failure.
///
/// Takes the Tokio runtime `Handle` explicitly because the console runs this
/// inside `spawn_blocking`, where `Handle::current()` would panic (blocking
/// threads carry no runtime context). `mpsc::Sender::blocking_send` is safe
/// from rayon/spawn_blocking threads (not an async execution context).
pub fn build_site_with_progress(
    db: &SqlitePool,
    builders: &[Box<dyn BuildExt>],
    rt: &tokio::runtime::Handle,
    tx: &tokio::sync::mpsc::Sender<BuildEvent>,
) -> Result<BuildOutput, Box<dyn Error + Send + Sync>> {
    use rayon::prelude::*;

    let total = builders.len();
    let _ = tx.blocking_send(BuildEvent::BuildStarted { total });

    let results: Vec<Result<ExtBuildOutput, String>> = builders
        .par_iter()
        .map(|ext| {
            let ext_id = ext.ext_id();
            let _ = tx.blocking_send(BuildEvent::ExtensionStart {
                ext_id: ext_id.to_string(),
            });
            let pages = ext
                .build_pages(db, rt)
                .map_err(|e| format!("[{}] build_pages: {}", ext_id, e))?;
            let page_count = pages.len();
            let data = ext
                .build_data(db, rt)
                .map_err(|e| format!("[{}] build_data: {}", ext_id, e))?;
            let search_docs = ext
                .build_search_docs(db, rt)
                .map_err(|e| format!("[{}] build_search_docs: {}", ext_id, e))?;
            let _ = tx.blocking_send(BuildEvent::ExtensionDone {
                ext_id: ext_id.to_string(),
                pages: page_count,
            });
            Ok(ExtBuildOutput {
                ext_id: ext_id.to_string(),
                pages,
                data,
                search_docs,
            })
        })
        .collect();

    let mut outputs = Vec::with_capacity(results.len());
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(output) => outputs.push(output),
            Err(e) => errors.push(e),
        }
    }

    if !errors.is_empty() {
        let error = format!("Build errors:\n{}", errors.join("\n"));
        let _ = tx.blocking_send(BuildEvent::BuildFailed {
            error: error.clone(),
        });
        return Err(error.into());
    }

    let merged = BuildOutput::merge(outputs);
    let _ = tx.blocking_send(BuildEvent::BuildComplete {
        total_pages: merged.pages.len(),
    });
    Ok(merged)
}

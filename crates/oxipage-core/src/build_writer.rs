//! Write build output to the `out/` directory.
//!
//! Handles creation of static HTML files, JSON data files, search index,
//! media copying, and SPA bundle emission from the embedded binary.

use crate::builder::BuildOutput;
use std::fs;
use std::path::Path;

/// Write a completed `BuildOutput` to the filesystem under `out_dir`.
///
/// Layout (v2 SSG):
/// ```text
/// out/
/// ├── index.html              ← SPA entry (embedded bundle)
/// ├── 404.html                ← SPA fallback for deep links on static hosts
/// ├── assets/…                ← hashed JS/CSS chunks (embedded bundle)
/// ├── blog/<slug>/index.html  ← per-content SEO shell (OG meta + hydrates SPA)
/// ├── data/<ext>.json         ← collection JSON the SPA fetches
/// ├── data/search-index.json
/// └── media/                  ← copied from /data/media/
/// ```
///
/// The SPA bundle is sourced from the embedded binary (`oxipage_core::http`),
/// NOT the working-directory `web/dist`, so a release binary builds a correct
/// site from any CWD.
pub fn write_build_output(
    output: &BuildOutput,
    out_dir: &Path,
    media_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Clean or create output directory
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    // Real <script>/<link> asset tags from the embedded SPA index.html, used to
    // fix up the per-content shells (which hardcode a non-hashed placeholder).
    let asset_tags = extract_asset_tags();

    // 2. Write all static pages, injecting the real hashed asset tags into HTML shells
    for page in &output.pages {
        let content = if page.path.ends_with(".html") {
            inject_assets(&page.content, asset_tags.as_deref())
        } else {
            page.content.clone()
        };
        let file_path = out_dir.join(&page.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &content)?;
    }

    // 3. Write extension data JSON files
    let data_dir = out_dir.join("data");
    fs::create_dir_all(&data_dir)?;
    for (ext_id, data) in &output.extensions_data {
        let path = data_dir.join(format!("{ext_id}.json"));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, &json)?;
    }

    // 4. Write search index
    let search_json = serde_json::to_string_pretty(&output.search_docs)?;
    fs::write(data_dir.join("search-index.json"), &search_json)?;

    // 5. Emit the embedded SPA bundle to out/ ROOT (so `/index.html` is the entry
    //    and `/assets/<hashed>.js` resolves). GitHub Pages has no SPA fallback, so
    //    also drop a `404.html` copy for deep-link routes not covered by a shell.
    write_embedded_assets(out_dir)?;
    let index_html = out_dir.join("index.html");
    if index_html.exists() {
        let _ = fs::copy(&index_html, out_dir.join("404.html"));
    }

    // 6. Copy media files
    if media_dir.exists() {
        copy_dir_recursive(media_dir, &out_dir.join("media"))?;
    }

    tracing::info!(
        pages = output.pages.len(),
        extensions = output.extensions_data.len(),
        dir = %out_dir.display(),
        "build output written"
    );

    Ok(())
}

/// Pull the hashed `<script>` and `<link rel="stylesheet">` tags out of the
/// embedded SPA `index.html`. Returns `None` if the SPA isn't embedded.
fn extract_asset_tags() -> Option<String> {
    let html = crate::http::spa_index_html()?;
    let mut tags = Vec::new();
    for line in html.lines() {
        let t = line.trim();
        if t.starts_with("<script ") || t.starts_with("<link rel=\"stylesheet") {
            tags.push(t.to_string());
        }
    }
    if tags.is_empty() {
        None
    } else {
        Some(tags.join("\n    "))
    }
}

/// Replace the non-hashed placeholder script in a shell with the real asset tags.
fn inject_assets(shell: &str, asset_tags: Option<&str>) -> String {
    match asset_tags {
        Some(tags) => shell.replace(r#"<script src="/assets/index.js"></script>"#, tags),
        None => shell.to_string(),
    }
}

/// Write every embedded SPA file to `out_dir`, preserving its relative path.
fn write_embedded_assets(out_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for (path, bytes) in crate::http::embedded_spa_files() {
        let dest = out_dir.join(&path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, &bytes)?;
    }
    Ok(())
}

/// Recursive directory copy (simple implementation).
fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !src.is_dir() {
        return Err(format!("not a directory: {}", src.display()).into());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

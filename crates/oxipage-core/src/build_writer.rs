//! Write build output to the `out/` directory.
//!
//! Handles creation of static HTML files, JSON data files, search index,
//! media copying, and web asset embedding.

use crate::builder::BuildOutput;
use std::fs;
use std::path::{Path, PathBuf};

/// Write a completed `BuildOutput` to the filesystem under `out_dir`.
///
/// Layout:
/// ```text
/// out/
/// ├── blog/my-post/index.html
/// ├── blog/my-post/index.md
/// ├── data/blog.json
/// ├── data/search-index.json
/// ├── media/           (copied from /data/media/)
/// └── assets/          (React SPA bundle from web/dist/)
/// ```
pub fn write_build_output(
    output: &BuildOutput,
    out_dir: &Path,
    media_dir: &Path,
    web_dist: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Clean or create output directory
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    // 2. Write all static pages
    for page in &output.pages {
        let file_path = out_dir.join(&page.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &page.content)?;
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

    // 5. Copy web assets (React SPA bundle)
    if web_dist.exists() {
        copy_dir_recursive(web_dist, &out_dir.join("assets"))?;
    }

    // 6. Copy media files
    if media_dir.exists() {
        let out_media = out_dir.join("media");
        copy_dir_recursive(media_dir, &out_media)?;
    }

    tracing::info!(
        pages = output.pages.len(),
        extensions = output.extensions_data.len(),
        dir = %out_dir.display(),
        "build output written"
    );

    Ok(())
}

/// Recursive directory copy (simple implementation).
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

//! Write build output to the `out/` directory.
//!
//! Handles creation of static HTML files, JSON data files, search index,
//! media copying, SPA bundle emission from the embedded binary, and the
//! `BuildManifest` that records the deployment base + asset revision.
//!
//! All public-facing asset tags are relativized (`/assets/...` → `assets/...`)
//! and a `<base href="{deployment_base}">` is injected so the same artifact
//! resolves under both a GitHub Pages project path and the preview prefix.

use crate::build_manifest::BuildManifest;
use crate::builder::{BuildInputs, BuildOutput};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Write a completed `BuildOutput` to the filesystem under `out_dir`.
///
/// Layout (v2 SSG):
/// ```text
/// out/
/// ├── index.html              ← SPA entry (embedded bundle), relativized + <base>
/// ├── 404.html                ← SPA fallback for deep links on static hosts
/// ├── assets/…                ← hashed JS/CSS chunks (embedded bundle)
/// ├── blog/<slug>/index.html  ← per-content SEO shell (OG meta + hydrates SPA)
/// ├── data/<ext>.json         ← collection JSON the SPA fetches
/// ├── data/search-index.json
/// ├── media/                  ← copied from /data/media/
/// └── .oxibuilder-build.json     ← BuildManifest (deployment_base, theme, revision)
/// ```
///
/// The SPA bundle is sourced from the embedded binary (`oxibuilder_core::http`),
/// NOT the working-directory `web/dist`, so a release binary builds a correct
/// site from any CWD. `deployment_base` is derived INSIDE this function from
/// `inputs.site_base_url` via [`BuildManifest::from_site_base`] — the single
/// derivation site for the whole codebase.
pub fn write_build_output(
    output: &BuildOutput,
    out_dir: &Path,
    media_dir: &Path,
    inputs: &BuildInputs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Clean or create output directory.
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    // 2. Derive deployment_base from site.base_url (the SINGLE derivation site).
    let deployment_base = BuildManifest::from_site_base(
        &inputs.site_base_url,
        &inputs.theme_id,
        &inputs.asset_revision_seed,
    )
    .deployment_base;

    // Absolute base (origin + deployment base) for OG image URLs, which
    // crawlers require to be absolute. Derived from the same `site.base_url`.
    let absolute_base = absolute_site_base(&inputs.site_base_url, &deployment_base);

    // 3. Pull the hashed <script>/<link> asset tags from the embedded static SPA
    //    index.html, transform `/assets/...` → relative, and prepend a
    //    `<base href="{deployment_base}">`.
    let asset_tags = extract_asset_tags(&deployment_base);

    // 4. Write all static pages, injecting the transformed asset tags into HTML
    //    shells and guaranteeing every HTML carries the deployment `<base href>`.
    //    BASE_PLACEHOLDER (injected by `markdown::render` while building each
    //    blog page) is resolved HERE in the per-page local — `output` is
    //    borrowed immutably, so we must not mutate it; rewriting the local
    //    `content` before `fs::write` is equivalent and avoids the corner
    //    case of double-mutating a shared collection.
    //
    //    `render_image_open` emits `{prefix}/{url}` literally (a single `/`
    //    separator); when called with `BASE_PLACEHOLDER` the emitted form is
    //    `\0BASE\0/media/...`. `deployment_base` is always trailing-slash
    //    terminated (`/blog/` or `/`). A bare `replace(BASE, base)` therefore
    //    produces `/blog//media/...` (project) or `//media/...` (apex →
    //    protocol-relative, broken in the no-JS / crawler view). We strip the
    //    separator FIRST — `BASE_PLACEHOLDER` + the `/` render inserts →
    //    `deployment_base` (which carries its own trailing slash) — and fall
    //    back to a bare `BASE_PLACEHOLDER` replace for any occurrence not
    //    followed by `/`. Result: apex → `/media/...`, project → `/blog/media/...`.
    for page in &output.pages {
        let mut content = if page.path.ends_with(".html") {
            let with_assets = inject_assets(&page.content, asset_tags.as_deref());
            ensure_base_href(&with_assets, &deployment_base)
        } else {
            page.content.clone()
        };
        if content.contains(crate::markdown::BASE_PLACEHOLDER) {
            let with_slash = format!("{}/", crate::markdown::BASE_PLACEHOLDER);
            content = content.replace(&with_slash, &deployment_base);
            content = content.replace(crate::markdown::BASE_PLACEHOLDER, &deployment_base);
        }
        if content.contains(crate::markdown::OG_IMAGE_PLACEHOLDER) {
            content = content.replace(crate::markdown::OG_IMAGE_PLACEHOLDER, &absolute_base);
        }
        let file_path = out_dir.join(&page.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &content)?;
    }

    // 5. Write extension data JSON files.
    let data_dir = out_dir.join("data");
    fs::create_dir_all(&data_dir)?;
    for (ext_id, data) in &output.extensions_data {
        let path = data_dir.join(format!("{ext_id}.json"));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, &json)?;
    }

    // 6. Collection shell fallback: ensure every extension with build data also
    //    has a `{ext}/index.html` landing page so direct loads return 200
    //    instead of a (SEO-harmful) 404.html fallback.
    let has_collection_shell: std::collections::HashSet<&str> = output
        .pages
        .iter()
        .filter_map(|p| {
            let mut parts = p.path.split('/');
            let head = parts.next()?;
            if parts.next() == Some("index.html") {
                Some(head)
            } else {
                None
            }
        })
        .collect();
    for (ext_id, _data) in &output.extensions_data {
        if has_collection_shell.contains(ext_id.as_str()) {
            continue;
        }
        let shell = inject_assets(
            &format!(
                r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>{ext_id}</title><link rel="icon" type="image/png" href="favicon-32.png"><link rel="canonical" href="/{ext_id}/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#
            ),
            asset_tags.as_deref(),
        );
        let path = out_dir.join(format!("{ext_id}/index.html"));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, &shell)?;
    }

    // 7. Core SPA route `/search` isn't extension-owned; give it the same shell.
    let search_path = out_dir.join("search/index.html");
    if !search_path.exists() {
        let shell = inject_assets(
            r#"<!DOCTYPE html><html lang="ko"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>Search</title><link rel="icon" type="image/png" href="favicon-32.png"><link rel="canonical" href="/search/"></head><body><div id="root"></div><script src="/assets/index.js"></script></body></html>"#,
            asset_tags.as_deref(),
        );
        fs::create_dir_all(search_path.parent().unwrap())?;
        fs::write(&search_path, &shell)?;
    }

    // 8. Write search index.
    let search_json = serde_json::to_string_pretty(&output.search_docs)?;
    fs::write(data_dir.join("search-index.json"), &search_json)?;

    // 9. Emit the embedded SPA bundle (the entry index.html is relativized +
    //    <base>-tagged) and a 404.html copy for static-host deep links.
    write_embedded_assets(out_dir, &deployment_base)?;
    let index_html = out_dir.join("index.html");
    if index_html.exists() {
        let _ = fs::copy(&index_html, out_dir.join("404.html"));
    }

    // 10. Copy media files.
    if media_dir.exists() {
        copy_dir_recursive(media_dir, &out_dir.join("media"))?;
    }

    // 10b. (Task 5) Copy optimized images from the staging dir into the
    //     freshly-cleaned out/. The image pre-pass wrote variants to
    //     `<staging>/media/_derived/` OUTSIDE out/, because out/ is wiped at
    //     step 1 — we re-materialize them here so the deployed site ships
    //     pre-resized WebP. Also emit the manifest as `out/data/image-manifest.json`
    //     so the static-mode SPA plugin (Task 6) can map `media/...` refs to
    //     srcset/dims without re-decoding source bytes.
    if let (Some(staging_dir), Some(manifest)) = (&inputs.image_staging_dir, &inputs.image_manifest)
    {
        let src_derived = staging_dir.join("media").join("_derived");
        let dst_derived = out_dir.join("media").join("_derived");
        if src_derived.is_dir() {
            copy_derived_into(&src_derived, &dst_derived)?;
        }
        let manifest_path = out_dir.join("data").join("image-manifest.json");
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(manifest)?;
        fs::write(&manifest_path, json)?;
    }

    // 11. Compute the final asset revision over the materialized output and
    //     write the manifest with the SAME deployment_base emitted into the HTML.
    let asset_revision = compute_asset_revision(out_dir);
    let manifest = BuildManifest {
        build_id: uuid::Uuid::new_v4().to_string(),
        deployment_base,
        theme_id: inputs.theme_id.clone(),
        asset_revision,
        built_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    manifest.write_to(out_dir)?;

    tracing::info!(
        pages = output.pages.len(),
        extensions = output.extensions_data.len(),
        build_id = %manifest.build_id,
        deployment_base = %manifest.deployment_base,
        dir = %out_dir.display(),
        "build output written"
    );

    Ok(())
}

/// Pull the hashed `<script>` and `<link rel="stylesheet">` tags out of the
/// embedded SPA `index.html`, convert `/assets/...` URLs to relative
/// `assets/...`, and prepend a `<base href="{deployment_base}">` tag so the
/// browser resolves relative asset URLs against the deployment base.
///
/// Returns `None` if the SPA isn't embedded or has no extractable asset tags.
fn extract_asset_tags(deployment_base: &str) -> Option<String> {
    let html = crate::http::static_spa_index_html()?;
    let mut tags: Vec<String> = Vec::new();
    tags.push(format!(r#"<base href="{}">"#, escape_attr(deployment_base)));
    for line in html.lines() {
        let t = line.trim();
        if t.starts_with("<script ") || t.starts_with("<link rel=\"stylesheet") {
            tags.push(relative_asset_tag(t));
        }
    }
    if tags.len() == 1 {
        // Only the <base> tag — no assets were extracted. Treat as missing.
        return None;
    }
    Some(tags.join("\n    "))
}

/// Convert `<script src="/assets/...">` and `<link ... href="/assets/...">`
/// to relative form. Other tags pass through unchanged.
fn relative_asset_tag(tag: &str) -> String {
    convert_asset_attr(tag, "src")
        .unwrap_or_else(|| convert_asset_attr(tag, "href").unwrap_or_else(|| tag.to_string()))
}

fn convert_asset_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    if !value.starts_with("/assets/") {
        return None;
    }
    let mut out = String::with_capacity(tag.len());
    out.push_str(&tag[..start]);
    out.push_str(&value[1..]); // strip leading slash → relative
    out.push('"');
    out.push_str(&rest[end + 1..]);
    Some(out)
}

/// Minimal HTML attribute escaper for the `<base href>` value. Neutralizes
/// `&`, `"`, `<` since `deployment_base` is application-controlled (origin +
/// path normalized to leading + trailing slash).
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// Absolute site base (`origin` + `deployment_base`) for OG image URLs —
/// crawlers require absolute URLs. `site.base_url` carries the origin; the
/// deployment base carries the pathname. Falls back to the path-only base
/// when no origin can be split off (invalid URL — `derive_deployment_base`
/// would have normalized it to `/` anyway).
fn absolute_site_base(base_url: &str, deployment_base: &str) -> String {
    let origin = base_url
        .split_once("://")
        .and_then(|(scheme, rest)| {
            rest.split('/')
                .next()
                .map(|host| format!("{scheme}://{host}"))
        })
        .unwrap_or_default();
    if origin.is_empty() {
        deployment_base.to_string()
    } else {
        format!("{origin}{deployment_base}")
    }
}

/// Replace the non-hashed placeholder script in a shell with the real asset tags.
fn inject_assets(shell: &str, asset_tags: Option<&str>) -> String {
    match asset_tags {
        Some(tags) => shell.replace(r#"<script src="/assets/index.js"></script>"#, tags),
        None => shell.to_string(),
    }
}

/// Idempotently ensure an HTML document carries the deployment `<base href>`.
/// If one is already present (e.g. injected via the asset-tag block), the
/// document is returned unchanged. Otherwise the tag is inserted before
/// `</head>` (or prepended if there is no head). This guarantees every served
/// HTML shell — including ones without a placeholder `<script>` — resolves
/// relative asset URLs against the deployment base.
fn ensure_base_href(html: &str, deployment_base: &str) -> String {
    if html.contains("<base href=") {
        return html.to_string();
    }
    let base = format!(r#"<base href="{}">"#, escape_attr(deployment_base));
    if let Some(idx) = html.find("</head>") {
        let (before, after) = html.split_at(idx);
        format!("{before}{base}{after}")
    } else {
        format!("{base}{html}")
    }
}

/// Write every embedded SPA file to `out_dir`, preserving its relative path.
/// The SPA entry `index.html` is transformed to relativize its own
/// `/assets/...` references and inject the deployment `<base href>` so the
/// same artifact resolves under a GitHub Pages project path.
fn write_embedded_assets(
    out_dir: &Path,
    deployment_base: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for (path, bytes) in crate::http::static_spa_files() {
        let dest = out_dir.join(&path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = if path == "index.html" {
            let html = String::from_utf8_lossy(&bytes);
            transform_spa_index(&html, deployment_base).into_bytes()
        } else {
            bytes
        };
        fs::write(&dest, &bytes)?;
    }
    Ok(())
}

/// Relativize a SPA index document's `/assets/...` references and inject a
/// `<base href="{deployment_base}">` before `</head>`. The base tag lets the
/// browser resolve the now-relative asset URLs against the deployment base
/// (apex `/` or project `/<repo>/`).
fn transform_spa_index(html: &str, deployment_base: &str) -> String {
    let relativized = html.replace("=\"/assets/", "=\"assets/");
    let base = format!(r#"<base href="{}">"#, escape_attr(deployment_base));
    if let Some(idx) = relativized.find("</head>") {
        let (before, after) = relativized.split_at(idx);
        format!("{before}{base}{after}")
    } else {
        format!("{base}{relativized}")
    }
}

/// Deterministic SHA-256 of the materialized output set (after writing).
/// Walks `out_dir` recursively, sorts entries, hashes `<relative_path>\0<bytes>`.
/// 16-byte prefix → 32 hex chars: terse in the manifest, collision-safe in the
/// per-site revision namespace.
fn compute_asset_revision(out_dir: &Path) -> String {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(out_dir, "", &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (name, data) in &entries {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(data);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(32);
    for b in &digest[..16] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn collect_files(base: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let dir = if rel.is_empty() {
        base
    } else {
        &base.join(rel)
    };
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                collect_files(base, &rel_path, out);
            } else if let Ok(data) = std::fs::read(&path) {
                out.push((rel_path, data));
            }
        }
    }
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

/// Like `copy_dir_recursive`, but skips the build-time `.cache.json` so the
/// canonical cache stays in staging (where subsequent builds can read it) and
/// the deployed site ships only the actual WebP variants + their dims.
fn copy_derived_into(
    src: &Path,
    dst: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !src.is_dir() {
        return Ok(()); // no derived images yet — nothing to copy, not an error
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == ".cache.json" {
            continue;
        }
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if file_type.is_dir() {
            copy_derived_into(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::absolute_site_base;

    #[test]
    fn absolute_base_apex() {
        assert_eq!(
            absolute_site_base("https://example.com", "/"),
            "https://example.com/"
        );
    }

    #[test]
    fn absolute_base_project_path() {
        assert_eq!(
            absolute_site_base("https://a7garden.github.io/blog/", "/blog/"),
            "https://a7garden.github.io/blog/"
        );
    }

    #[test]
    fn absolute_base_keeps_port() {
        assert_eq!(
            absolute_site_base("http://127.0.0.1:8787", "/"),
            "http://127.0.0.1:8787/"
        );
    }

    #[test]
    fn absolute_base_invalid_url_falls_back_to_path() {
        assert_eq!(absolute_site_base("not a url", "/"), "/");
    }
}

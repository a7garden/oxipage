//! Regression test for the SSG build pipeline.
//!
//! Verifies that `build_site` and `write_build_output` produce a working static
//! site. This is the test that would have caught the v0.4.0 build panic (every
//! `BuildExt` called `Handle::current()` on a rayon thread, which panics).
//!
//! Also asserts the embed-output contract that the deployed SPA relies on:
//!   - `out/index.html` and `out/404.html` exist (lobby entry + GitHub Pages fallback)
//!   - `out/assets/<file>` exists (the SPA bundle; no double-nesting)
//!   - the root `index.html` references the hashed script from the embedded bundle

use oxibuilder_core::build::build_site;
use oxibuilder_core::build_writer::write_build_output;
use oxibuilder_core::builder::{BuildExt, BuildOutput, SearchDoc, StaticPage};
use oxibuilder_core::db;
use oxibuilder_core::http::embedded_spa_files;
use sqlx::SqlitePool;

struct StubBuilder;
impl BuildExt for StubBuilder {
    fn ext_id(&self) -> &'static str {
        "stub"
    }
    fn build_pages(
        &self,
        _db: &SqlitePool,
        _rt: &tokio::runtime::Handle,
    ) -> Result<Vec<StaticPage>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
    fn build_data(
        &self,
        _db: &SqlitePool,
        _rt: &tokio::runtime::Handle,
    ) -> Result<Box<dyn erased_serde::Serialize + Send>, Box<dyn std::error::Error + Send + Sync>>
    {
        Ok(Box::new(serde_json::json!([])))
    }
    fn build_search_docs(
        &self,
        _db: &SqlitePool,
        _rt: &tokio::runtime::Handle,
    ) -> Result<Vec<SearchDoc>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn build_site_runs_without_panic_and_writes_correct_layout() {
    // 2. Spin up an empty SQLite + a media dir under the system temp root.
    let tmp_root = std::env::temp_dir().join(format!("oxibuilder-ssg-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_root);
    std::fs::create_dir_all(&tmp_root).unwrap();
    let db_path = tmp_root.join("oxibuilder.db");
    let media_dir = tmp_root.join("media");
    std::fs::create_dir_all(&media_dir).unwrap();
    let pool = db::connect(&db_path).await.unwrap();

    // 3. Run the real build pipeline.
    let builders: Vec<Box<dyn BuildExt>> = vec![Box::new(StubBuilder)];
    let output = build_site(&pool, &builders).expect("build_site should not panic");
    assert_eq!(output.extensions_data.len(), 1);

    // 4. Write to a fresh out dir and assert the layout.
    let out_dir = tmp_root.join("out");
    let inputs =
        oxibuilder_core::builder::BuildInputs::new("https://127.0.0.1:8787/", "paper", "test");
    write_build_output(&output, &out_dir, &media_dir, &inputs).expect("write_build_output");

    // Root SPA entry (lobby).
    assert!(
        out_dir.join("index.html").exists(),
        "out/index.html must exist"
    );

    // SPA fallback for deep links on hosts without a real SPA fallback
    // (GitHub Pages serves 404.html on 404).
    assert!(out_dir.join("404.html").exists(), "out/404.html must exist");

    // SPA bundle: hashed JS at /assets/, NOT /assets/assets/.
    assert!(out_dir.join("assets").is_dir(), "out/assets must exist");
    let bundled = embedded_spa_files();
    let js: Vec<_> = bundled
        .iter()
        .filter(|(p, _)| p.starts_with("assets/") && p.ends_with(".js"))
        .collect();
    assert!(
        !js.is_empty(),
        "embedded SPA must contain at least one JS chunk"
    );

    // Root index.html must reference the real relativized hashed script
    // (Task 2: `/assets/...` → `assets/...` + a deployment `<base href>`).
    let root = std::fs::read_to_string(out_dir.join("index.html")).unwrap();
    // Find the first `src="assets/...` (the relativized hashed chunk).
    // Non-asset tags like `<script src="/theme-boot.js">` precede it and are
    // intentionally left absolute, so search for the `assets/` reference.
    let root_ref = root
        .find("src=\"assets/")
        .map(|i| {
            let v = &root[i + 5..];
            v.split('"').next().unwrap_or("")
        })
        .unwrap_or("");
    assert!(
        root_ref.starts_with("assets/") && !root_ref.ends_with("/index.js"),
        "root index.html must reference a relativized hashed asset, got: {root_ref}"
    );
    assert!(
        root.contains("<base href=\"/\">"),
        "root index.html must carry the deployment base href"
    );
}

#[test]
fn embedded_spa_files_preserve_relative_paths() {
    // rust-embed stores paths relative to `#[folder]`. Verify we expose them so
    // `build_writer` can write them at the same relative path under `out/`.
    let files = embedded_spa_files();
    assert!(
        !files.is_empty(),
        "embedded-spa must contain at least index.html"
    );
    let names: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
    assert!(names.contains(&"index.html"), "index.html must be embedded");
    assert!(
        names
            .iter()
            .any(|p| p.starts_with("assets/") && p.ends_with(".js")),
        "at least one hashed JS chunk must be embedded"
    );
}

// --- Task 5 regression: staging survives the out/ wipe ---------------------

/// Minimal blog_post table — mirrors `oxibuilder-ext-blog/migrations/0001_init.sql`
/// so `run_image_pre_pass`'s `SELECT body FROM blog_post WHERE published_at IS NOT NULL`
/// works without booting the full extension migration runner.
const BLOG_POST_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS blog_post (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    lang TEXT NOT NULL DEFAULT 'ko' CHECK (lang IN ('ko','en')),
    translation_group_id INTEGER,
    tags JSON NOT NULL DEFAULT '[]',
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    FOREIGN KEY (translation_group_id) REFERENCES blog_post(id) ON DELETE SET NULL
);
"#;

#[tokio::test]
async fn derived_images_survive_out_wipe_and_manifest_is_written() {
    // 1. Spin up tmp dirs: data_dir holds the .db + media + (later) .image-build;
    //    out_dir is wiped by write_build_output and rebuilt from scratch.
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let out_dir = tmp.path().join("out");
    let media_dir = data_dir.join("media");
    std::fs::create_dir_all(&media_dir).unwrap();
    let db_path = data_dir.join("oxibuilder.db");

    // 2. Make a real source image big enough that all 4 configured widths apply
    //    (640/960/1280/1920) — proves the entry has a real srcset on disk.
    let src_path = media_dir.join("shot.png");
    let img = image::ImageBuffer::from_pixel(2000u32, 1125u32, image::Rgba([255u8, 0, 0, 255]));
    img.save(&src_path).unwrap();

    // 3. Open a fresh SQLite, install the blog_post schema, insert a published
    //    post whose body references the local media file. The pre-pass queries
    //    `published_at IS NOT NULL` to pick the row.
    let pool = db::connect(&db_path).await.unwrap();
    sqlx::query(BLOG_POST_SCHEMA).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO blog_post (slug, title, body, published_at) \
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    )
    .bind("hello")
    .bind("Hello")
    .bind("See ![shot](media/shot.png) for the picture.")
    .execute(&pool)
    .await
    .unwrap();

    // 4. Run the pre-pass against the real media_dir + data_dir.
    let (staging, manifest) =
        oxibuilder_core::build::run_image_pre_pass(&pool, &media_dir, &data_dir)
            .await
            .expect("pre-pass");

    let staging = staging.expect("staging dir present");
    let manifest = manifest.expect("manifest present");
    assert_eq!(manifest.entries.len(), 1, "one entry per unique ref");
    let entry = manifest.get("media/shot.png").expect("entry exists");
    assert_eq!(entry.srcset.len(), 4, "all four widths applied to a 2000px source");

    // 5. The staging tree has the on-disk WebP variants AND a build cache.
    let staging_derived = staging.join("media").join("_derived");
    assert!(staging_derived.is_dir(), "staging/<media>/_derived must exist");
    let staging_variants: Vec<String> = std::fs::read_dir(&staging_derived)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let staging_webp = staging_variants.iter().filter(|n| n.ends_with(".webp")).count();
    assert_eq!(staging_webp, 4, "4 .webp variants on staging");
    assert!(
        staging_variants.iter().any(|n| n == ".cache.json"),
        ".cache.json MUST live in staging so subsequent builds can read it"
    );

    // 6. Hand the staging + manifest to write_build_output. It wipes out/,
    //    rebuilds pages/data, then copies derived into out/media/_derived +
    //    writes out/data/image-manifest.json. This is the regression: any
    //    pre-pass output written directly into out/ would be destroyed by
    //    the wipe; the staging flow keeps it.
    let output = BuildOutput {
        pages: vec![],
        search_docs: vec![],
        extensions_data: vec![],
    };
    let mut inputs =
        oxibuilder_core::builder::BuildInputs::new("https://a7garden.github.io/blog/", "paper", "seed");
    inputs.image_staging_dir = Some(staging.clone());
    inputs.image_manifest = Some(manifest.clone());
    write_build_output(&output, &out_dir, &media_dir, &inputs).expect("write_build_output");

    // 6a. The out/media/_derived/ tree was re-materialized.
    let out_derived = out_dir.join("media").join("_derived");
    assert!(out_derived.is_dir(), "out/media/_derived must exist after write");
    let out_variants: Vec<String> = std::fs::read_dir(&out_derived)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let out_webp = out_variants.iter().filter(|n| n.ends_with(".webp")).count();
    assert_eq!(out_webp, 4, "4 .webp variants in out/media/_derived");

    // 6b. .cache.json MUST NOT leak into the deployed tree — it lives only in
    //     staging so subsequent builds can read it.
    assert!(
        !out_variants.iter().any(|n| n == ".cache.json"),
        ".cache.json must not ship to out/ (deployment bloat)"
    );

    // 6c. The manifest JSON shipped as out/data/image-manifest.json for the
    //     static-mode SPA plugin (Task 6) to read.
    let manifest_path = out_dir.join("data").join("image-manifest.json");
    let json = std::fs::read_to_string(&manifest_path).expect("manifest JSON exists");
    assert!(
        json.contains("\"media/shot.png\""),
        "manifest contains the entry: {json}"
    );
    assert!(json.contains(".webp"), "manifest includes srcset urls: {json}");
}

#[test]
fn base_placeholder_resolved_to_deployment_base_in_pages() {
    // Snapshot-style test: write a single page that embeds BASE_PLACEHOLDER,
    // run write_build_output with a project-pages base URL, and assert the
    // placeholder was rewritten into the deployment_base INSIDE the per-page
    // local — the function must NOT mutate `output` (it takes `&BuildOutput`),
    // so we re-read `output.pages[0].content` after writing and confirm it
    // still contains the raw placeholder.
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("out");
    let media_dir = tmp.path().join("media");
    std::fs::create_dir_all(&media_dir).unwrap();

    const BASE: &str = oxibuilder_core::markdown::BASE_PLACEHOLDER;
    let raw = format!(
        r#"<!DOCTYPE html><html><head><title>t</title></head><body><img src="{BASE}media/x.png"></body></html>"#
    );
    let page = StaticPage {
        path: "blog/hello/index.html".to_string(),
        content: raw.clone(),
    };
    let output = BuildOutput {
        pages: vec![page],
        search_docs: vec![],
        extensions_data: vec![],
    };
    // https://a7garden.github.io/blog/ → /blog/ — the canonical project-pages case.
    let inputs =
        oxibuilder_core::builder::BuildInputs::new("https://a7garden.github.io/blog/", "paper", "seed");
    write_build_output(&output, &out_dir, &media_dir, &inputs).expect("write_build_output");

    // 1. The on-disk file has the placeholder replaced with the real base.
    let on_disk =
        std::fs::read_to_string(out_dir.join("blog/hello/index.html")).expect("file written");
    assert!(
        !on_disk.contains(BASE),
        "BASE_PLACEHOLDER must NOT survive into the file (raw: {on_disk:?})"
    );
    assert!(
        on_disk.contains("src=\"/blog/media/x.png\""),
        "placeholder must be replaced with deployment_base /blog/: {on_disk}"
    );

    // 2. `output` is borrowed immutably — the page content the caller still
    //    holds MUST still contain the raw placeholder. If this regresses,
    //    write_build_output is mutating output and downstream callers (the
    //    build pipeline that re-reads the result for search/manifest
    //    reconciliation) will see corrupted state.
    assert!(
        output.pages[0].content.contains(BASE),
        "write_build_output must NOT mutate output — page content should still carry the raw placeholder"
    );

    // 3. Apex / user-pages case: placeholder → "/".
    let tmp2 = tempfile::tempdir().unwrap();
    let out2 = tmp2.path().join("out");
    let media2 = tmp2.path().join("media");
    std::fs::create_dir_all(&media2).unwrap();
    let raw2 = format!(r#"<img src="{BASE}media/y.png">"#);
    let output2 = BuildOutput {
        pages: vec![StaticPage {
            path: "x.html".to_string(),
            content: raw2,
        }],
        search_docs: vec![],
        extensions_data: vec![],
    };
    let inputs2 = oxibuilder_core::builder::BuildInputs::new("https://alice.github.io/", "paper", "seed");
    write_build_output(&output2, &out2, &media2, &inputs2).expect("write 2");
    let on_disk2 = std::fs::read_to_string(out2.join("x.html")).unwrap();
    assert!(
        on_disk2.contains("src=\"/media/y.png\""),
        "apex base: placeholder → '/', got: {on_disk2}"
    );
}

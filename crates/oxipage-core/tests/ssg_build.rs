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

use oxipage_core::build::build_site;
use oxipage_core::build_writer::write_build_output;
use oxipage_core::builder::{BuildExt, SearchDoc, StaticPage};
use oxipage_core::db;
use oxipage_core::http::embedded_spa_files;
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
    let tmp_root = std::env::temp_dir().join(format!("oxipage-ssg-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_root);
    std::fs::create_dir_all(&tmp_root).unwrap();
    let db_path = tmp_root.join("oxipage.db");
    let media_dir = tmp_root.join("media");
    std::fs::create_dir_all(&media_dir).unwrap();
    let pool = db::connect(&db_path).await.unwrap();

    // 3. Run the real build pipeline.
    let builders: Vec<Box<dyn BuildExt>> = vec![Box::new(StubBuilder)];
    let output = build_site(&pool, &builders).expect("build_site should not panic");
    assert_eq!(output.extensions_data.len(), 1);

    // 4. Write to a fresh out dir and assert the layout.
    let out_dir = tmp_root.join("out");
    let inputs = oxipage_core::builder::BuildInputs::new(
        "https://127.0.0.1:8787/",
        "paper",
        "test",
    );
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
    let root_ref = root
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
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

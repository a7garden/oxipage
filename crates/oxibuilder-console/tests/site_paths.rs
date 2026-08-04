//! Tests for SiteContext resolved paths.

use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::sites::SitesFile;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn site_context_resolves_absolute_data_dir() {
    let dir = TempDir::with_prefix("oxibuilder-paths-").unwrap();
    let toml = r#"[site]
name = "Test"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"
"#
    .to_string();
    std::fs::write(dir.path().join("oxibuilder.toml"), toml).unwrap();

    let mut sf = SitesFile::default();
    sf.add("test".into(), dir.path().to_path_buf());
    sf.set_default("test");

    let registry = Arc::new(SiteRegistry::new(sf, Default::default()).await.unwrap());
    let ctx = registry.ctx_for("test").await.unwrap();

    // data_dir should be project_dir/data (relative resolved against project_dir).
    assert_eq!(
        ctx.data_dir,
        dir.path().canonicalize().unwrap().join("data")
    );
    assert_eq!(ctx.out_dir, ctx.data_dir.join("out"));
    assert_eq!(ctx.media_dir, ctx.data_dir.join("media"));
    assert!(ctx.data_dir.is_absolute());
}

//! Tests for `SiteRegistry` — startup-loaded per-site contexts.

use oxibuilder_console::sites_runtime::SiteRegistry;
use oxibuilder_core::sites::SitesFile;
use std::path::PathBuf;
use tempfile::TempDir;

/// Minimal oxibuilder.toml content for a test site.
fn minimal_toml(name: &str) -> String {
    format!(
        r#"[site]
name = "{name}"
base_url = "http://127.0.0.1:8787"

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile", "blog"]
"#
    )
}

/// Create a temp directory with a minimal oxibuilder.toml and return (TempDir, path).
fn create_site_dir(name: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::with_prefix(format!("oxibuilder-test-{name}-")).unwrap();
    let toml_path = dir.path().join("oxibuilder.toml");
    std::fs::write(&toml_path, minimal_toml(name)).unwrap();
    let p = dir.path().to_path_buf();
    (dir, p)
}

#[tokio::test]
async fn registry_loads_each_valid_site_and_lookups_db() {
    let (_dir_a, path_a) = create_site_dir("TestA");
    let (_dir_b, path_b) = create_site_dir("TestB");

    let mut sf = SitesFile::default();
    sf.add("a".into(), path_a);
    sf.add("b".into(), path_b);
    sf.set_default("a");

    let reg = SiteRegistry::new(sf, Default::default()).await.unwrap();
    assert!(
        reg.db_for("a").await.is_some(),
        "site 'a' should have a DB pool"
    );
    assert!(
        reg.db_for("b").await.is_some(),
        "site 'b' should have a DB pool"
    );
    assert!(
        reg.db_for("missing").await.is_none(),
        "unknown slug should return None"
    );
    assert_eq!(reg.default_slug().await, Some("a".into()));
    assert_eq!(reg.len(), 2);
}

#[tokio::test]
async fn registry_skips_missing_path() {
    let mut sf = SitesFile::default();
    // Non-existent path — should be skipped with a warning.
    sf.add(
        "ghost".into(),
        PathBuf::from("/tmp/oxibuilder/nonexistent-XXXXXX"),
    );

    let reg = SiteRegistry::new(sf, Default::default()).await.unwrap();
    assert!(reg.db_for("ghost").await.is_none());
    assert_eq!(reg.len(), 0);
}

#[tokio::test]
async fn registry_default_slug_falls_back_to_first_site() {
    let (_dir_a, path_a) = create_site_dir("First");
    let (_dir_b, path_b) = create_site_dir("Second");

    let mut sf = SitesFile::default();
    sf.add("first".into(), path_a);
    sf.add("second".into(), path_b);
    // No default_site set — should fall back to first alphabetically
    assert_eq!(sf.default_site, None);

    let reg = SiteRegistry::new(sf, Default::default()).await.unwrap();
    let default = reg.default_slug().await;
    assert!(default.is_some(), "should fall back to first site");
}

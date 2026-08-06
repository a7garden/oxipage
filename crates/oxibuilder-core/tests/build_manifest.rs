//! Tests for BuildManifest serialization and deployment_base derivation.

use oxibuilder_core::build_manifest::{BuildManifest, MAG_FILENAME, derive_deployment_base};
use tempfile::TempDir;

#[test]
fn round_trip_preserves_fields() {
    let dir = TempDir::with_prefix("oxibuilder-mag-").unwrap();
    let m = BuildManifest {
        build_id: "11111111-2222-3333-4444-555555555555".to_string(),
        deployment_base: "/repo/".to_string(),
        theme_id: "paper".to_string(),
        layout_id: "shell".to_string(),
        asset_revision: "abcdef".to_string(),
        built_at: "2026-07-31T10:00:00Z".to_string(),
    };
    m.write_to(dir.path()).unwrap();
    let m2 = BuildManifest::read_from(dir.path())
        .unwrap()
        .expect("manifest written");
    assert_eq!(m.build_id, m2.build_id);
    assert_eq!(m.deployment_base, m2.deployment_base);
    assert_eq!(m.theme_id, m2.theme_id);
    assert_eq!(m.asset_revision, m2.asset_revision);
    assert_eq!(m.built_at, m2.built_at);
    assert_eq!(MAG_FILENAME, ".oxibuilder-build.json");
}

#[test]
fn read_returns_none_when_missing() {
    let dir = TempDir::with_prefix("oxibuilder-mag-missing-").unwrap();
    let got = BuildManifest::read_from(dir.path()).unwrap();
    assert!(got.is_none());
}

#[test]
fn write_to_missing_dir_creates_path() {
    let dir = TempDir::with_prefix("oxibuilder-mag-created-").unwrap();
    let out = dir.path().join("out");
    let m = BuildManifest::new("/myrepo/", "paper", "shell", "deadbeef");
    m.write_to(&out).unwrap();
    assert!(out.join(MAG_FILENAME).exists());
}

#[test]
fn derive_deployment_base_handles_apex_and_project_pages() {
    assert_eq!(derive_deployment_base("https://a7garden.github.io/"), "/");
    assert_eq!(
        derive_deployment_base("https://a7garden.github.io/blog/"),
        "/blog/"
    );
    assert_eq!(
        derive_deployment_base("https://example.com/deep/nested/"),
        "/deep/nested/"
    );
    assert_eq!(derive_deployment_base("http://127.0.0.1:8787/"), "/");
    // No trailing slash on the input — still produces a trailing slash on output.
    assert_eq!(derive_deployment_base("https://example.com/blog"), "/blog/");
}

#[test]
fn derive_deployment_base_falls_back_to_root_on_parse_error() {
    // Anything that fails URL parsing returns "/" — never blocks a build.
    assert_eq!(derive_deployment_base("not a url"), "/");
    assert_eq!(derive_deployment_base(""), "/");
    assert_eq!(derive_deployment_base("::::"), "/");
}

#[test]
fn new_helper_uses_supplied_base_without_normalization() {
    // BuildManifest::new is the low-level constructor; the caller is
    // expected to pass a normalized base. The high-level path goes through
    // derive_deployment_base (Task 2).
    let m = BuildManifest::new("/repo/", "paper", "shell", "deadbeef");
    assert_eq!(m.deployment_base, "/repo/");
    assert!(!m.build_id.is_empty());
    assert!(!m.built_at.is_empty());
}

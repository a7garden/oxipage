//! Tests for build_writer tag transformations and manifest derivation.

use oxipage_core::build_manifest::BuildManifest;
use oxipage_core::builder::{BuildInputs, BuildOutput, StaticPage};
use oxipage_core::build_writer::write_build_output;
use tempfile::TempDir;

fn page(rel: &str, body: &str) -> StaticPage {
    StaticPage {
        path: rel.to_string(),
        content: body.to_string(),
    }
}

fn empty_output_with(pages: Vec<StaticPage>) -> BuildOutput {
    BuildOutput {
        pages,
        search_docs: vec![],
        extensions_data: vec![],
    }
}

#[test]
fn relative_assets_drop_leading_slash() {
    let tmp = TempDir::with_prefix("oxipage-bw-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct = empty_output_with(vec![page(
        "index.html",
        r#"<!DOCTYPE html><html><head></head><body><script src="/assets/index.js"></script></body></html>"#,
    )]);
    let inputs = BuildInputs::new("https://a7garden.github.io/blog/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!html.contains("/assets/index-"), "raw /assets/ leaked: {html}");
    assert!(
        html.contains("assets/index-"),
        "relative asset missing: {html}"
    );
    assert!(
        html.contains("<base href=\"/blog/\">"),
        "base missing: {html}"
    );
}

#[test]
fn apex_base_url_emits_root_base() {
    let tmp = TempDir::with_prefix("oxipage-bw-apex-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct = empty_output_with(vec![page(
        "index.html",
        "<!DOCTYPE html><html><head></head><body></body></html>",
    )]);
    // Apex / user-pages deploy → base must be "/".
    let inputs = BuildInputs::new("https://a7garden.github.io/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(html.contains("<base href=\"/\">"), "expected `/` base: {html}");
}

#[test]
fn manifest_reflects_derived_deployment_base() {
    let tmp = TempDir::with_prefix("oxipage-bw-mag-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    let out_struct =
        empty_output_with(vec![page("index.html", "<!DOCTYPE html><html></html>")]);
    let inputs = BuildInputs::new("https://example.com/repo/", "paper", "seed");
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    let m = BuildManifest::read_from(&out).unwrap().expect("manifest exists");
    assert_eq!(m.deployment_base, "/repo/");
    assert_eq!(m.theme_id, "paper");
    assert!(!m.asset_revision.is_empty());
    assert!(!m.build_id.is_empty());
}

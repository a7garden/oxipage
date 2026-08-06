//! Static mount copy behavior + MountConfig→MountCopy mapping.

use oxibuilder_core::build_writer::write_build_output;
use oxibuilder_core::builder::{BuildInputs, BuildOutput, MountCopy, StaticPage};
use oxibuilder_core::config::MountConfig;
use tempfile::TempDir;

fn page(rel: &str, body: &str) -> StaticPage {
    StaticPage { path: rel.to_string(), content: body.to_string() }
}

fn empty_output_with(pages: Vec<StaticPage>) -> BuildOutput {
    BuildOutput { pages, search_docs: vec![], extensions_data: vec![] }
}

#[test]
fn write_build_output_copies_mount_into_out() {
    let tmp = TempDir::with_prefix("oxibuilder-mount-").unwrap();
    let out = tmp.path().join("out");
    let media = tmp.path().join("media");
    std::fs::create_dir_all(&media).unwrap();

    // Mount source: index.html + nested asset.
    let src = tmp.path().join("portfolio");
    std::fs::create_dir_all(src.join("assets")).unwrap();
    std::fs::write(src.join("index.html"), "<!DOCTYPE html><html>portfolio</html>").unwrap();
    std::fs::write(src.join("assets").join("pic.png"), b"PNGBYTES").unwrap();

    let out_struct = empty_output_with(vec![page(
        "index.html",
        "<!DOCTYPE html><html><body>lobby</body></html>",
    )]);
    // Test-only default: this writer test has no DB or on-disk config.
    let mut inputs = BuildInputs::new("https://example.com/", "paper", "shell", "seed");
    inputs.mounts = vec![MountCopy { source: src.clone(), path: "portfolio".into() }];
    write_build_output(&out_struct, &out, &media, &inputs).unwrap();

    // Mount materialized under out/portfolio/.
    let html = std::fs::read_to_string(out.join("portfolio").join("index.html")).unwrap();
    assert!(html.contains("portfolio"), "mount index missing: {html}");
    assert!(out.join("portfolio").join("assets").join("pic.png").exists(), "nested asset missing");

    // Core SPA shell survives. Step 9 of write_build_output writes the embedded
    // SPA's index.html over any user-emitted lobby page; the mount step (10c)
    // must not touch out/index.html. Assert the SPA shell is intact and that
    // the mount's content did not bleed into it.
    let shell = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!shell.contains("portfolio"), "core index clobbered by mount: {shell}");
}

#[test]
fn mount_copy_from_config_normalizes_path() {
    let mc = MountConfig {
        id: "p".into(),
        source: "/abs/portfolio".into(),
        path: "/portfolio/".into(),
        title_ko: "k".into(),
        title_en: "e".into(),
        description: None,
        icon: None,
        open_in_new_tab: false,
    };
    let copy = MountCopy::from_config(&mc);
    assert_eq!(copy.path, "portfolio", "leading/trailing slashes stripped");
    assert_eq!(copy.source, std::path::PathBuf::from("/abs/portfolio"));
}
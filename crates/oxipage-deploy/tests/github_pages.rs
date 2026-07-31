//! Tests for the repository-scoped GitHub Pages deploy core.

use oxipage_core::build_manifest::BuildManifest;
use oxipage_core::site_paths::GitHubPagesTarget;
use oxipage_deploy::{DeployError, deploy_github_pages, origin_matches};
use tempfile::TempDir;

#[test]
fn manifest_mismatch_precedes_git_changes() {
    let repo = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    std::fs::write(out.path().join("index.html"), "ok").unwrap();
    let target = GitHubPagesTarget {
        owner: "owner".into(),
        repo: "site".into(),
        branch: "gh-pages".into(),
    };
    let manifest = BuildManifest {
        build_id: "b1".into(),
        deployment_base: "/wrong/".into(),
        theme_id: "paper".into(),
        asset_revision: "a".into(),
        built_at: "2026-07-31T00:00:00Z".into(),
    };
    let (tx, _) = tokio::sync::mpsc::channel(8);
    assert!(matches!(
        deploy_github_pages(repo.path(), out.path(), &target, &manifest, &tx),
        Err(DeployError::ManifestBaseMismatch { .. })
    ));
    // No git state was touched.
    assert!(!repo.path().join(".git").exists());
}

#[test]
fn origin_matching_is_exact() {
    let t = GitHubPagesTarget {
        owner: "owner".into(),
        repo: "site".into(),
        branch: "gh-pages".into(),
    };
    assert!(origin_matches("https://github.com/owner/site.git", &t));
    assert!(origin_matches("git@github.com:owner/site.git", &t));
    assert!(!origin_matches("https://github.com/other/site.git", &t));
}

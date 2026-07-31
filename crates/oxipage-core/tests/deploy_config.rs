use oxipage_core::{config::Config, site_paths::GitHubPagesTarget};

fn target(owner: &str, repo: &str, branch: &str) -> GitHubPagesTarget {
    GitHubPagesTarget {
        owner: owner.into(),
        repo: repo.into(),
        branch: branch.into(),
    }
}

#[test]
fn parses_and_derives_pages_targets() {
    let cfg = Config::from_toml_str(
        r#"
[site]
name="Site"
base_url="https://project-oxi.github.io/oxipage/"
[deploy.github_pages]
owner="project-oxi"
repo="oxipage"
"#,
    )
    .unwrap();
    let pages = cfg.deploy.github_pages.unwrap();
    assert_eq!(pages.branch, "gh-pages");
    assert_eq!(pages.pages_url(), "https://project-oxi.github.io/oxipage/");
    assert_eq!(pages.base_path(), "/oxipage/");
    let root = target("a7garden", "a7garden.github.io", "pages/v1");
    assert_eq!(
        (root.pages_url(), root.base_path()),
        ("https://a7garden.github.io/".into(), "/".into())
    );
}

#[test]
fn project_repo_derives_subpath() {
    let p = target("a7garden", "notes", "gh-pages");
    assert_eq!(p.pages_url(), "https://a7garden.github.io/notes/");
    assert_eq!(p.base_path(), "/notes/");
}

#[test]
fn rejects_unsafe_values() {
    for value in ["", "owner/repo", "$(id)", "white space"] {
        assert!(target(value, "repo", "gh-pages").validate().is_err());
        assert!(target("owner", value, "gh-pages").validate().is_err());
    }
    for branch in ["", "../main", "/pages", "pages/", "pages shell", "pages;rm"] {
        assert!(
            target("owner", "repo", branch).validate().is_err(),
            "{branch}"
        );
    }
}

use crate::output::Output;
use clap::Args;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct DeployArgs {
    /// Target platform: github-pages, cloudflare, netlify
    #[arg(long, default_value = "github-pages")]
    pub target: String,
    /// Optional site name (from sites.toml)
    #[arg(long)]
    pub site: Option<String>,
    /// Dry run — shows what would be deployed without actually pushing
    #[arg(long)]
    pub dry_run: bool,
}

pub(crate) async fn deploy(c: DeployArgs, out: &Output) -> anyhow::Result<()> {
    // Resolve the project from the registered-site registry first (--site or
    // default), falling back to the legacy oxipage.toml path.
    let sites = crate::sites::load_sites();
    let legacy = std::env::var_os("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            Path::new("oxipage.toml")
                .exists()
                .then(|| PathBuf::from("oxipage.toml"))
        });
    let project = resolve_deploy_project(c.site.as_deref(), &sites, legacy.as_deref())?;
    let cfg = oxipage_core::config::Config::load(&project.join("oxipage.toml"))?;
    let data_dir = if cfg.server.data_dir.is_absolute() {
        cfg.server.data_dir
    } else {
        project.join(&cfg.server.data_dir)
    };
    let out_dir = data_dir.join("out");

    if c.dry_run {
        let target = cfg
            .deploy
            .github_pages
            .ok_or_else(|| anyhow::anyhow!("[deploy.github_pages] is not configured"))?;
        let _ = out.ok(format!(
            "dry-run: would deploy {} to {}",
            out_dir.display(),
            target.pages_url()
        ));
        return Ok(());
    }

    match c.target.as_str() {
        "github-pages" => {
            let target = cfg
                .deploy
                .github_pages
                .ok_or_else(|| anyhow::anyhow!("[deploy.github_pages] is not configured"))?;
            target.validate()?;
            let manifest = oxipage_core::build_manifest::BuildManifest::read_from(&out_dir)?
                .ok_or_else(|| {
                    anyhow::anyhow!("build required: no manifest in {}", out_dir.display())
                })?;

            let (tx, mut rx) = tokio::sync::mpsc::channel::<oxipage_deploy::DeployEvent>(32);
            let repo = project.clone();
            let out_dir_owned = out_dir.clone();
            let target2 = target.clone();
            let handle = tokio::task::spawn_blocking(move || {
                oxipage_deploy::deploy_github_pages(&repo, &out_dir_owned, &target2, &manifest, &tx)
            });
            while let Some(ev) = rx.recv().await {
                let _ = out.ok(deploy_event_label(&ev));
            }
            match handle.await {
                Ok(Ok(oxipage_deploy::DeployOutcome::Deployed { url, commit })) => {
                    out.ok(format!("deployed {commit} to {url}"))
                }
                Ok(Ok(oxipage_deploy::DeployOutcome::Unchanged { url, commit })) => {
                    out.ok(format!("unchanged at {commit}: {url}"))
                }
                Ok(Err(e)) => Err(e.into()),
                Err(e) => Err(anyhow::anyhow!("deploy task panicked: {e}")),
            }
        }
        "cloudflare" | "netlify" => {
            anyhow::bail!("{} target not yet implemented", c.target);
        }
        _ => anyhow::bail!("unsupported deploy target: {}", c.target),
    }
}

/// Resolve the deploy project directory: registered site (--site or default)
/// wins; otherwise fall back to the legacy oxipage.toml path's parent.
pub fn resolve_deploy_project(
    requested: Option<&str>,
    sites: &oxipage_core::sites::SitesFile,
    legacy: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if !sites.sites.is_empty() {
        // Validate an explicitly requested site EXISTS before resolve_name
        // (which silently falls back to the default otherwise).
        if let Some(name) = requested
            && !sites.sites.contains_key(name)
        {
            return Err(anyhow::anyhow!("site '{name}' is not registered"));
        }
        let name = sites
            .resolve_name(requested)
            .ok_or_else(|| anyhow::anyhow!("select a site with --site or set a default"))?;
        return sites
            .sites
            .get(name)
            .map(|e| e.path.clone())
            .ok_or_else(|| anyhow::anyhow!("site '{name}' is not registered"));
    }
    legacy
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("no registered site and no oxipage.toml"))
}

fn deploy_event_label(ev: &oxipage_deploy::DeployEvent) -> String {
    use oxipage_deploy::DeployEvent;
    match ev {
        DeployEvent::PreflightStarted => "Preflight checks…".into(),
        DeployEvent::GhReady => "GitHub CLI ready".into(),
        DeployEvent::AuthReady => "Authenticated with GitHub CLI".into(),
        DeployEvent::RepositoryReady => "Repository verified".into(),
        DeployEvent::WorktreeReady => "Prepared gh-pages worktree".into(),
        DeployEvent::FilesCopied { count } => format!("Copied {count} files to worktree"),
        DeployEvent::CommitCreated { commit } => format!("Committed {commit}"),
        DeployEvent::Pushing { branch } => format!("Pushing to {branch}…"),
        DeployEvent::Deployed { url, commit } => {
            format!("Deployed {commit} to GitHub Pages: {url}")
        }
        DeployEvent::Unchanged { url, commit } => {
            format!("Unchanged at {commit}: {url}")
        }
        DeployEvent::Failed { error, .. } => format!("Deploy failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_deploy_project;
    use oxipage_core::sites::SitesFile;
    use std::path::{Path, PathBuf};

    fn registry() -> SitesFile {
        let mut sf = SitesFile::default();
        sf.add("alpha".into(), PathBuf::from("/sites/alpha"));
        sf.add("beta".into(), PathBuf::from("/sites/beta"));
        sf.set_default("alpha");
        sf
    }

    #[test]
    fn explicit_site_wins() {
        assert_eq!(
            resolve_deploy_project(Some("beta"), &registry(), None).unwrap(),
            PathBuf::from("/sites/beta")
        );
    }

    #[test]
    fn default_is_used() {
        assert_eq!(
            resolve_deploy_project(None, &registry(), None).unwrap(),
            PathBuf::from("/sites/alpha")
        );
    }

    #[test]
    fn legacy_only_without_registry() {
        assert_eq!(
            resolve_deploy_project(
                None,
                &SitesFile::default(),
                Some(Path::new("/legacy/oxipage.toml"))
            )
            .unwrap(),
            PathBuf::from("/legacy")
        );
        assert!(
            resolve_deploy_project(
                Some("missing"),
                &registry(),
                Some(Path::new("/legacy/oxipage.toml"))
            )
            .is_err()
        );
    }

    #[test]
    fn missing_site_is_err() {
        assert!(resolve_deploy_project(Some("nope"), &registry(), None).is_err());
    }
}
